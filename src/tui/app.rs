//! App render state -- drives the TUI render loop.
//!
//! Owns the UI state (messages, spinner, prompt) and renders the full screen
//! as a flat list of ANSI-styled lines. No DOM/taffy pipeline -- just direct
//! line-by-line terminal output for simplicity and reliability.
//!
//! Screen layout (from bottom):
//!   Row H  : status footer (dimmed, right-aligned)
//!   Row H-1: prompt border bottom (gray ─)
//!   Row H-2: prompt input (❯ text)
//!   Row H-3: prompt border top (gray ─)
//!   Row H-4: spinner (if active) or permission dialog
//!   Rows 1..N: scrollable message area (transcript)

use std::io::{self, Write};
use std::time::Instant;

use crate::tui::components::spinner_widget::SpinnerWidget;
use crate::tui::components::status_line::{StatusLine, StatusLineData};
use crate::tui::components::prompt_input::PromptInput;
use crate::tui::renderer::Renderer;
use crate::tui::termio::{csi, dec};

/// Permission dialog display data (lightweight, for the render loop).
#[derive(Debug, Clone)]
pub struct PermissionDialogData {
    pub tool_name: String,
    pub description: String,
    pub response: Option<bool>,
}

impl PermissionDialogData {
    pub fn new(tool_name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            description: description.into(),
            response: None,
        }
    }

    pub fn is_pending(&self) -> bool {
        self.response.is_none()
    }

    pub fn respond(&mut self, allow: bool) {
        self.response = Some(allow);
    }

    /// Render the permission dialog as ANSI strings.
    ///
    /// Uses the ref's `permission` color rgb(177,185,249) for borders
    /// and `success` rgb(78,186,101) / `error` rgb(255,107,128) for buttons.
    pub fn render_lines(&self, width: usize) -> Vec<String> {
        let border_color = "\x1b[38;2;177;185;249m";
        let reset = "\x1b[0m";
        let bold = "\x1b[1m";
        let dim = "\x1b[2m";
        let green = "\x1b[38;2;78;186;101m";
        let red = "\x1b[38;2;255;107;128m";

        let inner_width = width.saturating_sub(4);
        let h_border: String = "\u{2500}".repeat(inner_width);

        let mut lines = Vec::new();
        lines.push(format!("{border_color}\u{256d}{h_border}\u{256e}{reset}"));
        lines.push(format!(
            "{border_color}\u{2502}{reset} {bold}Allow {}{reset}?",
            self.tool_name
        ));
        if !self.description.is_empty() {
            let desc = if self.description.len() > inner_width.saturating_sub(4) {
                format!("{}...", &self.description[..inner_width.saturating_sub(7)])
            } else {
                self.description.clone()
            };
            lines.push(format!(
                "{border_color}\u{2502}{reset} {dim}{desc}{reset}"
            ));
        }
        lines.push(format!("{border_color}\u{2502}{reset}"));
        lines.push(format!(
            "{border_color}\u{2502}{reset} {green}{bold}[y]{reset} Allow  {red}{bold}[n]{reset} Deny"
        ));
        lines.push(format!("{border_color}\u{2570}{h_border}\u{256f}{reset}"));
        lines
    }
}

/// A rendered message in the transcript.
#[derive(Debug, Clone)]
pub enum TranscriptEntry {
    User { text: String },
    AssistantText { text: String, is_streaming: bool },
    Thinking { text: Option<String> },
    ToolUse { tool_name: String, display_name: String, input_summary: String, in_progress: bool },
    ToolResult { tool_name: String, content: String, is_error: bool },
}

