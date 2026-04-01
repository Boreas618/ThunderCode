//! Virtual scrolling for the message list.
//!
//! Only renders messages within the visible viewport, matching the ref's
//! virtual-scroll approach for performance on long conversations.

use std::ops::Range;

// ---------------------------------------------------------------------------
// Message entry metadata
// ---------------------------------------------------------------------------

/// Metadata for a single entry in the virtual list.
///
/// Each entry knows its measured height (in terminal rows) so the list can
/// compute which entries fall within the viewport without rendering them all.
#[derive(Debug, Clone)]
pub struct MessageEntry {
    /// Unique message identifier (matches the API message id).
    pub id: String,
    /// The measured height of this message in terminal rows.
    /// Updated after layout; 1 is the minimum.
    pub height: usize,
    /// Whether this message is part of a tool-use group.
    pub group_id: Option<String>,
}

impl MessageEntry {
    pub fn new(id: impl Into<String>, height: usize) -> Self {
        Self {
            id: id.into(),
            height,
            group_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// VirtualMessageList
// ---------------------------------------------------------------------------

/// A virtual-scrolling message list.
///
/// Tracks all messages and a scroll offset, exposing only the visible range
/// for rendering. This avoids layout/render work for offscreen messages.
#[derive(Debug)]
pub struct VirtualMessageList {
    /// All message entries in order.
    pub messages: Vec<MessageEntry>,
    /// Scroll offset in rows from the top of the content.
    scroll_offset: usize,
    /// Height of the viewport in terminal rows.
    viewport_height: usize,
    /// Whether auto-scroll-to-bottom is active (like `tail -f`).
    pub auto_scroll: bool,
}

impl VirtualMessageList {
    pub fn new(viewport_height: usize) -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            viewport_height,
            auto_scroll: true,
        }
    }

    /// Total content height (sum of all message heights).
    pub fn total_height(&self) -> usize {
        self.messages.iter().map(|m| m.height).sum()
    }

    /// Current scroll offset in rows.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set the viewport height (e.g. on terminal resize).
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
        self.clamp_scroll();
    }

    /// Return the range of message *indices* that overlap the viewport.
    pub fn visible_range(&self) -> Range<usize> {
        if self.messages.is_empty() {
            return 0..0;
        }

        let mut cumulative = 0usize;
        let mut start_idx = None;
        let mut end_idx = self.messages.len();

        for (i, entry) in self.messages.iter().enumerate() {
            let entry_top = cumulative;
            let entry_bottom = cumulative + entry.height;

            if start_idx.is_none() && entry_bottom > self.scroll_offset {
                start_idx = Some(i);
            }
            if entry_top >= self.scroll_offset + self.viewport_height {
                end_idx = i;
                break;
            }
            cumulative += entry.height;
        }

        let start = start_idx.unwrap_or(0);
        start..end_idx
    }

    /// Scroll so the bottom of content aligns with the bottom of viewport.
    pub fn scroll_to_bottom(&mut self) {
        let total = self.total_height();
        if total > self.viewport_height {
            self.scroll_offset = total - self.viewport_height;
        } else {
            self.scroll_offset = 0;
        }
        self.auto_scroll = true;
    }

    /// Scroll by a signed delta (positive = down, negative = up).
    pub fn scroll_by(&mut self, delta: i32) {
        if delta < 0 {
            let abs = (-delta) as usize;
            self.scroll_offset = self.scroll_offset.saturating_sub(abs);
            self.auto_scroll = false;
        } else {
            self.scroll_offset = self.scroll_offset.saturating_add(delta as usize);
            self.clamp_scroll();
            // Re-enable auto-scroll if we're at the bottom
            let total = self.total_height();
            if total <= self.viewport_height
                || self.scroll_offset >= total - self.viewport_height
            {
                self.auto_scroll = true;
            }
        }
    }

    /// Scroll to make a specific message index visible.
    pub fn scroll_to_message(&mut self, index: usize) {
        if index >= self.messages.len() {
            return;
        }

        let mut cumulative = 0usize;
        for (i, entry) in self.messages.iter().enumerate() {
            if i == index {
                // If message is above viewport, scroll up to it
                if cumulative < self.scroll_offset {
                    self.scroll_offset = cumulative;
                    self.auto_scroll = false;
                }
                // If message is below viewport, scroll down so it's visible
                let entry_bottom = cumulative + entry.height;
                if entry_bottom > self.scroll_offset + self.viewport_height {
                    self.scroll_offset = entry_bottom.saturating_sub(self.viewport_height);
                    self.auto_scroll = false;
                }
                break;
            }
            cumulative += entry.height;
        }
    }

    /// Push a new message entry. If auto-scroll is on, scrolls to bottom.
    pub fn push(&mut self, entry: MessageEntry) {
        self.messages.push(entry);
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Update the height of an existing message (e.g. after re-layout).
    pub fn update_height(&mut self, index: usize, new_height: usize) {
        if let Some(entry) = self.messages.get_mut(index) {
            entry.height = new_height;
        }
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    /// Number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn clamp_scroll(&mut self) {
        let total = self.total_height();
        let max_offset = if total > self.viewport_height {
            total - self.viewport_height
        } else {
            0
        };
        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }
    }
}

