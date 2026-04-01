//! Render pipeline: DOM tree -> layout -> screen -> diff -> ANSI output.
//!
//! The `Renderer` owns front and back screen buffers, and produces ANSI diff
//! strings for incremental terminal updates. The pipeline matches the ref:
//!
//! 1. Build/update DOM tree (caller-managed via widgets)
//! 2. Sync layout nodes with taffy
//! 3. Compute layout (taffy flexbox)
//! 4. Walk DOM tree with computed positions -> write styled text to back buffer
//! 5. Diff front vs back -> generate ANSI delta
//! 6. Swap buffers

use crate::tui::cell::{Cell, CellWidth, STYLE_SHIFT};
use crate::tui::components::BorderStyle;
use crate::tui::dom::{DomElement, DomNode, DomTree, NodeId};
use crate::tui::layout::{ElementType, LayoutEngine, LayoutOverflow};
use crate::tui::pools::{CharPool, HyperlinkPool, StylePool};
use crate::tui::screen::Screen;
use crate::tui::style::{Style, TextStyles};
use crate::tui::termio::{csi, osc};
use crate::tui::text;

/// Clipping rectangle for overflow:hidden rendering.
#[derive(Debug, Clone, Copy)]
struct ClipRect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl ClipRect {
    /// Intersect two clip rects. Returns the tighter constraint.
    fn intersect(self, other: ClipRect) -> ClipRect {
        ClipRect {
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
            x2: self.x2.min(other.x2),
            y2: self.y2.min(other.y2),
        }
    }

    /// Check if a point is within the clip rect.
    #[allow(dead_code)]
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x1 && x < self.x2 && y >= self.y1 && y < self.y2
    }

    /// Check if the clip rect has any area.
    fn is_valid(&self) -> bool {
        self.x2 > self.x1 && self.y2 > self.y1
    }
}

/// Double-buffered terminal renderer with full DOM-to-screen pipeline.
pub struct Renderer {
    /// Front buffer: what the terminal currently shows.
    pub front: Screen,
    /// Back buffer: what we're rendering into.
    pub back: Screen,
    /// Layout engine (taffy).
    pub layout: LayoutEngine,
    /// Shared character pool.
    pub char_pool: CharPool,
    /// Shared hyperlink pool.
    pub hyperlink_pool: HyperlinkPool,
    /// Shared style pool.
    pub style_pool: StylePool,
    /// Terminal width.
    pub width: u16,
    /// Terminal height.
    pub height: u16,
}

impl Renderer {
    /// Create a new renderer with given terminal dimensions.
    pub fn new(width: u16, height: u16) -> Self {
        let style_pool = StylePool::new();
        let front = Screen::new(width as usize, height as usize, &style_pool);
        let back = Screen::new(width as usize, height as usize, &style_pool);
        Self {
            front,
            back,
            layout: LayoutEngine::new(),
            char_pool: CharPool::new(),
            hyperlink_pool: HyperlinkPool::new(),
            style_pool,
            width,
            height,
        }
    }

