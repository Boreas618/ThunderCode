//! Generic reactive store with thread-safe state and subscription support.
//!
//! Ported from ref/state/store.ts.  The TypeScript original uses a plain
//! object with `Object.is` equality for skip detection; we use `PartialEq`
//! and `Arc<RwLock<..>>` to make the store safe across threads.

use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// SubscriptionHandle
// ---------------------------------------------------------------------------

/// RAII guard returned by [`Store::subscribe`].  Dropping the handle
/// unsubscribes the listener automatically.
pub struct SubscriptionHandle {
    /// Index into the listeners Vec.
    id: usize,
    /// Shared reference to the list so we can mark the slot as `None`.
    listeners: Arc<RwLock<Vec<Option<Box<dyn Fn(&dyn std::any::Any) + Send + Sync>>>>>,
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        if let Ok(mut listeners) = self.listeners.write() {
            if self.id < listeners.len() {
                listeners[self.id] = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// OnChange callback
// ---------------------------------------------------------------------------

/// Signature for the optional `on_change` callback passed at construction.
/// Called whenever state actually changes (after skip-same-value check).
pub type OnChangeFn<T> = Box<dyn Fn(&T, &T) + Send + Sync>;

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// A thread-safe reactive store holding a single value of type `T`.
///
/// # Semantics
///
/// * **Same-value skip** -- `set_state` only notifies listeners when the new
///   value differs from the current one (via `PartialEq`).
/// * **Thread safety** -- all access goes through `Arc<RwLock<..>>`.
/// * **Non-blocking listeners** -- listeners receive an immutable `&T`
///   reference; they must not block.
pub struct Store<T: Clone + PartialEq + Send + Sync + 'static> {
    state: Arc<RwLock<T>>,
    listeners: Arc<RwLock<Vec<Option<Box<dyn Fn(&dyn std::any::Any) + Send + Sync>>>>>,
    on_change: Option<OnChangeFn<T>>,
}

impl<T: Clone + PartialEq + Send + Sync + 'static> Store<T> {
    /// Create a new store with the given initial value.
    pub fn new(initial: T) -> Self {
        Self {
            state: Arc::new(RwLock::new(initial)),
            listeners: Arc::new(RwLock::new(Vec::new())),
            on_change: None,
        }
    }

    /// Create a new store with an `on_change` callback that fires whenever
    /// the state transitions from one value to another.
    pub fn with_on_change(initial: T, on_change: impl Fn(&T, &T) + Send + Sync + 'static) -> Self {
        Self {
            state: Arc::new(RwLock::new(initial)),
            listeners: Arc::new(RwLock::new(Vec::new())),
            on_change: Some(Box::new(on_change)),
        }
    }

    /// Return a clone of the current state.
    pub fn get_state(&self) -> T {
        self.state.read().expect("state lock poisoned").clone()
    }

    /// Update the state by applying `f` to the current value.
    ///
    /// If the new value equals the old one (`PartialEq`), the update is
    /// silently skipped -- no listeners fire and `on_change` is not called.
    pub fn set_state(&self, f: impl FnOnce(&T) -> T) {
        let (old, new) = {
            let mut state = self.state.write().expect("state lock poisoned");
            let old = state.clone();
            let new = f(&old);
            if new == old {
                return;
            }
            *state = new.clone();
            (old, new)
        };

        // Fire the structured on_change callback (old, new).
        if let Some(ref on_change) = self.on_change {
            on_change(&new, &old);
        }

        // Notify subscribers.
        let listeners = self.listeners.read().expect("listeners lock poisoned");
        for slot in listeners.iter() {
            if let Some(listener) = slot {
                listener(&new as &dyn std::any::Any);
            }
        }
    }

    /// Register a listener that will be called with `&T` on every state
    /// change.  Returns a [`SubscriptionHandle`]; dropping it unsubscribes.
    pub fn subscribe(
        &self,
        listener: impl Fn(&T) + Send + Sync + 'static,
    ) -> SubscriptionHandle {
        // Wrap the typed listener into a type-erased `Fn(&dyn Any)`.
        let wrapped: Box<dyn Fn(&dyn std::any::Any) + Send + Sync> = Box::new(move |any| {
            if let Some(val) = any.downcast_ref::<T>() {
                listener(val);
            }
        });

        let mut listeners = self.listeners.write().expect("listeners lock poisoned");
        let id = listeners.len();
        listeners.push(Some(wrapped));

        SubscriptionHandle {
            id,
            listeners: Arc::clone(&self.listeners),
        }
    }
}

// Allow cloning a store handle (shares the same inner state).
impl<T: Clone + PartialEq + Send + Sync + 'static> Clone for Store<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            listeners: Arc::clone(&self.listeners),
            // on_change is NOT cloned -- it belongs to the original creator.
            on_change: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn get_returns_initial_state() {
        let store = Store::new(42);
        assert_eq!(store.get_state(), 42);
    }