/// The main application render state.
pub struct App {
    /// The double-buffered renderer (kept for API compat; not used by line renderer).
    pub renderer: Renderer,
    /// Terminal width.
    pub width: u16,
    /// Terminal height.
    pub height: u16,
    /// Prompt input state.
    pub prompt: PromptInput,
    /// Spinner state (Some when active).
    pub spinner: Option<SpinnerWidget>,
    /// Permission dialog (Some when active).
    pub permission_dialog: Option<PermissionDialogData>,
    /// Conversation transcript entries.
    pub transcript: Vec<TranscriptEntry>,
    /// Scroll offset for the message area (lines from bottom).
    pub scroll_offset: i32,
    /// Whether to auto-scroll to bottom on new content.
    pub sticky_scroll: bool,
    /// Status line data.
    pub status: StatusLineData,
    /// Whether the welcome banner has been shown.
    pub welcome_shown: bool,
    /// Welcome banner content lines (cached after first render).
    welcome_lines: Vec<String>,
    /// Last render time for FPS throttling.
    last_render: Instant,
    /// Whether the screen needs a full redraw.
    pub needs_full_redraw: bool,
    /// Command suggestions (shown when typing /...)
    pub command_suggestions: Vec<(String, String)>, // (name, description)
    /// Selected suggestion index
    pub selected_suggestion: usize,
}

