//! Screen buffer with packed cell storage and damage tracking.
//!
//! The screen stores cells as 2 consecutive `u32` values in a flat `Vec<u32>`:
//! `[charId, packed(styleId|hyperlinkId|width)]` per cell. This avoids per-cell
//! heap objects and enables fast integer-based diffing.

use crate::tui::cell::{
    pack_word1, Cell, CellWidth, EMPTY_CHAR_INDEX, HYPERLINK_MASK, HYPERLINK_SHIFT, SPACER_CHAR_INDEX,
    STYLE_SHIFT,
};
use crate::tui::pools::{CharPool, HyperlinkPool, StylePool};

/// Axis-aligned rectangle for damage tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    /// Compute the union of two rectangles.
    pub fn union(a: &Rect, b: &Rect) -> Rect {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        let right = (a.x + a.width).max(b.x + b.width);
        let bottom = (a.y + a.height).max(b.y + b.height);
        Rect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }
}

/// Terminal screen buffer with packed cell storage.
pub struct Screen {
    pub width: usize,
    pub height: usize,
    /// Packed cells: 2 u32 per cell, `cells[ci]` = charId, `cells[ci+1]` = word1.
    pub cells: Vec<u32>,
    /// Per-row soft-wrap continuation marker. `soft_wrap[r] = N > 0` means row r
    /// is a word-wrap continuation of row r-1.
    pub soft_wrap: Vec<i32>,
    /// Per-cell noSelect bitmap (1 byte per cell). 1 = exclude from text selection.
    pub no_select: Vec<u8>,
    /// Bounding box of cells written during rendering. Used by diff to limit iteration.
    pub damage: Option<Rect>,
    /// Empty style ID for comparisons.
    pub empty_style_id: u32,
}