    #[test]
    fn set_state_updates_value() {
        let store = Store::new(0);
        store.set_state(|prev| prev + 1);
        assert_eq!(store.get_state(), 1);
    }

    #[test]
    fn set_state_skips_same_value() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);
        let store = Store::new(10);
        let _handle = store.subscribe(move |_: &i32| {
            cc.fetch_add(1, Ordering::SeqCst);
        });

        // Same value -- listener should NOT fire.
        store.set_state(|prev| *prev);
        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        // Different value -- listener should fire.
        store.set_state(|_| 20);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn subscribe_notifies_on_change() {
        let values = Arc::new(RwLock::new(Vec::<i32>::new()));
        let store = Store::new(0);

        let v = Arc::clone(&values);
        let _handle = store.subscribe(move |val: &i32| {
            v.write().unwrap().push(*val);
        });

        store.set_state(|_| 1);
        store.set_state(|_| 2);
        store.set_state(|_| 3);

        assert_eq!(*values.read().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn dropping_handle_unsubscribes() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let store = Store::new(0);

        let cc = Arc::clone(&call_count);
        let handle = store.subscribe(move |_: &i32| {
            cc.fetch_add(1, Ordering::SeqCst);
        });

        store.set_state(|_| 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        drop(handle);

        store.set_state(|_| 2);
        // Should still be 1 because the listener was unsubscribed.
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn multiple_subscribers() {
        let a = Arc::new(AtomicUsize::new(0));
        let b = Arc::new(AtomicUsize::new(0));
        let store = Store::new(0);

        let aa = Arc::clone(&a);
        let _h1 = store.subscribe(move |_: &i32| {
            aa.fetch_add(1, Ordering::SeqCst);
        });

        let bb = Arc::clone(&b);
        let _h2 = store.subscribe(move |_: &i32| {
            bb.fetch_add(1, Ordering::SeqCst);
        });

        store.set_state(|_| 1);
        assert_eq!(a.load(Ordering::SeqCst), 1);
        assert_eq!(b.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn on_change_callback() {
        let old_vals = Arc::new(RwLock::new(Vec::<i32>::new()));
        let new_vals = Arc::new(RwLock::new(Vec::<i32>::new()));

        let ov = Arc::clone(&old_vals);
        let nv = Arc::clone(&new_vals);
        let store = Store::with_on_change(0, move |new_state: &i32, old_state: &i32| {
            nv.write().unwrap().push(*new_state);
            ov.write().unwrap().push(*old_state);
        });

        store.set_state(|_| 10);
        store.set_state(|_| 20);

        assert_eq!(*new_vals.read().unwrap(), vec![10, 20]);
        assert_eq!(*old_vals.read().unwrap(), vec![0, 10]);
    }

    #[test]
    fn store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Store<i32>>();
        assert_send_sync::<Store<String>>();
    }

    #[test]
    fn concurrent_set_state() {
        let store = Store::new(0i32);
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let s = store.clone();
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        s.set_state(|prev| prev + 1);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(store.get_state(), 1000);
    }
}