impl App {
    /// Create a new App with the given terminal dimensions.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            renderer: Renderer::new(width, height),
            width,
            height,
            prompt: PromptInput::new(),
            spinner: None,
            permission_dialog: None,
            transcript: Vec::new(),
            scroll_offset: 0,
            sticky_scroll: true,
            status: StatusLineData {
                model: String::new(),
                cost_usd: 0.0,
                input_tokens: 0,
                output_tokens: 0,
                session_info: None,
                permission_mode: None,
            },
            welcome_shown: false,
            welcome_lines: Vec::new(),
            last_render: Instant::now(),
            needs_full_redraw: true,
            command_suggestions: Vec::new(),
            selected_suggestion: 0,
        }
    }

    /// Update command suggestions based on current input.
    pub fn update_suggestions(&mut self, all_commands: &[(String, String)]) {
        let buf = self.prompt.buffer().to_string();
        if buf.starts_with('/') && !buf.contains(' ') {
            let prefix = &buf[1..]; // strip the /
            self.command_suggestions = all_commands
                .iter()
                .filter(|(name, _)| name.starts_with(prefix))
                .take(8)
                .cloned()
                .collect();
            self.selected_suggestion = 0;
        } else {
            self.command_suggestions.clear();
        }
    }

    /// Move suggestion selection up.
    pub fn suggestion_up(&mut self) {
        if !self.command_suggestions.is_empty() && self.selected_suggestion > 0 {
            self.selected_suggestion -= 1;
        }
    }

    /// Move suggestion selection down.
    pub fn suggestion_down(&mut self) {
        if self.selected_suggestion + 1 < self.command_suggestions.len() {
            self.selected_suggestion += 1;
        }
    }

    /// Accept the current suggestion (fill the prompt with the command).
    pub fn accept_suggestion(&mut self) -> bool {
        if let Some((name, _)) = self.command_suggestions.get(self.selected_suggestion) {
            let cmd = format!("/{name}");
            self.prompt.clear();
            self.prompt.insert_str(&cmd);
            self.command_suggestions.clear();
            true
        } else {
            false
        }
    }

    /// Resize the terminal.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.renderer.resize(width, height);
        self.needs_full_redraw = true;
    }

    /// Set the welcome state.
    ///
    /// The ref shows NO big banner -- just a blank line then the prompt.
    /// All "welcome" info (model, etc.) lives in the footer/status area.
    pub fn set_welcome(&mut self, model: &str, _tool_count: usize, _command_count: usize) {
        self.welcome_lines.clear();
        self.welcome_lines.push(String::new());
        self.welcome_shown = true;
        self.status.model = model.to_string();
    }

    /// Start a spinner with default verb.
    pub fn start_spinner(&mut self) {
        self.spinner = Some(SpinnerWidget::new());
    }

    /// Stop the spinner.
    pub fn stop_spinner(&mut self) {
        self.spinner = None;
    }

    /// Tick the spinner (advance one frame).
    pub fn tick_spinner(&mut self) {
        if let Some(ref mut s) = self.spinner {
            s.tick();
        }
    }

    // -----------------------------------------------------------------------
    // Layout helpers -- compute consistent row positions
    // -----------------------------------------------------------------------

    /// Number of fixed rows at the bottom: top_border + input + bottom_border + status = 4.
    const FIXED_BOTTOM: usize = 4;

    /// Number of rows used by the middle area (spinner or permission dialog).
    fn middle_area_rows(&self) -> usize {
        if self.spinner.is_some() {
            1
        } else if self.permission_dialog.is_some() {
            6
        } else {
            0
        }
    }

    /// Height available for the message area.
    fn message_area_height(&self) -> usize {
        let h = self.height as usize;
        h.saturating_sub(Self::FIXED_BOTTOM + self.middle_area_rows())
    }

    /// 1-based row of the prompt input line.
    /// From bottom: status(row H), bottom_border(H-1), input(H-2), top_border(H-3).
    /// So input is at row H-2, which is (h - 2) in 1-based.
    #[allow(dead_code)]
    fn prompt_input_row(&self) -> usize {
        let h = self.height as usize;
        h.saturating_sub(2)
    }

    /// 1-based row where the spinner sits (just above the prompt top border).
    #[allow(dead_code)]
    fn spinner_row(&self) -> usize {
        let h = self.height as usize;
        // top_border is at h-3, spinner is at h-3-1 = h-4
        h.saturating_sub(Self::FIXED_BOTTOM)
    }

    // -----------------------------------------------------------------------
    // Line-based render pipeline
    // -----------------------------------------------------------------------

    /// Render the full screen using direct line-based ANSI output.
    ///
    /// Layout (from bottom, 1-based rows):
    ///   Row H  : status line (dimmed, right-aligned)
    ///   Row H-1: prompt border bottom (gray ─)
    ///   Row H-2: prompt input (❯ text)
    ///   Row H-3: prompt border top (gray ─)
    ///   [Row H-4: spinner or permission dialog, if active]
    ///   Rows 1..N: scrollable message area (transcript)
    pub fn render(&mut self) {
        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        let mut output = String::with_capacity(w * h * 4);

        // Hide cursor during render to prevent flicker
        output.push_str(&dec::hide_cursor());

        // Clear entire screen on first render or resize
        if self.needs_full_redraw {
            output.push_str("\x1b[2J");
        }

        // -- 1. Build message lines and render the message area --
        let msg_lines = self.build_message_lines(w);
        let msg_area_h = self.message_area_height();
        let total_msg = msg_lines.len();

        // Sticky scroll: when at bottom, stay at bottom as new content arrives
        if self.sticky_scroll {
            self.scroll_offset = 0;
        }

        // Clamp scroll_offset to valid range
        let max_scroll = if total_msg > msg_area_h {
            (total_msg - msg_area_h) as i32
        } else {
            0
        };
        self.scroll_offset = self.scroll_offset.clamp(0, max_scroll);

        // Scroll: show the latest messages (auto-scroll to bottom when offset=0)
        let visible_start = if total_msg > msg_area_h {
            (total_msg - msg_area_h) as i32 - self.scroll_offset
        } else {
            0
        } as usize;

        for row_idx in 0..msg_area_h {
            let line_idx = visible_start + row_idx;
            let term_row = row_idx + 1; // 1-based
            output.push_str(&csi::cursor_position(term_row as u32, 1));
            output.push_str(csi::ERASE_LINE);
            if line_idx < total_msg {
                output.push_str(&msg_lines[line_idx]);
            }
        }

        // -- 2. Middle area: spinner or permission dialog --
        let middle_start = msg_area_h + 1; // 1-based row
        if let Some(ref spinner) = self.spinner {
            output.push_str(&csi::cursor_position(middle_start as u32, 1));
            output.push_str(csi::ERASE_LINE);
            output.push_str(&Self::format_spinner_line(spinner));
        } else if let Some(ref dialog) = self.permission_dialog {
            let dialog_lines = dialog.render_lines(w);
            for (i, line) in dialog_lines.iter().enumerate() {
                let term_row = middle_start + i;
                if term_row <= h {
                    output.push_str(&csi::cursor_position(term_row as u32, 1));
                    output.push_str(csi::ERASE_LINE);
                    output.push_str(line);
                }
            }
        }

        // -- 2.5. Command suggestions (above prompt border) --
        if !self.command_suggestions.is_empty() {
            let max_suggestions = 8.min(self.command_suggestions.len());
            let suggestion_color = "\x1b[38;2;177;185;249m"; // suggestion blue
            let selected_bg = "\x1b[48;2;38;79;120m"; // selectionBg
            let dim = "\x1b[2m";
            let reset = "\x1b[0m";
            let suggestions_start = h.saturating_sub(Self::FIXED_BOTTOM + self.middle_area_rows() + max_suggestions);
            for (i, (name, desc)) in self.command_suggestions.iter().take(max_suggestions).enumerate() {
                let row = suggestions_start + i;
                output.push_str(&csi::cursor_position(row as u32, 1));
                output.push_str(csi::ERASE_LINE);
                if i == self.selected_suggestion {
                    output.push_str(&format!(
                        "{selected_bg}  {suggestion_color}/{name}{reset}{selected_bg} {dim}{desc}{reset}"
                    ));
                } else {
                    output.push_str(&format!(
                        "  {suggestion_color}/{name}{reset} {dim}{desc}{reset}"
                    ));
                }
            }
        }

        // -- 3. Prompt area: top_border, input, bottom_border --
        let border = Self::format_border(w);

        // Top border: row H-3
        let top_border_row = h.saturating_sub(3);
        output.push_str(&csi::cursor_position(top_border_row as u32, 1));
        output.push_str(csi::ERASE_LINE);
        output.push_str(&border);

        // Prompt input: row H-2
        let input_row = h.saturating_sub(2);
        let (prompt_line, _) = self.prompt.render();
        output.push_str(&csi::cursor_position(input_row as u32, 1));
        output.push_str(csi::ERASE_LINE);
        output.push_str(&prompt_line);

        // Bottom border: row H-1
        let bottom_border_row = h.saturating_sub(1);
        output.push_str(&csi::cursor_position(bottom_border_row as u32, 1));
        output.push_str(csi::ERASE_LINE);
        output.push_str(&border);

        // -- 4. Status footer: row H --
        let status_line = StatusLine::render_ansi(&self.status, w);
        output.push_str(&csi::cursor_position(h as u32, 1));
        output.push_str(csi::ERASE_LINE);
        output.push_str(&status_line);

        // -- 5. Position cursor in prompt for typing --
        if self.prompt.is_active() && self.spinner.is_none() && self.permission_dialog.is_none() {
            // "❯ " prefix = 2 display columns, then cursor position within input
            let cursor_col = 2 + self.prompt.cursor_display_col() + 1; // +1 for 1-based
            output.push_str(&csi::cursor_position(input_row as u32, cursor_col as u32));
            output.push_str(&dec::show_cursor());
        }

        // -- 6. Flush to terminal --
        let mut stdout = io::stdout();
        let _ = stdout.write_all(output.as_bytes());
        let _ = stdout.flush();

        self.last_render = Instant::now();
        self.needs_full_redraw = false;
    }

    /// Quick render of just the prompt input line (for typing responsiveness).
    pub fn render_prompt_only(&mut self) {
        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        // Prompt input is always at row H-2
        let input_row = h.saturating_sub(2);
        let (prompt_line, _) = self.prompt.render();

        let mut output = String::new();
        output.push_str(&csi::cursor_position(input_row as u32, 1));
        output.push_str(csi::ERASE_LINE);
        output.push_str(&prompt_line);

        // Restore cursor position
        let cursor_col = 2 + self.prompt.cursor_display_col() + 1;
        output.push_str(&csi::cursor_position(input_row as u32, cursor_col as u32));
        output.push_str(&dec::show_cursor());

        let mut stdout = io::stdout();
        let _ = stdout.write_all(output.as_bytes());
        let _ = stdout.flush();
    }

    /// Render just the spinner line (for animation ticks).
    pub fn render_spinner_only(&mut self) {
        if self.spinner.is_none() {
            return;
        }
        let h = self.height as usize;
        if h == 0 {
            return;
        }

        // Spinner is at row H-4 (one above the prompt top border at H-3)
        let spinner_row = h.saturating_sub(4);
        if spinner_row == 0 {
            return;
        }

        if let Some(ref spinner) = self.spinner {
            let mut output = String::new();
            output.push_str(&dec::hide_cursor());
            output.push_str(&csi::cursor_position(spinner_row as u32, 1));
            output.push_str(csi::ERASE_LINE);
            output.push_str(&Self::format_spinner_line(spinner));

            let mut stdout = io::stdout();
            let _ = stdout.write_all(output.as_bytes());
            let _ = stdout.flush();
        }
    }

    // -----------------------------------------------------------------------
    // Line building helpers
    // -----------------------------------------------------------------------

    /// Build all message area lines from the transcript.
    /// Returns a Vec<String> where each string is one ANSI-styled terminal line.
    fn build_message_lines(&self, _width: usize) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();

        // Welcome: single blank line for breathing room
        if self.welcome_shown {
            for wl in &self.welcome_lines {
                lines.push(wl.clone());
            }
        }

        // ANSI escape constants
        let _subtle = "\x1b[38;2;80;80;80m";       // ❯ color for user messages
        let dim = "\x1b[2m";
        let bold = "\x1b[1m";
        let reset = "\x1b[0m";
        let success = "\x1b[38;2;78;186;101m";     // green ⏺ for completed tools
        let error_color = "\x1b[38;2;255;107;128m"; // red for error results
        let user_bg = "\x1b[48;2;55;55;55m";       // user message background
        let user_arrow = "\x1b[38;2;177;185;249m"; // blue-purple arrow in user msg
        let white = "\x1b[38;2;255;255;255m";      // white text

        // ⎿ prefix for assistant/tool results: "  ⎿  " (2 spaces, glyph, 2 spaces)
        let response_prefix = format!("  {dim}\u{23BF}{reset}  ");

        let mut prev_was_user = false;
        let mut prev_was_tool_result = false;
        for entry in &self.transcript {
            match entry {
                TranscriptEntry::User { text } => {
                    if !lines.is_empty() {
                        lines.push(String::new());
                    }
                    lines.push(format!(
                        "{user_bg}  {user_arrow}\u{276F}{reset}{user_bg} {white}{text}{reset}"
                    ));
                    prev_was_user = true;
                    prev_was_tool_result = false;
                }

                TranscriptEntry::AssistantText { text, is_streaming } => {
                    // Blank line before assistant text if preceded by user or tool result
                    if prev_was_user || prev_was_tool_result {
                        lines.push(String::new());
                    }
                    prev_was_user = false;
                    prev_was_tool_result = false;
                    if text.is_empty() && *is_streaming {
                        continue;
                    }
                    if text.is_empty() {
                        lines.push(response_prefix.clone());
                        continue;
                    }
                    // Render markdown with syntax highlighting
                    let md_lines = crate::tui::markdown::render_markdown(text, _width.saturating_sub(5));
                    for (i, styled_line) in md_lines.iter().enumerate() {
                        let ansi = styled_line.to_ansi();
                        if i == 0 {
                            lines.push(format!("{response_prefix}{ansi}"));
                        } else {
                            lines.push(format!("     {ansi}"));
                        }
                    }
                    // Show streaming cursor
                    if *is_streaming {
                        if let Some(last) = lines.last_mut() {
                            last.push_str("\x1b[7m \x1b[0m"); // inverse space = block cursor
                        }
                    }
                }

                TranscriptEntry::Thinking { .. } => {
                    prev_was_user = false;
                    prev_was_tool_result = false;
                }

                TranscriptEntry::ToolUse {
                    display_name,
                    input_summary,
                    in_progress,
                    ..
                } => {
                    // Blank line before each tool use block (except at start)
                    if !lines.is_empty() && !prev_was_user {
                        lines.push(String::new());
                    }
                    prev_was_user = false;
                    prev_was_tool_result = false;
                    let dot = if *in_progress {
                        format!("{dim}\u{23FA}{reset}")
                    } else {
                        format!("{success}\u{23FA}{reset}")
                    };
                    if input_summary.is_empty() {
                        lines.push(format!("  {dot} {bold}{display_name}{reset}"));
                    } else {
                        lines.push(format!(
                            "  {dot} {bold}{display_name}{reset} ({input_summary})"
                        ));
                    }
                }

                TranscriptEntry::ToolResult { tool_name, content, is_error } => {
                    prev_was_user = false;
                    prev_was_tool_result = true;

                    if *is_error {
                        let first_line = content.lines().next().unwrap_or(content);
                        lines.push(format!(
                            "{response_prefix}{error_color}{first_line}{reset}"
                        ));
                    } else {
                        // Condensed rendering per tool type (matching ref)
                        let total_lines = content.lines().count();
                        let tn = tool_name.as_str();

                        match tn {
                            // Read/Write: not shown in condensed mode (ref returns null)
                            "Read" | "file_read" | "Write" | "file_write" => {
                                // Silent — ref doesn't render result for read/write in condensed
                            }

                            // Edit: brief diff summary
                            "Edit" | "file_edit" => {
                                let is_diff = content.starts_with("---") || content.starts_with("diff ");
                                if is_diff {
                                    let added = content.lines().filter(|l| l.starts_with('+')).count().saturating_sub(1);
                                    let removed = content.lines().filter(|l| l.starts_with('-')).count().saturating_sub(1);
                                    lines.push(format!(
                                        "{response_prefix}{dim}{added} lines added, {removed} lines removed{reset}"
                                    ));
                                } else {
                                    lines.push(format!("{response_prefix}{dim}Applied edit{reset}"));
                                }
                            }

                            // Bash: show first few lines of output
                            "Bash" | "bash" => {
                                let max_lines = 3;
                                for (i, line) in content.lines().take(max_lines).enumerate() {
                                    let truncated = if line.len() > 120 {
                                        format!("{}…", &line[..117])
                                    } else {
                                        line.to_string()
                                    };
                                    if i == 0 {
                                        lines.push(format!("{response_prefix}{truncated}"));
                                    } else {
                                        lines.push(format!("     {truncated}"));
                                    }
                                }
                                if total_lines > max_lines {
                                    lines.push(format!(
                                        "     {dim}… +{} lines{reset}",
                                        total_lines - max_lines
                                    ));
                                }
                            }

                            // Grep/Glob: "Found X files" or "Found X lines"
                            "Grep" | "grep" | "Glob" | "glob" => {
                                let count = total_lines.saturating_sub(1).max(1); // rough count
                                let unit = if tn == "Glob" || tn == "glob" { "files" } else { "results" };
                                lines.push(format!(
                                    "{response_prefix}{dim}Found {count} {unit}{reset}"
                                ));
                            }

                            // Agent: show brief result
                            "Agent" | "agent" => {
                                let first = content.lines().next().unwrap_or("Completed");
                                let truncated = if first.len() > 100 {
                                    format!("{}…", &first[..97])
                                } else {
                                    first.to_string()
                                };
                                lines.push(format!("{response_prefix}{truncated}"));
                            }

                            // Default: show first line + count
                            _ => {
                                if total_lines <= 3 && content.len() < 200 {
                                    for (i, line) in content.lines().enumerate() {
                                        if i == 0 {
                                            lines.push(format!("{response_prefix}{line}"));
                                        } else {
                                            lines.push(format!("     {line}"));
                                        }
                                    }
                                } else {
                                    let first = content.lines().next().unwrap_or("");
                                    let truncated = if first.len() > 80 {
                                        format!("{}…", &first[..77])
                                    } else {
                                        first.to_string()
                                    };
                                    if total_lines > 1 {
                                        lines.push(format!(
                                            "{response_prefix}{truncated} {dim}({total_lines} lines){reset}"
                                        ));
                                    } else {
                                        lines.push(format!("{response_prefix}{truncated}"));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        lines
    }

    /// Format a spinner line: "  ⠋ Thinking..."
    /// Color: rgb(177,185,249) blue-purple, matching ref dark theme.
    fn format_spinner_line(spinner: &SpinnerWidget) -> String {
        let color = "\x1b[38;2;177;185;249m";
        let reset = "\x1b[0m";
        let frame = spinner.current_char();
        let msg = spinner.message();
        format!("  {color}{frame}{reset} {color}{msg}{reset}")
    }

    /// Format a horizontal border line in gray rgb(136,136,136).
    fn format_border(width: usize) -> String {
        let color = "\x1b[38;2;136;136;136m";
        let reset = "\x1b[0m";
        let bar: String = "\u{2500}".repeat(width);
        format!("{color}{bar}{reset}")
    }

    /// Render the full screen to a string (for testing).
    /// Builds the same content as render() but returns it instead of writing to stdout.
    #[cfg(test)]
    pub fn render_to_string(&mut self) -> String {
        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return String::new();
        }

        let mut output = String::new();

        // Message area
        let msg_lines = self.build_message_lines(w);
        let msg_area_h = self.message_area_height();
        let total_msg = msg_lines.len();
        let visible_start = if total_msg > msg_area_h {
            total_msg - msg_area_h
        } else {
            0
        };

        for row_idx in 0..msg_area_h {
            let line_idx = visible_start + row_idx;
            if line_idx < total_msg {
                output.push_str(&msg_lines[line_idx]);
            }
            output.push('\n');
        }

        // Spinner
        if let Some(ref spinner) = self.spinner {
            output.push_str(&Self::format_spinner_line(spinner));
            output.push('\n');
        } else if let Some(ref dialog) = self.permission_dialog {
            for line in dialog.render_lines(w) {
                output.push_str(&line);
                output.push('\n');
            }
        }

        // Prompt area
        let border = Self::format_border(w);
        output.push_str(&border);
        output.push('\n');

        let (prompt_line, _) = self.prompt.render();
        output.push_str(&prompt_line);
        output.push('\n');

        output.push_str(&border);
        output.push('\n');

        // Status line
        let status_line = StatusLine::render_ansi(&self.status, w);
        output.push_str(&status_line);
        output.push('\n');

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_render_produces_visible_output() {
        let mut app = App::new(80, 24);
        app.set_welcome("test-model", 10, 5);
        let output = app.render_to_string();

        // Output should not be empty
        assert!(!output.is_empty(), "output should not be empty after welcome");

        // The ANSI output should contain the model name in the status line
        assert!(
            output.contains("test-model"),
            "output should contain 'test-model': {}",
            output
        );
    }

    #[test]
    fn test_app_render_with_transcript() {
        let mut app = App::new(80, 24);
        app.set_welcome("test-model", 5, 3);

        // Add a user message
        app.transcript.push(TranscriptEntry::User {
            text: "hello-world".into(),
        });

        let output = app.render_to_string();

        // Should contain the user's message
        assert!(
            output.contains("hello-world"),
            "output should contain 'hello-world': {}",
            output
        );
    }

    #[test]
    fn test_app_render_with_assistant_text() {
        let mut app = App::new(80, 24);

        // Add assistant text
        app.transcript.push(TranscriptEntry::AssistantText {
            text: "Response-from-assistant".into(),
            is_streaming: false,
        });

        let output = app.render_to_string();

        assert!(
            output.contains("Response-from-assistant"),
            "output should contain assistant text: {}",
            output
        );
    }

    #[test]
    fn test_app_render_prompt() {
        let mut app = App::new(80, 24);
        app.prompt.set_active(true);

        let output = app.render_to_string();

        // The prompt prefix "❯" should be visible
        assert!(
            output.contains("\u{276F}"),
            "output should contain prompt prefix ❯: {}",
            output
        );
    }

    #[test]
    fn test_app_render_status_line() {
        let mut app = App::new(80, 24);
        app.status.model = "primary-test".into();

        let output = app.render_to_string();

        assert!(
            output.contains("primary-test"),
            "output should contain model name: {}",
            output
        );
    }

    #[test]
    fn test_app_render_spinner() {
        let mut app = App::new(80, 24);
        app.start_spinner();

        let output = app.render_to_string();

        // Should render without panic and produce non-empty output
        assert!(!output.is_empty(), "output should not be empty with spinner active");
    }

    #[test]
    fn test_app_resize_then_render() {
        let mut app = App::new(80, 24);
        app.set_welcome("test-model", 5, 3);
        let _output1 = app.render_to_string();

        // Resize
        app.resize(120, 40);
        let output2 = app.render_to_string();

        // After resize, model name should still appear in the status line
        assert!(
            output2.contains("test-model"),
            "output should contain 'test-model' after resize: {}",
            output2
        );
    }

    #[test]
    fn test_layout_row_calculations() {
        let app = App::new(80, 24);
        // With 24 rows and no spinner:
        // status = row 24, bottom_border = row 23, input = row 22, top_border = row 21
        // message area = rows 1-20 = 20 rows
        assert_eq!(app.message_area_height(), 20);
        assert_eq!(app.prompt_input_row(), 22);
        assert_eq!(app.spinner_row(), 20);
    }

    #[test]
    fn test_layout_with_spinner() {
        let mut app = App::new(80, 24);
        app.start_spinner();
        // With spinner: message area shrinks by 1
        assert_eq!(app.message_area_height(), 19);
        assert_eq!(app.middle_area_rows(), 1);
    }

    #[test]
    fn test_layout_with_dialog() {
        let mut app = App::new(80, 24);
        app.permission_dialog = Some(PermissionDialogData::new("Read", "test.txt"));
        // With dialog (6 rows): message area shrinks by 6
        assert_eq!(app.message_area_height(), 14);
        assert_eq!(app.middle_area_rows(), 6);
    }

    #[test]
    fn test_border_format() {
        let border = App::format_border(10);
        assert!(border.contains("\u{2500}"), "border should contain ─ character");
        assert!(border.contains("136;136;136"), "border should use gray color");
    }

    #[test]
    fn test_message_lines_empty() {
        let app = App::new(80, 24);
        let lines = app.build_message_lines(80);
        assert!(lines.is_empty(), "no transcript should produce no lines");
    }

    #[test]
    fn test_message_lines_user() {
        let mut app = App::new(80, 24);
        app.transcript.push(TranscriptEntry::User {
            text: "test message".into(),
        });
        let lines = app.build_message_lines(80);
        // Should have blank line + user message = 2 lines
        assert_eq!(lines.len(), 2);
        assert!(lines[0].is_empty(), "first line should be blank");
        assert!(lines[1].contains("test message"), "second line should contain message");
        assert!(lines[1].contains("\u{276F}"), "should contain arrow glyph");
    }

    #[test]
    fn test_message_lines_tool() {
        let mut app = App::new(80, 24);
        app.transcript.push(TranscriptEntry::ToolUse {
            tool_name: "Read".into(),
            display_name: "Read".into(),
            input_summary: "src/main.rs".into(),
            in_progress: false,
        });
        let lines = app.build_message_lines(80);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Read"), "should contain tool name");
        assert!(lines[0].contains("src/main.rs"), "should contain input summary");
        assert!(lines[0].contains("\u{23FA}"), "should contain ⏺ dot");
    }

    #[test]
    fn test_permission_dialog_render() {
        let dialog = PermissionDialogData::new("Read", "src/main.rs");
        let lines = dialog.render_lines(80);
        assert!(lines.len() >= 5, "dialog should have at least 5 lines");
        assert!(lines.iter().any(|l| l.contains("Allow")));
        assert!(lines.iter().any(|l| l.contains("[y]")));
    }

    #[test]
    fn test_scrolling() {
        let mut app = App::new(80, 10); // small terminal
        // Add many messages to force scrolling
        for i in 0..20 {
            app.transcript.push(TranscriptEntry::User {
                text: format!("message-{}", i),
            });
        }
        let output = app.render_to_string();
        // Last messages should be visible, first messages should be scrolled away
        assert!(
            output.contains("message-19"),
            "latest message should be visible: {}",
            output
        );
    }
}
