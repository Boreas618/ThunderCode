//! Priority-based notification queue.
//!
//! Ported from ref/context/notifications.tsx` -- simplified to a
//! priority-ordered queue with a "current" display slot.  The TUI
//! renderer pops from the queue and shows one notification at a time.

use std::collections::VecDeque;

use crate::state::notification::{Notification, NotificationPriority};

// ---------------------------------------------------------------------------
// NotificationQueue
// ---------------------------------------------------------------------------

/// A priority-ordered queue of [`Notification`]s with a single "current"
/// display slot.
///
/// The queue is sorted by priority on insertion (highest first).
/// `Immediate`-priority notifications bypass the queue and replace the
/// current notification directly.
#[derive(Debug, Clone, Default)]
pub struct NotificationQueue {
    /// Pending notifications ordered by priority (highest first).
    queue: VecDeque<Notification>,
    /// The notification currently being displayed, if any.
    current: Option<Notification>,
}

impl NotificationQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a notification into the queue.
    ///
    /// `Immediate`-priority notifications replace the current notification
    /// instantly and move any displaced notification back to the front of
    /// the queue.  Other priorities are inserted in sorted order.
    pub fn push(&mut self, notif: Notification) {
        if notif.priority == NotificationPriority::Immediate {
            // Immediate: bypass the queue.
            if let Some(displaced) = self.current.take() {
                // Put the displaced notification back at the front.
                self.queue.push_front(displaced);
            }
            self.current = Some(notif);
        } else {
            // Insert in priority-sorted position (highest first).
            let pos = self
                .queue
                .iter()
                .position(|n| priority_rank(n.priority) < priority_rank(notif.priority))
                .unwrap_or(self.queue.len());
            self.queue.insert(pos, notif);
        }
    }

    /// Pop the next notification from the queue and make it "current".
    ///
    /// If there is already a current notification it is *not* replaced;
    /// call [`dismiss_current`](Self::dismiss_current) first.
    ///
    /// Returns the newly-promoted current notification, if any.
    pub fn pop(&mut self) -> Option<Notification> {
        if self.current.is_some() {
            return None; // current slot is occupied
        }
        let next = self.queue.pop_front();
        self.current = next.clone();
        next
    }

    /// Return a reference to the currently displayed notification.
    pub fn current(&self) -> Option<&Notification> {
        self.current.as_ref()
    }

    /// Dismiss the current notification and return it.
    ///
    /// The caller should follow up with [`pop`](Self::pop) to promote the
    /// next queued notification if desired.
    pub fn dismiss_current(&mut self) -> Option<Notification> {
        self.current.take()
    }

    /// Remove all queued notifications with the given `id`.
    ///
    /// If the current notification matches, it is also dismissed.
    pub fn remove_by_id(&mut self, id: &str) {
        self.queue.retain(|n| n.id != id);
        if self.current.as_ref().map_or(false, |n| n.id == id) {
            self.current = None;
        }
    }

    /// Number of notifications waiting in the queue (excludes current).
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Whether the queue is completely empty (no current, nothing pending).
    pub fn is_empty(&self) -> bool {
        self.current.is_none() && self.queue.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a [`NotificationPriority`] to a numeric rank for sorting.
/// Higher numbers = higher priority.
fn priority_rank(p: NotificationPriority) -> u8 {
    match p {
        NotificationPriority::Low => 0,
        NotificationPriority::Medium => 1,
        NotificationPriority::High => 2,
        NotificationPriority::Immediate => 3,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make(msg: &str, priority: NotificationPriority) -> Notification {
        Notification::new(msg, priority, Some(3_000))
    }

    #[test]
    fn push_and_pop() {
        let mut q = NotificationQueue::new();
        q.push(make("first", NotificationPriority::Low));
        q.push(make("second", NotificationPriority::Low));

        let n = q.pop().unwrap();
        assert_eq!(n.message, "first");
        assert!(q.current().is_some());

        // pop while current is occupied returns None.
        assert!(q.pop().is_none());

        q.dismiss_current();
        let n = q.pop().unwrap();
        assert_eq!(n.message, "second");
    }

    #[test]
    fn priority_ordering() {
        let mut q = NotificationQueue::new();
        q.push(make("low", NotificationPriority::Low));
        q.push(make("high", NotificationPriority::High));
        q.push(make("medium", NotificationPriority::Medium));

        let n = q.pop().unwrap();
        assert_eq!(n.message, "high");
        q.dismiss_current();

        let n = q.pop().unwrap();
        assert_eq!(n.message, "medium");
        q.dismiss_current();

        let n = q.pop().unwrap();
        assert_eq!(n.message, "low");
    }

    #[test]
    fn immediate_bypasses_queue() {
        let mut q = NotificationQueue::new();
        q.push(make("normal", NotificationPriority::Low));
        let _ = q.pop(); // promote "normal" to current

        q.push(make("urgent", NotificationPriority::Immediate));

        // "urgent" should have replaced current.
        assert_eq!(q.current().unwrap().message, "urgent");
        // "normal" should be back in the queue.
        assert_eq!(q.pending_count(), 1);
    }

    #[test]
    fn remove_by_id() {
        let mut q = NotificationQueue::new();
        let n = make("target", NotificationPriority::Low);
        let target_id = n.id.clone();
        q.push(n);
        q.push(make("other", NotificationPriority::Low));

        q.remove_by_id(&target_id);
        assert_eq!(q.pending_count(), 1);

        let popped = q.pop().unwrap();
        assert_eq!(popped.message, "other");
    }

    #[test]
    fn is_empty() {
        let mut q = NotificationQueue::new();
        assert!(q.is_empty());

        q.push(make("x", NotificationPriority::Low));
        assert!(!q.is_empty());

        let _ = q.pop();
        assert!(!q.is_empty()); // current is occupied

        q.dismiss_current();
        assert!(q.is_empty());
    }
}