    /// Resize the renderer. Resets both buffers.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.front.reset(width as usize, height as usize);
        self.back.reset(width as usize, height as usize);
    }

    // -----------------------------------------------------------------------
    // Full render pipeline
    // -----------------------------------------------------------------------

    /// Run the full render pipeline:
    /// 1. Sync DOM tree to taffy layout nodes
    /// 2. Compute layout
    /// 3. Render DOM to back buffer
    /// 4. Diff front vs back
    /// 5. Swap buffers
    ///
    /// Returns the ANSI string to write to stdout.
    pub fn render(&mut self, dom: &mut DomTree, root: NodeId) -> String {
        // Reset the taffy tree before syncing. The DOM is rebuilt from scratch
        // each frame (in App::build_dom), so old taffy nodes would be orphaned.
        // We also clear layout_node references so sync creates fresh nodes.
        self.layout = LayoutEngine::new();
        clear_layout_nodes(dom, root);

        // Step 1: sync DOM to taffy layout nodes
        self.sync_layout_nodes(dom, root);

        // Step 2: compute layout
        if let Some(layout_node) = dom.element(root).and_then(|e| e.layout_node) {
            self.layout.compute_layout(
                layout_node,
                self.width as f32,
                self.height as f32,
            );
        }

        // Step 3: render DOM tree to back buffer
        let screen_clip = ClipRect {
            x1: 0,
            y1: 0,
            x2: self.width as i32,
            y2: self.height as i32,
        };
        self.render_node_to_screen(dom, root, 0, 0, screen_clip);

        // Step 4: diff front vs back
        let output = self.diff_screens();

        // Step 5: swap buffers
        std::mem::swap(&mut self.front, &mut self.back);
        self.back.reset(self.width as usize, self.height as usize);

        // Clear dirty flags
        dom.clear_dirty(root);

        output
    }

    // -----------------------------------------------------------------------
    // Step 1: Sync DOM elements to taffy layout nodes
    // -----------------------------------------------------------------------

    /// Ensure every DOM element that needs layout has a taffy node,
    /// and the parent-child relationships in taffy match the DOM tree.
    pub fn sync_layout_nodes(&mut self, dom: &mut DomTree, node_id: NodeId) {
        let node = match dom.get(node_id) {
            Some(n) => n,
            None => return,
        };

        match node {
            DomNode::Element(elem) => {
                let elem_type = elem.element_type;
                let needs_layout = elem_type.needs_layout_node();

                // Create taffy node if needed
                if needs_layout && elem.layout_node.is_none() {
                    let layout_node = self.layout.new_leaf(elem_type);
                    if let Some(elem_mut) = dom.element_mut(node_id) {
                        elem_mut.layout_node = Some(layout_node);
                    }
                }

                // Update style on the taffy node
                if needs_layout {
                    if let Some(elem) = dom.element(node_id) {
                        if let Some(layout_node) = elem.layout_node {
                            let taffy_style = build_taffy_style(elem, dom);
                            self.layout.set_style(layout_node, taffy_style);
                        }
                    }
                }

                // Recurse children
                let children: Vec<NodeId> = dom
                    .element(node_id)
                    .map(|e| e.children.clone())
                    .unwrap_or_default();

                for &child_id in &children {
                    self.sync_layout_nodes(dom, child_id);
                }

                // Sync taffy children in order
                if needs_layout {
                    if let Some(elem) = dom.element(node_id) {
                        if let Some(parent_layout) = elem.layout_node {
                            let child_layout_nodes: Vec<taffy::NodeId> = children
                                .iter()
                                .filter_map(|&cid| {
                                    dom.element(cid).and_then(|ce| ce.layout_node)
                                })
                                .collect();

                            // Set children in taffy
                            self.layout
                                .taffy
                                .set_children(parent_layout, &child_layout_nodes)
                                .ok();
                        }
                    }
                }

                // Handle text measurement for Text elements
                if elem_type == ElementType::Text || elem_type == ElementType::RawAnsi {
                    self.setup_text_measure(dom, node_id);
                }
            }
            DomNode::Text(_) => {
                // Text nodes don't have layout nodes; they're measured
                // via their parent Text element's measure function.
            }
        }
    }

    /// Set up text measurement on a Text element's taffy node.
    ///
    /// Stores the text content in the taffy node's context data so that
    /// the measure callback can compute wrapped dimensions dynamically
    /// during layout (when the available width is known). This mirrors
    /// the ref's `measureTextNode` bound to `yogaNode.setMeasureFunc()`.
    fn setup_text_measure(&mut self, dom: &DomTree, node_id: NodeId) {
        let elem = match dom.element(node_id) {
            Some(e) => e,
            None => return,
        };
        let layout_node = match elem.layout_node {
            Some(n) => n,
            None => return,
        };

        // Collect text content from children
        let text_content = collect_text_content(dom, node_id);

        if text_content.is_empty() {
            return;
        }

        // Store text content in the taffy node context so the measure
        // callback can compute correct wrapped dimensions.
        if let Some(ctx) = self.layout.taffy.get_node_context_mut(layout_node) {
            ctx.text_content = Some(text_content);
        }
    }

    // -----------------------------------------------------------------------
    // Step 3: Render DOM tree to screen buffer
    // -----------------------------------------------------------------------

    /// Walk the DOM tree with computed positions and write styled characters
    /// to the back buffer. This is the port of render-node-to-output.ts.
    fn render_node_to_screen(
        &mut self,
        dom: &DomTree,
        node_id: NodeId,
        offset_x: i32,
        offset_y: i32,
        clip: ClipRect,
    ) {
        let elem = match dom.element(node_id) {
            Some(e) => e,
            None => return,
        };

        // Skip hidden nodes
        if elem.is_hidden {
            return;
        }

        // Get computed layout from taffy
        let layout_node = match elem.layout_node {
            Some(n) => n,
            None => return,
        };

        let layout = self.layout.layout(layout_node).clone();

        let x = offset_x + layout.location.x as i32;
        let y = offset_y + layout.location.y as i32;
        let w = layout.size.width as i32;
        let h = layout.size.height as i32;

        if w <= 0 || h <= 0 {
            return;
        }

        // Determine clip for this node
        let node_clip = match elem.layout_style.overflow {
            LayoutOverflow::Hidden | LayoutOverflow::Scroll => {
                let node_bounds = ClipRect {
                    x1: x,
                    y1: y,
                    x2: x + w,
                    y2: y + h,
                };
                clip.intersect(node_bounds)
            }
            LayoutOverflow::Visible => clip,
        };

        if !node_clip.is_valid() {
            return;
        }

        // Compute scroll offset for overflow:scroll
        let scroll_top = if elem.layout_style.overflow == LayoutOverflow::Scroll {
            elem.scroll_top.unwrap_or(0)
        } else {
            0
        };

        // Get border metrics
        let border_style = get_border_style(elem);
        let has_border = border_style != BorderStyle::None;
        let border_width = if has_border { 1i32 } else { 0 };

        // Draw borders
        if has_border {
            self.draw_borders(
                elem, border_style, x, y, w, h, &clip,
            );
        }

        // Fill background if present
        if let Some(ref text_styles) = elem.text_styles {
            if text_styles.background_color.is_some() {
                self.fill_background(elem, text_styles, x, y, w, h, &clip);
            }
        }

        // Compute inner area (inside borders and padding)
        let padding = &elem.layout_style.padding;
        let inner_x = x + border_width + padding.left as i32;
        let inner_y = y + border_width + padding.top as i32 - scroll_top;
        let inner_w = (w - border_width * 2 - padding.left as i32 - padding.right as i32).max(0);
        let _inner_h = (h - border_width * 2 - padding.top as i32 - padding.bottom as i32).max(0);

        // Render content based on element type
        match elem.element_type {
            ElementType::Text | ElementType::RawAnsi => {
                self.render_text_node(dom, node_id, inner_x, inner_y, inner_w, &node_clip);
            }
            ElementType::Root | ElementType::Box => {
                // Recurse into children using `x, y` (NOT inner_x, inner_y).
                // Taffy's computed `layout.location` for children already accounts
                // for the parent's padding, so adding padding again would double-count.
                let children: Vec<NodeId> = elem.children.clone();
                for child_id in children {
                    self.render_node_to_screen(
                        dom,
                        child_id,
                        x,
                        y - scroll_top,
                        node_clip,
                    );
                }
            }
            _ => {
                // VirtualText, Link, Progress don't have their own layout
            }
        }
    }

    // -----------------------------------------------------------------------
    // Text rendering
    // -----------------------------------------------------------------------

    /// Render a Text element's content into the screen buffer.
    ///
    /// Text may contain embedded ANSI escape sequences (e.g., from
    /// `set_welcome` or transcript entries). We parse those as style
    /// changes rather than rendering escape bytes as visible characters.
    fn render_text_node(
        &mut self,
        dom: &DomTree,
        node_id: NodeId,
        x: i32,
        y: i32,
        max_width: i32,
        clip: &ClipRect,
    ) {
        let elem = match dom.element(node_id) {
            Some(e) => e,
            None => return,
        };

        if max_width <= 0 {
            return;
        }

        // Collect text content from child text nodes
        let raw_text = collect_text_content(dom, node_id);
        if raw_text.is_empty() {
            return;
        }

        // Get text wrap mode
        let text_wrap = elem.layout_style.text_wrap;

        // Wrap the text (wrap_text is ANSI-aware for width computation)
        let wrapped = text::wrap_text(&raw_text, max_width as usize, text_wrap);

        // Resolve the base style for this text element
        let base_style = resolve_text_styles(elem);
        let base_style_id = self.intern_style(&base_style);

        // Parse the wrapped text line by line, handling embedded ANSI escapes.
        // We maintain a running SGR state that ANSI codes modify.
        let mut ansi_style = base_style.clone();
        let mut cur_y = y;

        for line in wrapped.split('\n') {
            if cur_y >= clip.y2 {
                break;
            }
            if cur_y >= clip.y1 {
                let mut cur_x = x;
                let mut chars = line.chars().peekable();

                while let Some(ch) = chars.next() {
                    if cur_x >= clip.x2 {
                        break;
                    }

                    // Handle ANSI escape sequences
                    if ch == '\x1b' {
                        if chars.peek() == Some(&'[') {
                            chars.next(); // consume '['
                            // Read CSI parameters until final byte (0x40-0x7E)
                            let mut params = String::new();
                            while let Some(&next) = chars.peek() {
                                if (0x40..=0x7E).contains(&(next as u32)) {
                                    let final_byte = chars.next().unwrap();
                                    if final_byte == 'm' {
                                        // SGR sequence: apply to running style
                                        crate::tui::termio::sgr::apply_sgr(&params, &mut ansi_style);
                                    }
                                    break;
                                }
                                params.push(chars.next().unwrap());
                            }
                        } else if chars.peek() == Some(&']') {
                            // OSC sequence: skip until BEL or ST
                            chars.next();
                            while let Some(&next) = chars.peek() {
                                chars.next();
                                if next == '\x07' {
                                    break;
                                }
                                if next == '\x1b' {
                                    if chars.peek() == Some(&'\\') {
                                        chars.next();
                                    }
                                    break;
                                }
                            }
                        } else {
                            // Simple ESC sequence (ESC + one char) - skip
                            chars.next();
                        }
                        continue;
                    }

                    // Skip control characters
                    if ch.is_control() {
                        continue;
                    }

                    let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);

                    // Skip zero-width characters
                    if char_width == 0 {
                        continue;
                    }

                    let is_wide = char_width >= 2;

                    // Compute style for this character
                    let char_style_id = if ansi_style == base_style {
                        base_style_id
                    } else {
                        self.intern_style(&ansi_style)
                    };

                    // Handle wide char at screen edge
                    if is_wide && cur_x + 2 > clip.x2 {
                        if cur_x >= clip.x1 && cur_x >= 0 {
                            self.back.set_cell_at(
                                cur_x as usize,
                                cur_y as usize,
                                &Cell {
                                    char: " ".into(),
                                    style_id: self.style_pool.none,
                                    width: CellWidth::SpacerHead,
                                    hyperlink: None,
                                },
                                &mut self.char_pool,
                                &mut self.hyperlink_pool,
                            );
                        }
                        cur_x += 1;
                        continue;
                    }

                    if cur_x >= clip.x1 && cur_x >= 0 {
                        let cell_width = if is_wide {
                            CellWidth::Wide
                        } else {
                            CellWidth::Narrow
                        };
                        self.back.set_cell_at(
                            cur_x as usize,
                            cur_y as usize,
                            &Cell {
                                char: ch.to_string(),
                                style_id: char_style_id,
                                width: cell_width,
                                hyperlink: None,
                            },
                            &mut self.char_pool,
                            &mut self.hyperlink_pool,
                        );
                    }
                    cur_x += if is_wide { 2 } else { 1 };
                }
            }
            cur_y += 1;
        }
    }

    // -----------------------------------------------------------------------
    // Border rendering
    // -----------------------------------------------------------------------

    /// Draw border characters around a box element.
    fn draw_borders(
        &mut self,
        elem: &DomElement,
        border_style: BorderStyle,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        clip: &ClipRect,
    ) {
        let chars = match border_style.chars() {
            Some(c) => c,
            None => return,
        };

        // Resolve border color
        let border_color_style = get_border_color_style(elem);
        let border_style_id = self.intern_style(&border_color_style);

        // Top border: top_left + horizontal... + top_right
        if y >= clip.y1 && y < clip.y2 {
            self.write_border_char(x, y, chars.top_left, border_style_id, clip);
            for col in 1..(w - 1) {
                self.write_border_char(x + col, y, chars.horizontal, border_style_id, clip);
            }
            if w > 1 {
                self.write_border_char(x + w - 1, y, chars.top_right, border_style_id, clip);
            }
        }

        // Side borders
        for row in 1..(h - 1) {
            let cy = y + row;
            if cy >= clip.y1 && cy < clip.y2 {
                self.write_border_char(x, cy, chars.vertical, border_style_id, clip);
                if w > 1 {
                    self.write_border_char(
                        x + w - 1,
                        cy,
                        chars.vertical,
                        border_style_id,
                        clip,
                    );
                }
            }
        }

        // Bottom border: bottom_left + horizontal... + bottom_right
        let bottom_y = y + h - 1;
        if h > 1 && bottom_y >= clip.y1 && bottom_y < clip.y2 {
            self.write_border_char(x, bottom_y, chars.bottom_left, border_style_id, clip);
            for col in 1..(w - 1) {
                self.write_border_char(
                    x + col,
                    bottom_y,
                    chars.horizontal,
                    border_style_id,
                    clip,
                );
            }
            if w > 1 {
                self.write_border_char(
                    x + w - 1,
                    bottom_y,
                    chars.bottom_right,
                    border_style_id,
                    clip,
                );
            }
        }
    }

    /// Write a single border character to the back buffer.
    fn write_border_char(
        &mut self,
        x: i32,
        y: i32,
        ch: char,
        style_id: u32,
        clip: &ClipRect,
    ) {
        if x >= clip.x1 && x < clip.x2 && y >= clip.y1 && y < clip.y2 {
            let ux = x as usize;
            let uy = y as usize;
            if ux < self.back.width && uy < self.back.height {
                self.back.set_cell_at(
                    ux,
                    uy,
                    &Cell {
                        char: ch.to_string(),
                        style_id,
                        width: CellWidth::Narrow,
                        hyperlink: None,
                    },
                    &mut self.char_pool,
                    &mut self.hyperlink_pool,
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Background filling
    // -----------------------------------------------------------------------

    /// Fill a box's area with its background color.
    fn fill_background(
        &mut self,
        _elem: &DomElement,
        text_styles: &TextStyles,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        clip: &ClipRect,
    ) {
        let bg_style = Style {
            bg_color: text_styles.background_color.clone(),
            ..Style::default()
        };
        let bg_style_id = self.intern_style(&bg_style);

        let x1 = x.max(clip.x1);
        let y1 = y.max(clip.y1);
        let x2 = (x + w).min(clip.x2);
        let y2 = (y + h).min(clip.y2);

        for row in y1..y2 {
            for col in x1..x2 {
                let ux = col as usize;
                let uy = row as usize;
                if ux < self.back.width && uy < self.back.height {
                    // Only fill if cell is currently empty
                    if self.back.is_empty_at(ux, uy) {
                        self.back.set_cell_at(
                            ux,
                            uy,
                            &Cell {
                                char: " ".into(),
                                style_id: bg_style_id,
                                width: CellWidth::Narrow,
                                hyperlink: None,
                            },
                            &mut self.char_pool,
                            &mut self.hyperlink_pool,
                        );
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Style interning
    // -----------------------------------------------------------------------

    /// Convert a resolved `Style` to an interned style_id.
    fn intern_style(&mut self, style: &Style) -> u32 {
        let codes = style.to_ansi_codes();
        self.style_pool.intern(&codes)
    }

    // -----------------------------------------------------------------------
    // Step 4: Screen diffing
    // -----------------------------------------------------------------------

    /// Compute the ANSI diff between front and back buffers.
    pub fn diff_screens(&mut self) -> String {
        let w = self.back.width;
        let h = self.back.height;

        // Determine the scan region from damage
        let (scan_min_y, scan_max_y, scan_min_x, scan_max_x) = match self.back.damage {
            Some(d) => (
                d.y,
                (d.y + d.height).min(h),
                d.x,
                (d.x + d.width).min(w),
            ),
            None => return String::new(), // No damage, nothing to diff
        };

        let mut output = String::with_capacity(w * h);
        let mut last_style_id: u32 = 0;
        let mut active_hyperlink: Option<String> = None;
        let mut cursor_row: usize = usize::MAX;
        let mut cursor_col: usize = usize::MAX;

        for y in scan_min_y..scan_max_y {
            for x in scan_min_x..scan_max_x {
                let back_ci = (y * w + x) * 2;
                let front_ci = if y < self.front.height && x < self.front.width {
                    (y * self.front.width + x) * 2
                } else {
                    usize::MAX
                };

                let back_w0 = self.back.cells[back_ci];
                let back_w1 = self.back.cells[back_ci + 1];

                // Check if cell changed
                let front_w0 = if front_ci < self.front.cells.len() {
                    self.front.cells[front_ci]
                } else {
                    0
                };
                let front_w1 = if front_ci + 1 < self.front.cells.len() {
                    self.front.cells[front_ci + 1]
                } else {
                    0
                };

                if back_w0 == front_w0 && back_w1 == front_w1 {
                    continue; // No change
                }

                let width = CellWidth::from_u32(back_w1);

                // Skip spacer cells
                if width == CellWidth::SpacerTail || width == CellWidth::SpacerHead {
                    continue;
                }

                // Skip invisible spaces (empty char, no hyperlink, fg-only style)
                if back_w0 == 0 && (back_w1 & 0x3fffc) == 0 {
                    let fg_style = back_w1 >> STYLE_SHIFT;
                    if fg_style == 0 || fg_style == last_style_id {
                        // Also check if the front buffer had content that needs clearing
                        if front_w0 == 0 && front_w1 == 0 {
                            continue;
                        }
                    }
                }

                // Position cursor
                if cursor_row != y || cursor_col != x {
                    output.push_str(&csi::cursor_position(y as u32 + 1, x as u32 + 1));
                    cursor_row = y;
                    cursor_col = x;
                }

                // Style transition
                let style_id = back_w1 >> STYLE_SHIFT;
                if style_id != last_style_id {
                    let transition = self.style_pool.transition(last_style_id, style_id);
                    output.push_str(transition);
                    last_style_id = style_id;
                }

                // Hyperlink transition
                let hid = (back_w1 >> 2) & 0x7fff;
                let new_hyperlink = if hid == 0 {
                    None
                } else {
                    self.hyperlink_pool.get(hid).map(|s| s.to_string())
                };
                if new_hyperlink != active_hyperlink {
                    match &new_hyperlink {
                        Some(url) => output.push_str(&osc::hyperlink(url, None)),
                        None => output.push_str(&osc::hyperlink_end()),
                    }
                    active_hyperlink = new_hyperlink;
                }

                // Write character
                let ch = self.char_pool.get(back_w0);
                if ch.is_empty() {
                    // Spacer char -- write space to clear stale content
                    output.push(' ');
                    cursor_col += 1;
                } else {
                    output.push_str(ch);
                    let advance = if width == CellWidth::Wide { 2 } else { 1 };
                    cursor_col += advance;
                }
            }
        }

        // Reset style at end
        if last_style_id != 0 {
            output.push_str("\x1b[0m");
        }
        if active_hyperlink.is_some() {
            output.push_str(&osc::hyperlink_end());
        }

        output
    }

    /// Swap front and back buffers. After diff_screens(), call this so that
    /// the front buffer reflects what was sent to the terminal.
    pub fn swap_buffers(&mut self) {
        std::mem::swap(&mut self.front, &mut self.back);
        self.back.reset(self.width as usize, self.height as usize);
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Collect all text content from a Text element's children (text nodes
/// and virtual text nodes).
fn collect_text_content(dom: &DomTree, node_id: NodeId) -> String {
    let elem = match dom.element(node_id) {
        Some(e) => e,
        None => return String::new(),
    };

    let mut result = String::new();
    for &child_id in &elem.children {
        match dom.get(child_id) {
            Some(DomNode::Text(t)) => {
                result.push_str(&t.value);
            }
            Some(DomNode::Element(child_elem))
                if child_elem.element_type == ElementType::VirtualText
                    || child_elem.element_type == ElementType::Link =>
            {
                result.push_str(&collect_text_content(dom, child_id));
            }
            _ => {}
        }
    }
    result
}

/// A styled text segment (text with associated styles).
#[allow(dead_code)]
struct StyledSegment {
    text: String,
    styles: Option<TextStyles>,
}

/// Collect styled segments from a Text element's children.
#[allow(dead_code)]
fn collect_styled_segments(dom: &DomTree, node_id: NodeId) -> Vec<StyledSegment> {
    let elem = match dom.element(node_id) {
        Some(e) => e,
        None => return Vec::new(),
    };

    let mut segments = Vec::new();
    for &child_id in &elem.children {
        match dom.get(child_id) {
            Some(DomNode::Text(t)) => {
                if !t.value.is_empty() {
                    segments.push(StyledSegment {
                        text: t.value.clone(),
                        styles: None, // Inherits from parent
                    });
                }
            }
            Some(DomNode::Element(child_elem))
                if child_elem.element_type == ElementType::VirtualText
                    || child_elem.element_type == ElementType::Link =>
            {
                // Collect text from this virtual text's children
                let sub_text = collect_text_content(dom, child_id);
                if !sub_text.is_empty() {
                    segments.push(StyledSegment {
                        text: sub_text,
                        styles: child_elem.text_styles.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    segments
}

/// Resolve a TextStyles into a runtime Style for interning.
fn resolve_text_styles(elem: &DomElement) -> Style {
    let ts = match &elem.text_styles {
        Some(ts) => ts,
        None => return Style::default(),
    };
    text_styles_to_style(ts)
}

/// Convert TextStyles to a runtime Style.
fn text_styles_to_style(ts: &TextStyles) -> Style {
    Style {
        fg_color: ts.color.clone(),
        bg_color: ts.background_color.clone(),
        bold: ts.bold.unwrap_or(false),
        dim: ts.dim.unwrap_or(false),
        italic: ts.italic.unwrap_or(false),
        underline: ts.underline.unwrap_or(false),
        strikethrough: ts.strikethrough.unwrap_or(false),
        inverse: ts.inverse.unwrap_or(false),
        ..Style::default()
    }
}

/// Resolve a segment's style by merging parent and segment styles.
#[allow(dead_code)]
fn resolve_segment_style(parent_elem: &DomElement, segment: &StyledSegment) -> Style {
    let parent_style = resolve_text_styles(parent_elem);
    match &segment.styles {
        None => parent_style,
        Some(ts) => {
            // Segment styles override parent
            let mut style = parent_style;
            if let Some(ref c) = ts.color {
                style.fg_color = Some(c.clone());
            }
            if let Some(ref c) = ts.background_color {
                style.bg_color = Some(c.clone());
            }
            if let Some(b) = ts.bold {
                style.bold = b;
            }
            if let Some(d) = ts.dim {
                style.dim = d;
            }
            if let Some(i) = ts.italic {
                style.italic = i;
            }
            if let Some(u) = ts.underline {
                style.underline = u;
            }
            if let Some(s) = ts.strikethrough {
                style.strikethrough = s;
            }
            if let Some(inv) = ts.inverse {
                style.inverse = inv;
            }
            style
        }
    }
}

/// Get the border style from an element's attributes.
fn get_border_style(elem: &DomElement) -> BorderStyle {
    use crate::tui::dom::DomNodeAttribute;
    match elem.attributes.get("borderStyle") {
        Some(DomNodeAttribute::String(s)) => match s.as_str() {
            "single" => BorderStyle::Single,
            "double" => BorderStyle::Double,
            "round" => BorderStyle::Round,
            "heavy" => BorderStyle::Heavy,
            "ascii" => BorderStyle::Ascii,
            _ => BorderStyle::None,
        },
        _ => BorderStyle::None,
    }
}

/// Get the border color style for drawing.
fn get_border_color_style(elem: &DomElement) -> Style {
    use crate::tui::dom::DomNodeAttribute;
    use crate::tui::style::parse_color;
    match elem.attributes.get("borderColor") {
        Some(DomNodeAttribute::String(s)) => {
            if let Some(color) = parse_color(s) {
                Style {
                    fg_color: Some(color),
                    ..Style::default()
                }
            } else {
                Style::default()
            }
        }
        _ => Style::default(),
    }
}

/// Build a taffy Style from a DomElement, incorporating text measurement.
fn build_taffy_style(elem: &DomElement, _dom: &DomTree) -> taffy::Style {
    elem.layout_style.to_taffy_style()
}

/// Recursively clear layout_node references from all DOM elements.
/// Called before rebuilding the taffy tree to prevent stale node IDs.
fn clear_layout_nodes(dom: &mut DomTree, node_id: NodeId) {
    if let Some(elem) = dom.element_mut(node_id) {
        elem.layout_node = None;
        let children = elem.children.clone();
        for child_id in children {
            clear_layout_nodes(dom, child_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::cell::Cell;
    use crate::tui::layout::{Dimension, LayoutFlexDirection, LayoutStyle};

    #[test]
    fn test_renderer_new() {
        let renderer = Renderer::new(80, 24);
        assert_eq!(renderer.width, 80);
        assert_eq!(renderer.height, 24);
    }

    #[test]
    fn test_renderer_resize() {
        let mut renderer = Renderer::new(80, 24);
        renderer.resize(120, 40);
        assert_eq!(renderer.width, 120);
        assert_eq!(renderer.height, 40);
        assert_eq!(renderer.front.width, 120);
        assert_eq!(renderer.back.width, 120);
    }

    #[test]
    fn test_renderer_diff_empty() {
        let mut renderer = Renderer::new(10, 5);
        // No damage -> empty diff
        let diff = renderer.diff_screens();
        assert!(diff.is_empty());
    }

    #[test]
    fn test_renderer_diff_single_cell() {
        let mut renderer = Renderer::new(10, 5);
        // Write a cell to back buffer
        renderer.back.set_cell_at(
            0,
            0,
            &Cell {
                char: "A".into(),
                style_id: 0,
                width: CellWidth::Narrow,
                hyperlink: None,
            },
            &mut renderer.char_pool,
            &mut renderer.hyperlink_pool,
        );

        let diff = renderer.diff_screens();
        assert!(!diff.is_empty());
        assert!(diff.contains('A'));
    }

    #[test]
    fn test_renderer_swap_buffers() {
        let mut renderer = Renderer::new(10, 5);
        renderer.back.set_cell_at(
            0,
            0,
            &Cell {
                char: "A".into(),
                style_id: 0,
                width: CellWidth::Narrow,
                hyperlink: None,
            },
            &mut renderer.char_pool,
            &mut renderer.hyperlink_pool,
        );

        renderer.swap_buffers();
        // After swap, front has the content, back is reset
        assert!(!renderer.front.is_empty_at(0, 0));
        assert!(renderer.back.is_empty_at(0, 0));
    }

    #[test]
    fn test_renderer_full_pipeline() {
        let mut renderer = Renderer::new(40, 10);
        let mut dom = DomTree::new();

        // Build a simple DOM: root -> box -> text
        let root = dom.create_element(ElementType::Root);
        dom.set_style(
            root,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(40.0),
                height: Dimension::Points(10.0),
                ..LayoutStyle::default()
            },
        );

        let text_elem = dom.create_element(ElementType::Text);
        dom.set_style(
            text_elem,
            LayoutStyle {
                flex_shrink: 1.0,
                ..LayoutStyle::default()
            },
        );
        let text_node = dom.create_text_node("Hello, World!");
        dom.append_child(text_elem, text_node);
        dom.append_child(root, text_elem);

        // Run the full render pipeline
        let output = renderer.render(&mut dom, root);

        // The output should contain the text content.
        // It may be split across multiple cursor-positioned writes.
        assert!(output.contains("Hello,"), "output should contain 'Hello,': {}", output);
        assert!(output.contains("World!"), "output should contain 'World!': {}", output);
    }

    #[test]
    fn test_renderer_with_borders() {
        let mut renderer = Renderer::new(20, 5);
        let mut dom = DomTree::new();

        let root = dom.create_element(ElementType::Root);
        dom.set_style(
            root,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(20.0),
                height: Dimension::Points(5.0),
                ..LayoutStyle::default()
            },
        );

        let box_elem = dom.create_element(ElementType::Box);
        dom.set_style(
            box_elem,
            LayoutStyle {
                width: Dimension::Points(20.0),
                height: Dimension::Points(5.0),
                border: crate::tui::layout::Edges {
                    top: 1.0,
                    bottom: 1.0,
                    left: 1.0,
                    right: 1.0,
                },
                ..LayoutStyle::default()
            },
        );
        dom.set_attribute(
            box_elem,
            "borderStyle",
            crate::tui::dom::DomNodeAttribute::String("single".into()),
        );
        dom.append_child(root, box_elem);

        let output = renderer.render(&mut dom, root);
        // Should contain box-drawing characters
        assert!(
            output.contains('\u{250C}') || output.contains('\u{2500}'),
            "output should contain border chars: {}",
            output
        );
    }

    #[test]
    fn test_clip_rect_intersect() {
        let a = ClipRect {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
        };
        let b = ClipRect {
            x1: 5,
            y1: 5,
            x2: 15,
            y2: 15,
        };
        let c = a.intersect(b);
        assert_eq!(c.x1, 5);
        assert_eq!(c.y1, 5);
        assert_eq!(c.x2, 10);
        assert_eq!(c.y2, 10);
    }

    #[test]
    fn test_clip_rect_invalid() {
        let a = ClipRect {
            x1: 10,
            y1: 10,
            x2: 5,
            y2: 5,
        };
        assert!(!a.is_valid());
    }

    #[test]
    fn test_collect_text_content() {
        let mut dom = DomTree::new();
        let text_elem = dom.create_element(ElementType::Text);
        let t1 = dom.create_text_node("hello ");
        let t2 = dom.create_text_node("world");
        dom.append_child(text_elem, t1);
        dom.append_child(text_elem, t2);

        let content = collect_text_content(&dom, text_elem);
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_incremental_diff() {
        let mut renderer = Renderer::new(10, 5);
        let mut dom = DomTree::new();

        // First render
        let root = dom.create_element(ElementType::Root);
        dom.set_style(
            root,
            LayoutStyle {
                width: Dimension::Points(10.0),
                height: Dimension::Points(5.0),
                ..LayoutStyle::default()
            },
        );
        let text_elem = dom.create_element(ElementType::Text);
        let text_node = dom.create_text_node("AAA");
        dom.append_child(text_elem, text_node);
        dom.append_child(root, text_elem);

        let _first = renderer.render(&mut dom, root);

        // Second render: change text
        dom.set_text_value(text_node, "BBB");
        dom.mark_dirty(root);
        let second = renderer.render(&mut dom, root);

        // Should contain "BBB" in the diff
        assert!(second.contains("BBB"), "incremental diff: {}", second);
    }

    #[test]
    fn test_text_with_ansi_escapes() {
        // Text nodes with embedded ANSI codes should render the visible text,
        // not the raw escape bytes.
        let mut renderer = Renderer::new(40, 5);
        let mut dom = DomTree::new();

        let root = dom.create_element(ElementType::Root);
        dom.set_style(
            root,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(40.0),
                height: Dimension::Points(5.0),
                ..LayoutStyle::default()
            },
        );

        let text_elem = dom.create_element(ElementType::Text);
        dom.set_style(
            text_elem,
            LayoutStyle {
                flex_shrink: 1.0,
                ..LayoutStyle::default()
            },
        );
        // This mimics welcome banner lines with ANSI codes
        let text_node = dom.create_text_node("\x1b[1;36mThunderCode\x1b[0m v1.0");
        dom.append_child(text_elem, text_node);
        dom.append_child(root, text_elem);

        let output = renderer.render(&mut dom, root);

        // Should contain the visible text "ThunderCode" and "v1.0" but NOT raw "[1;36m"
        assert!(
            output.contains("ThunderCode"),
            "should contain 'ThunderCode': {}",
            output
        );
        assert!(
            output.contains("v1.0"),
            "should contain 'v1.0': {}",
            output
        );
        // The literal "[1;36m" should NOT appear as visible text
        assert!(
            !output.contains("[1;36m"),
            "should NOT contain literal '[1;36m': {}",
            output
        );
    }

    #[test]
    fn test_multiline_text_layout() {
        // Multiple text children in a column should each get their own row.
        let mut renderer = Renderer::new(40, 10);
        let mut dom = DomTree::new();

        let root = dom.create_element(ElementType::Root);
        dom.set_style(
            root,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(40.0),
                height: Dimension::Points(10.0),
                ..LayoutStyle::default()
            },
        );

        // Line 1
        let text1 = dom.create_element(ElementType::Text);
        dom.set_style(text1, LayoutStyle { flex_shrink: 1.0, ..LayoutStyle::default() });
        let t1 = dom.create_text_node("Line-one");
        dom.append_child(text1, t1);
        dom.append_child(root, text1);

        // Line 2
        let text2 = dom.create_element(ElementType::Text);
        dom.set_style(text2, LayoutStyle { flex_shrink: 1.0, ..LayoutStyle::default() });
        let t2 = dom.create_text_node("Line-two");
        dom.append_child(text2, t2);
        dom.append_child(root, text2);

        // Line 3
        let text3 = dom.create_element(ElementType::Text);
        dom.set_style(text3, LayoutStyle { flex_shrink: 1.0, ..LayoutStyle::default() });
        let t3 = dom.create_text_node("Line-three");
        dom.append_child(text3, t3);
        dom.append_child(root, text3);

        let output = renderer.render(&mut dom, root);

        // The diff output may split words across cursor-positioning escapes
        // because unchanged cells (spaces) are skipped. Check that text
        // fragments appear in the output.
        assert!(output.contains("Line-one"), "should contain 'Line-one': {}", output);
        assert!(output.contains("Line-two"), "should contain 'Line-two': {}", output);
        assert!(output.contains("Line-three"), "should contain 'Line-three': {}", output);

        // Each line should be on a different row (check cursor positions)
        // Row 1 for Line-one, Row 2 for Line-two, Row 3 for Line-three
        assert!(output.contains("[1;1H"), "Line-one should start at row 1: {}", output);
        assert!(output.contains("[2;1H"), "Line-two should start at row 2: {}", output);
        assert!(output.contains("[3;1H"), "Line-three should start at row 3: {}", output);
    }

    #[test]
    fn test_welcome_banner_style_rendering() {
        // Simulate the welcome banner: column of Text elements, some with ANSI codes.
        let mut renderer = Renderer::new(80, 24);
        let mut dom = DomTree::new();

        let root = dom.create_element(ElementType::Root);
        dom.set_style(
            root,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(80.0),
                height: Dimension::Points(24.0),
                ..LayoutStyle::default()
            },
        );

        // Messages area box
        let messages = dom.create_element(ElementType::Box);
        dom.set_style(
            messages,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..LayoutStyle::default()
            },
        );
        dom.append_child(root, messages);

        // Welcome lines (mimicking App::set_welcome)
        let lines = vec![
            "",
            "  \x1b[1;36mThunderCode\x1b[0m \x1b[2mv0.1.0\x1b[0m",
            "",
            "  \x1b[2mModel:\x1b[0m    test-model",
            "  \x1b[2mTools:\x1b[0m    5 registered",
        ];
        for line in &lines {
            let text_elem = dom.create_element(ElementType::Text);
            dom.set_style(
                text_elem,
                LayoutStyle {
                    flex_direction: crate::tui::layout::LayoutFlexDirection::Row,
                    flex_shrink: 1.0,
                    ..LayoutStyle::default()
                },
            );
            let text_node = dom.create_text_node(line);
            dom.append_child(text_elem, text_node);
            dom.append_child(messages, text_elem);
        }

        let output = renderer.render(&mut dom, root);

        // Visible text should appear in the output. Note: the diff output
        // may split words around spaces because unchanged cells are skipped.
        assert!(output.contains("ThunderCode"), "should contain 'ThunderCode': {}", output);
        assert!(output.contains("test-model"), "should contain 'test-model': {}", output);
        assert!(output.contains("registered"), "should contain 'registered': {}", output);
    }

    #[test]
    fn test_back_buffer_has_content_after_render() {
        // After render, the front buffer (swapped from back) should have non-empty cells
        let mut renderer = Renderer::new(80, 24);
        let mut dom = DomTree::new();

        let root = dom.create_element(ElementType::Root);
        dom.set_style(
            root,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(80.0),
                height: Dimension::Points(24.0),
                ..LayoutStyle::default()
            },
        );

        let text_elem = dom.create_element(ElementType::Text);
        dom.set_style(text_elem, LayoutStyle { flex_shrink: 1.0, ..LayoutStyle::default() });
        let text_node = dom.create_text_node("Visible content");
        dom.append_child(text_elem, text_node);
        dom.append_child(root, text_elem);

        let _output = renderer.render(&mut dom, root);

        // After render, front buffer = what was the back buffer
        // At least some cells on row 0 should be non-empty
        let mut found_content = false;
        for x in 0..80 {
            if !renderer.front.is_empty_at(x, 0) {
                found_content = true;
                break;
            }
        }
        assert!(found_content, "front buffer should have non-empty cells on row 0 after rendering text");
    }

    #[test]
    fn test_box_with_padding_does_not_double_count() {
        // A Box with padding should not cause its children to be offset twice
        let mut renderer = Renderer::new(40, 10);
        let mut dom = DomTree::new();

        let root = dom.create_element(ElementType::Root);
        dom.set_style(
            root,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(40.0),
                height: Dimension::Points(10.0),
                ..LayoutStyle::default()
            },
        );

        let padded_box = dom.create_element(ElementType::Box);
        dom.set_style(
            padded_box,
            LayoutStyle {
                flex_direction: LayoutFlexDirection::Column,
                width: Dimension::Points(40.0),
                padding: crate::tui::layout::Edges {
                    top: 2.0,
                    left: 4.0,
                    ..crate::tui::layout::Edges::default()
                },
                ..LayoutStyle::default()
            },
        );
        dom.append_child(root, padded_box);

        let text_elem = dom.create_element(ElementType::Text);
        dom.set_style(text_elem, LayoutStyle { flex_shrink: 1.0, ..LayoutStyle::default() });
        let text_node = dom.create_text_node("padded");
        dom.append_child(text_elem, text_node);
        dom.append_child(padded_box, text_elem);

        let _output = renderer.render(&mut dom, root);

        // The text "padded" should appear at row 2 (padding.top=2), col 4 (padding.left=4)
        // Check that the front buffer has 'p' at approximately (4, 2)
        let cell = renderer.front.cell_at(4, 2, &renderer.char_pool, &renderer.hyperlink_pool);
        assert_eq!(
            cell.map(|c| c.char.clone()),
            Some("p".to_string()),
            "text should be at (4, 2) with padding, not double-offset"
        );

        // Also verify it is NOT at (8, 4) which would indicate double-counting
        let cell_double = renderer.front.cell_at(8, 4, &renderer.char_pool, &renderer.hyperlink_pool);
        assert_ne!(
            cell_double.map(|c| c.char.clone()),
            Some("p".to_string()),
            "text should NOT be at (8, 4) -- that would be double padding"
        );
    }
}