// ---------------------------------------------------------------------------
// Message grouping
// ---------------------------------------------------------------------------

/// Group consecutive tool-use messages from the same assistant turn.
///
/// Returns a list of `(group_start_idx, group_len)` pairs.
/// Messages that are not part of a group have `group_len == 1`.
pub fn group_tool_uses(entries: &[MessageEntry]) -> Vec<(usize, usize)> {
    let mut groups = Vec::new();
    let mut i = 0;

    while i < entries.len() {
        if let Some(ref gid) = entries[i].group_id {
            // Start of a group: collect consecutive entries with same group_id
            let start = i;
            let group_id = gid.clone();
            while i < entries.len()
                && entries[i].group_id.as_ref() == Some(&group_id)
            {
                i += 1;
            }
            groups.push((start, i - start));
        } else {
            groups.push((i, 1));
            i += 1;
        }
    }

    groups
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_list() {
        let list = VirtualMessageList::new(20);
        assert_eq!(list.visible_range(), 0..0);
        assert_eq!(list.total_height(), 0);
        assert!(list.is_empty());
    }

    #[test]
    fn test_all_visible() {
        let mut list = VirtualMessageList::new(100);
        list.push(MessageEntry::new("a", 5));
        list.push(MessageEntry::new("b", 3));
        list.push(MessageEntry::new("c", 2));
        assert_eq!(list.visible_range(), 0..3);
        assert_eq!(list.total_height(), 10);
    }

    #[test]
    fn test_scroll_down() {
        let mut list = VirtualMessageList::new(5);
        for i in 0..10 {
            list.push(MessageEntry::new(format!("m{}", i), 3));
        }
        // total height = 30, viewport = 5
        // auto_scroll puts us at bottom
        assert_eq!(list.scroll_offset(), 25);

        // Scroll up
        list.scroll_by(-10);
        assert_eq!(list.scroll_offset(), 15);
        assert!(!list.auto_scroll);

        // visible_range should cover messages overlapping rows 15..20
        let range = list.visible_range();
        assert!(range.start <= 5);
        assert!(range.end >= 6);
    }

    #[test]
    fn test_scroll_to_bottom() {
        let mut list = VirtualMessageList::new(10);
        for i in 0..20 {
            list.push(MessageEntry::new(format!("m{}", i), 2));
        }
        list.scroll_by(-100); // scroll to top
        assert_eq!(list.scroll_offset(), 0);

        list.scroll_to_bottom();
        assert_eq!(list.scroll_offset(), 30); // 40 - 10
        assert!(list.auto_scroll);
    }

    #[test]
    fn test_scroll_to_message() {
        let mut list = VirtualMessageList::new(5);
        for i in 0..10 {
            list.push(MessageEntry::new(format!("m{}", i), 3));
        }
        // Scroll to message 2 (rows 6..9)
        list.scroll_by(-100); // reset to top
        list.scroll_to_message(2);
        // Message 2 starts at row 6, should be visible
        let range = list.visible_range();
        assert!(range.start <= 2 && range.end > 2);
    }

    #[test]
    fn test_update_height() {
        let mut list = VirtualMessageList::new(10);
        list.push(MessageEntry::new("a", 2));
        list.push(MessageEntry::new("b", 2));
        assert_eq!(list.total_height(), 4);

        list.update_height(1, 5);
        assert_eq!(list.total_height(), 7);
    }

    #[test]
    fn test_set_viewport_height() {
        let mut list = VirtualMessageList::new(10);
        for i in 0..5 {
            list.push(MessageEntry::new(format!("m{}", i), 3));
        }
        // total = 15, viewport = 10 -> offset = 5
        assert_eq!(list.scroll_offset(), 5);

        list.set_viewport_height(20);
        // total = 15, viewport = 20 -> offset = 0
        assert_eq!(list.scroll_offset(), 0);
    }

    #[test]
    fn test_group_tool_uses() {
        let entries = vec![
            MessageEntry::new("1", 2), // no group
            MessageEntry {
                id: "2".into(),
                height: 1,
                group_id: Some("turn1".into()),
            },
            MessageEntry {
                id: "3".into(),
                height: 1,
                group_id: Some("turn1".into()),
            },
            MessageEntry {
                id: "4".into(),
                height: 1,
                group_id: Some("turn1".into()),
            },
            MessageEntry::new("5", 2), // no group
        ];

        let groups = group_tool_uses(&entries);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0], (0, 1)); // standalone
        assert_eq!(groups[1], (1, 3)); // group of 3
        assert_eq!(groups[2], (4, 1)); // standalone
    }

    #[test]
    fn test_clear() {
        let mut list = VirtualMessageList::new(10);
        list.push(MessageEntry::new("a", 5));
        assert_eq!(list.len(), 1);

        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.scroll_offset(), 0);
    }
}