impl Screen {
    /// Create a new screen with given dimensions. All cells start empty.
    pub fn new(
        width: usize,
        height: usize,
        style_pool: &StylePool,
    ) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            cells: vec![0u32; size * 2],
            soft_wrap: vec![0i32; height],
            no_select: vec![0u8; size],
            damage: None,
            empty_style_id: style_pool.none,
        }
    }

    /// Reset the screen for reuse (double-buffer swap). Resizes if needed,
    /// clears all cells to empty.
    pub fn reset(&mut self, width: usize, height: usize) {
        let size = width * height;
        let needed = size * 2;
        if self.cells.len() < needed {
            self.cells.resize(needed, 0);
            self.no_select.resize(size, 0);
        }
        if self.soft_wrap.len() < height {
            self.soft_wrap.resize(height, 0);
        }
        // Clear
        self.cells[..needed].fill(0);
        self.no_select[..size].fill(0);
        self.soft_wrap[..height].fill(0);
        self.width = width;
        self.height = height;
        self.damage = None;
    }

    /// Get a cell view at the given position.
    pub fn cell_at(
        &self,
        x: usize,
        y: usize,
        char_pool: &CharPool,
        hyperlink_pool: &HyperlinkPool,
    ) -> Option<Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.cell_at_index(y * self.width + x, char_pool, hyperlink_pool))
    }

    /// Get a cell view by pre-computed array index.
    pub fn cell_at_index(
        &self,
        index: usize,
        char_pool: &CharPool,
        hyperlink_pool: &HyperlinkPool,
    ) -> Cell {
        let ci = index * 2;
        let word1 = self.cells[ci + 1];
        let hid = (word1 >> HYPERLINK_SHIFT) & HYPERLINK_MASK;
        Cell {
            char: char_pool.get(self.cells[ci]).into(),
            style_id: word1 >> STYLE_SHIFT,
            width: CellWidth::from_u32(word1),
            hyperlink: if hid == 0 {
                None
            } else {
                hyperlink_pool.get(hid).map(|s| s.into())
            },
        }
    }

    /// Check if a cell at (x, y) is empty (both words are 0).
    pub fn is_empty_at(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return true;
        }
        let ci = (y * self.width + x) * 2;
        self.cells[ci] == 0 && self.cells[ci + 1] == 0
    }

    /// Set a cell at (x, y). Handles wide character spacer creation and
    /// orphaned wide/spacer cleanup.
    pub fn set_cell_at(
        &mut self,
        x: usize,
        y: usize,
        cell: &Cell,
        char_pool: &mut CharPool,
        hyperlink_pool: &mut HyperlinkPool,
    ) {
        if x >= self.width || y >= self.height {
            return;
        }
        let ci = (y * self.width + x) * 2;

        // Clean up orphaned wide char / spacer when overwriting
        let prev_width = CellWidth::from_u32(self.cells[ci + 1]);

        // Previous was Wide, new is not -> clear its SpacerTail
        if prev_width == CellWidth::Wide && cell.width != CellWidth::Wide {
            let spacer_x = x + 1;
            if spacer_x < self.width {
                let spacer_ci = ci + 2;
                if CellWidth::from_u32(self.cells[spacer_ci + 1]) == CellWidth::SpacerTail {
                    self.cells[spacer_ci] = EMPTY_CHAR_INDEX;
                    self.cells[spacer_ci + 1] =
                        pack_word1(self.empty_style_id, 0, CellWidth::Narrow as u32);
                }
            }
        }

        // Previous was SpacerTail, new is not -> clear orphaned Wide at x-1
        let mut cleared_wide_x: Option<usize> = None;
        if prev_width == CellWidth::SpacerTail && cell.width != CellWidth::SpacerTail {
            if x > 0 {
                let wide_ci = ci - 2;
                if CellWidth::from_u32(self.cells[wide_ci + 1]) == CellWidth::Wide {
                    self.cells[wide_ci] = EMPTY_CHAR_INDEX;
                    self.cells[wide_ci + 1] =
                        pack_word1(self.empty_style_id, 0, CellWidth::Narrow as u32);
                    cleared_wide_x = Some(x - 1);
                }
            }
        }

        // Write the cell
        let char_id = char_pool.intern(&cell.char);
        let hyperlink_id = hyperlink_pool.intern(cell.hyperlink.as_deref());
        self.cells[ci] = char_id;
        self.cells[ci + 1] = pack_word1(cell.style_id, hyperlink_id, cell.width as u32);

        // Track damage
        let min_x = cleared_wide_x.unwrap_or(x);
        self.expand_damage(min_x, y, x, y);

        // Create spacer for wide characters
        if cell.width == CellWidth::Wide {
            let spacer_x = x + 1;
            if spacer_x < self.width {
                let spacer_ci = ci + 2;

                // If overwriting a Wide with our SpacerTail, clear its orphan
                if CellWidth::from_u32(self.cells[spacer_ci + 1]) == CellWidth::Wide {
                    let orphan_x = spacer_x + 1;
                    if orphan_x < self.width {
                        let orphan_ci = spacer_ci + 2;
                        if CellWidth::from_u32(self.cells[orphan_ci + 1]) == CellWidth::SpacerTail {
                            self.cells[orphan_ci] = EMPTY_CHAR_INDEX;
                            self.cells[orphan_ci + 1] =
                                pack_word1(self.empty_style_id, 0, CellWidth::Narrow as u32);
                        }
                    }
                }

                self.cells[spacer_ci] = SPACER_CHAR_INDEX;
                self.cells[spacer_ci + 1] =
                    pack_word1(self.empty_style_id, 0, CellWidth::SpacerTail as u32);
                self.expand_damage(spacer_x, y, spacer_x, y);
            }
        }
    }

    /// Replace the style_id of a cell in-place without disturbing char, width,
    /// or hyperlink. Skips spacer cells.
    pub fn set_cell_style_id(&mut self, x: usize, y: usize, style_id: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let ci = (y * self.width + x) * 2;
        let word1 = self.cells[ci + 1];
        let width = CellWidth::from_u32(word1);
        if width == CellWidth::SpacerTail || width == CellWidth::SpacerHead {
            return;
        }
        let hid = (word1 >> HYPERLINK_SHIFT) & HYPERLINK_MASK;
        self.cells[ci + 1] = pack_word1(style_id, hid, width as u32);
        self.expand_damage(x, y, x, y);
    }

    /// Bulk-copy a rectangular region from `src` to `self`.
    pub fn blit_region(
        &mut self,
        src: &Screen,
        region_x: usize,
        region_y: usize,
        max_x: usize,
        max_y: usize,
    ) {
        if region_x >= max_x || region_y >= max_y {
            return;
        }
        let row_len = max_x - region_x;

        for y in region_y..max_y {
            // Copy soft_wrap
            if y < src.soft_wrap.len() && y < self.soft_wrap.len() {
                self.soft_wrap[y] = src.soft_wrap[y];
            }

            let src_start = (y * src.width + region_x) * 2;
            let dst_start = (y * self.width + region_x) * 2;
            let len = row_len * 2;

            // Copy cells
            if src_start + len <= src.cells.len() && dst_start + len <= self.cells.len() {
                self.cells[dst_start..dst_start + len]
                    .copy_from_slice(&src.cells[src_start..src_start + len]);
            }

            // Copy noSelect
            let src_ns = y * src.width + region_x;
            let dst_ns = y * self.width + region_x;
            if src_ns + row_len <= src.no_select.len()
                && dst_ns + row_len <= self.no_select.len()
            {
                self.no_select[dst_ns..dst_ns + row_len]
                    .copy_from_slice(&src.no_select[src_ns..src_ns + row_len]);
            }
        }

        // Update damage
        let region_rect = Rect {
            x: region_x,
            y: region_y,
            width: row_len,
            height: max_y - region_y,
        };
        self.damage = Some(match self.damage {
            Some(d) => Rect::union(&d, &region_rect),
            None => region_rect,
        });
    }

    /// Bulk-clear a rectangular region.
    pub fn clear_region(
        &mut self,
        region_x: usize,
        region_y: usize,
        region_width: usize,
        region_height: usize,
    ) {
        let start_x = region_x;
        let start_y = region_y;
        let max_x = (region_x + region_width).min(self.width);
        let max_y = (region_y + region_height).min(self.height);
        if start_x >= max_x || start_y >= max_y {
            return;
        }

        let mut damage_min_x = start_x;
        let mut damage_max_x = max_x;

        for y in start_y..max_y {
            let row_start = y * self.width;

            // Left boundary: orphaned Wide cleanup
            if start_x > 0 {
                let left_ci = (row_start + start_x) * 2;
                if CellWidth::from_u32(self.cells[left_ci + 1]) == CellWidth::SpacerTail {
                    let prev_ci = left_ci - 2;
                    if CellWidth::from_u32(self.cells[prev_ci + 1]) == CellWidth::Wide {
                        self.cells[prev_ci] = EMPTY_CHAR_INDEX;
                        self.cells[prev_ci + 1] =
                            pack_word1(self.empty_style_id, 0, CellWidth::Narrow as u32);
                        damage_min_x = start_x.saturating_sub(1);
                    }
                }
            }

            // Right boundary: orphaned SpacerTail cleanup
            if max_x < self.width {
                let right_ci = (row_start + max_x - 1) * 2;
                if CellWidth::from_u32(self.cells[right_ci + 1]) == CellWidth::Wide {
                    let next_ci = (row_start + max_x) * 2;
                    if CellWidth::from_u32(self.cells[next_ci + 1]) == CellWidth::SpacerTail {
                        self.cells[next_ci] = EMPTY_CHAR_INDEX;
                        self.cells[next_ci + 1] =
                            pack_word1(self.empty_style_id, 0, CellWidth::Narrow as u32);
                        damage_max_x = max_x + 1;
                    }
                }
            }

            // Clear cells
            let ci_start = (row_start + start_x) * 2;
            let ci_end = (row_start + max_x) * 2;
            self.cells[ci_start..ci_end].fill(0);

            // Clear noSelect
            let ns_start = row_start + start_x;
            let ns_end = row_start + max_x;
            self.no_select[ns_start..ns_end].fill(0);
        }

        let region_rect = Rect {
            x: damage_min_x,
            y: start_y,
            width: damage_max_x - damage_min_x,
            height: max_y - start_y,
        };
        self.damage = Some(match self.damage {
            Some(d) => Rect::union(&d, &region_rect),
            None => region_rect,
        });
    }

    /// Shift full-width rows within `[top, bottom]` (inclusive, 0-indexed) by `n`.
    /// `n > 0` shifts UP (simulating CSI n S); `n < 0` shifts DOWN (CSI n T).
    /// Vacated rows are cleared.
    pub fn shift_rows(&mut self, top: usize, bottom: usize, n: i32) {
        if n == 0 || top > bottom || bottom >= self.height {
            return;
        }
        let w = self.width;
        let abs_n = n.unsigned_abs() as usize;

        if abs_n > bottom - top {
            // Clear entire region
            for y in top..=bottom {
                let ci_start = y * w * 2;
                let ci_end = (y + 1) * w * 2;
                self.cells[ci_start..ci_end].fill(0);
                let ns_start = y * w;
                let ns_end = (y + 1) * w;
                self.no_select[ns_start..ns_end].fill(0);
                self.soft_wrap[y] = 0;
            }
            return;
        }

        if n > 0 {
            // Shift UP
            for y in top..=(bottom - abs_n) {
                let src_y = y + abs_n;
                let src_ci = src_y * w * 2;
                let dst_ci = y * w * 2;
                self.cells.copy_within(src_ci..src_ci + w * 2, dst_ci);
                let src_ns = src_y * w;
                let dst_ns = y * w;
                self.no_select.copy_within(src_ns..src_ns + w, dst_ns);
                self.soft_wrap[y] = self.soft_wrap[src_y];
            }
            // Clear vacated rows
            for y in (bottom - abs_n + 1)..=bottom {
                let ci_start = y * w * 2;
                self.cells[ci_start..ci_start + w * 2].fill(0);
                self.no_select[y * w..(y + 1) * w].fill(0);
                self.soft_wrap[y] = 0;
            }
        } else {
            // Shift DOWN
            for y in (top + abs_n..=bottom).rev() {
                let src_y = y - abs_n;
                let src_ci = src_y * w * 2;
                let dst_ci = y * w * 2;
                self.cells.copy_within(src_ci..src_ci + w * 2, dst_ci);
                let src_ns = src_y * w;
                let dst_ns = y * w;
                self.no_select.copy_within(src_ns..src_ns + w, dst_ns);
                self.soft_wrap[y] = self.soft_wrap[src_y];
            }
            // Clear vacated rows
            for y in top..top + abs_n {
                let ci_start = y * w * 2;
                self.cells[ci_start..ci_start + w * 2].fill(0);
                self.no_select[y * w..(y + 1) * w].fill(0);
                self.soft_wrap[y] = 0;
            }
        }
    }

    // ---- internal ----

    fn expand_damage(&mut self, min_x: usize, min_y: usize, max_x: usize, max_y: usize) {
        let new_rect = Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
        };
        self.damage = Some(match self.damage {
            Some(d) => Rect::union(&d, &new_rect),
            None => new_rect,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::pools::{CharPool, HyperlinkPool, StylePool};

    fn make_pools() -> (CharPool, HyperlinkPool, StylePool) {
        (CharPool::new(), HyperlinkPool::new(), StylePool::new())
    }

    #[test]
    fn test_screen_new_all_empty() {
        let (char_pool, hyperlink_pool, style_pool) = make_pools();
        let screen = Screen::new(80, 24, &style_pool);
        for y in 0..24 {
            for x in 0..80 {
                assert!(screen.is_empty_at(x, y));
                let cell = screen.cell_at(x, y, &char_pool, &hyperlink_pool).unwrap();
                assert_eq!(cell.char, " ");
                assert_eq!(cell.style_id, 0);
                assert_eq!(cell.width, CellWidth::Narrow);
                assert!(cell.hyperlink.is_none());
            }
        }
    }

    #[test]
    fn test_set_and_get_cell() {
        let (mut char_pool, mut hyperlink_pool, style_pool) = make_pools();
        let mut screen = Screen::new(10, 5, &style_pool);

        let cell = Cell {
            char: "A".into(),
            style_id: 2,
            width: CellWidth::Narrow,
            hyperlink: None,
        };
        screen.set_cell_at(3, 1, &cell, &mut char_pool, &mut hyperlink_pool);

        let got = screen.cell_at(3, 1, &char_pool, &hyperlink_pool).unwrap();
        assert_eq!(got.char, "A");
        assert_eq!(got.style_id, 2);
        assert!(!screen.is_empty_at(3, 1));
    }

    #[test]
    fn test_wide_char_creates_spacer() {
        let (mut char_pool, mut hyperlink_pool, style_pool) = make_pools();
        let mut screen = Screen::new(10, 5, &style_pool);

        let cell = Cell {
            char: "\u{4e16}".into(), // CJK character (width=2)
            style_id: 0,
            width: CellWidth::Wide,
            hyperlink: None,
        };
        screen.set_cell_at(2, 0, &cell, &mut char_pool, &mut hyperlink_pool);

        // Position 2 should be Wide
        let got = screen.cell_at(2, 0, &char_pool, &hyperlink_pool).unwrap();
        assert_eq!(got.width, CellWidth::Wide);

        // Position 3 should be SpacerTail
        let spacer = screen.cell_at(3, 0, &char_pool, &hyperlink_pool).unwrap();
        assert_eq!(spacer.width, CellWidth::SpacerTail);
    }

    #[test]
    fn test_overwrite_wide_with_narrow_cleans_spacer() {
        let (mut char_pool, mut hyperlink_pool, style_pool) = make_pools();
        let mut screen = Screen::new(10, 5, &style_pool);

        // Place wide char
        screen.set_cell_at(
            2,
            0,
            &Cell {
                char: "\u{4e16}".into(),
                style_id: 0,
                width: CellWidth::Wide,
                hyperlink: None,
            },
            &mut char_pool,
            &mut hyperlink_pool,
        );

        // Overwrite with narrow char
        screen.set_cell_at(
            2,
            0,
            &Cell {
                char: "x".into(),
                style_id: 0,
                width: CellWidth::Narrow,
                hyperlink: None,
            },
            &mut char_pool,
            &mut hyperlink_pool,
        );

        // Spacer at 3 should now be Narrow (cleaned up)
        let spacer = screen.cell_at(3, 0, &char_pool, &hyperlink_pool).unwrap();
        assert_eq!(spacer.width, CellWidth::Narrow);
    }

    #[test]
    fn test_reset_screen() {
        let (mut char_pool, mut hyperlink_pool, style_pool) = make_pools();
        let mut screen = Screen::new(10, 5, &style_pool);

        screen.set_cell_at(
            0,
            0,
            &Cell {
                char: "A".into(),
                style_id: 1,
                width: CellWidth::Narrow,
                hyperlink: None,
            },
            &mut char_pool,
            &mut hyperlink_pool,
        );

        screen.reset(10, 5);
        assert!(screen.is_empty_at(0, 0));
        assert!(screen.damage.is_none());
    }

    #[test]
    fn test_damage_tracking() {
        let (mut char_pool, mut hyperlink_pool, style_pool) = make_pools();
        let mut screen = Screen::new(10, 5, &style_pool);

        assert!(screen.damage.is_none());

        screen.set_cell_at(
            2,
            1,
            &Cell::default(),
            &mut char_pool,
            &mut hyperlink_pool,
        );
        assert!(screen.damage.is_some());
        let d = screen.damage.unwrap();
        assert_eq!(d.x, 2);
        assert_eq!(d.y, 1);

        screen.set_cell_at(
            5,
            3,
            &Cell::default(),
            &mut char_pool,
            &mut hyperlink_pool,
        );
        let d = screen.damage.unwrap();
        assert!(d.x <= 2);
        assert!(d.y <= 1);
        assert!(d.x + d.width > 5);
        assert!(d.y + d.height > 3);
    }

    #[test]
    fn test_clear_region() {
        let (mut char_pool, mut hyperlink_pool, style_pool) = make_pools();
        let mut screen = Screen::new(10, 5, &style_pool);

        // Fill some cells
        for x in 0..10 {
            screen.set_cell_at(
                x,
                0,
                &Cell {
                    char: "X".into(),
                    ..Default::default()
                },
                &mut char_pool,
                &mut hyperlink_pool,
            );
        }

        // Clear a region
        screen.clear_region(2, 0, 5, 1);

        for x in 2..7 {
            assert!(screen.is_empty_at(x, 0));
        }
        // Cells outside should remain
        assert!(!screen.is_empty_at(0, 0));
        assert!(!screen.is_empty_at(8, 0));
    }

    #[test]
    fn test_shift_rows_up() {
        let (mut char_pool, mut hyperlink_pool, style_pool) = make_pools();
        let mut screen = Screen::new(5, 5, &style_pool);

        // Put 'A' on row 2
        screen.set_cell_at(
            0,
            2,
            &Cell {
                char: "A".into(),
                ..Default::default()
            },
            &mut char_pool,
            &mut hyperlink_pool,
        );

        screen.shift_rows(0, 4, 1);

        // Row 1 should now have 'A'
        let cell = screen.cell_at(0, 1, &char_pool, &hyperlink_pool).unwrap();
        assert_eq!(cell.char, "A");
        // Row 4 (bottom) should be empty
        assert!(screen.is_empty_at(0, 4));
    }
}
