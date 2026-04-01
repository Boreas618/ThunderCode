//! Function memoization with TTL-based expiration and LRU eviction.
//!
//! Ported from ref/utils/memoize.ts`. Provides three strategies:
//! - [`MemoizeWithTtl`]: write-through cache with time-based expiration
//! - [`MemoizeWithTtlAsync`]: async version of TTL memoization
//! - [`MemoizeWithLru`]: bounded LRU cache (prevents unbounded memory growth)

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// TTL-based memoization (sync)
// ---------------------------------------------------------------------------

struct TtlEntry<V> {
    value: V,
    inserted_at: Instant,
}

/// A thread-safe, TTL-based memoization cache.
///
/// Values that are older than `ttl` are lazily evicted on the next access.
/// Stale values are returned immediately while a background refresh is not
/// attempted in the sync variant (unlike the TS version which uses microtasks).
///
/// # Examples
/// ```
/// use crate::utils::memoize::MemoizeWithTtl;
/// use std::time::Duration;
///
/// let memo = MemoizeWithTtl::new(Duration::from_secs(60));
/// let result = memo.get_or_insert("key", || expensive_computation());
///
/// fn expensive_computation() -> i32 { 42 }
/// ```
pub struct MemoizeWithTtl<K, V> {
    cache: Mutex<HashMap<K, TtlEntry<V>>>,
    ttl: Duration,
}

impl<K, V> MemoizeWithTtl<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    /// Create a new TTL cache with the given lifetime.
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Get a cached value or compute it with `f`. Returns the cached value if
    /// still within TTL, otherwise recomputes.
    pub fn get_or_insert(&self, key: K, f: impl FnOnce() -> V) -> V {
        let mut cache = self.cache.lock().unwrap();
        if let Some(entry) = cache.get(&key) {
            if entry.inserted_at.elapsed() < self.ttl {
                return entry.value.clone();
            }
        }
        let value = f();
        cache.insert(
            key,
            TtlEntry {
                value: value.clone(),
                inserted_at: Instant::now(),
            },
        );
        value
    }

    /// Remove all entries from the cache.
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }

    /// Number of entries currently in the cache (including stale ones).
    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.lock().unwrap().is_empty()
    }
}

// ---------------------------------------------------------------------------
// TTL-based memoization (async)
// ---------------------------------------------------------------------------

/// An async, thread-safe, TTL-based memoization cache.
///
/// Wraps the synchronous TTL cache and is safe to use across `.await` points
/// because the lock is never held across awaits.
///
/// # Examples
/// ```
/// use crate::utils::memoize::MemoizeWithTtlAsync;
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let memo = MemoizeWithTtlAsync::new(Duration::from_secs(60));
/// let result = memo.get_or_insert("key", || async { 42 }).await;
/// assert_eq!(result, 42);
/// # });
/// ```
pub struct MemoizeWithTtlAsync<K, V> {
    cache: Arc<Mutex<HashMap<K, TtlEntry<V>>>>,
    ttl: Duration,
}

impl<K, V> MemoizeWithTtlAsync<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    /// Get a cached value or compute it with the async closure `f`.
    pub async fn get_or_insert<F, Fut>(&self, key: K, f: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = V>,
    {
        // Check cache (lock is dropped before await)
        {
            let cache = self.cache.lock().unwrap();
            if let Some(entry) = cache.get(&key) {
                if entry.inserted_at.elapsed() < self.ttl {
                    return entry.value.clone();
                }
            }
        }

        // Compute outside the lock
        let value = f().await;

        // Store result
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(
                key,
                TtlEntry {
                    value: value.clone(),
                    inserted_at: Instant::now(),
                },
            );
        }

        value
    }

    /// Remove all entries.
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.lock().unwrap().is_empty()
    }
}

impl<K, V> Clone for MemoizeWithTtlAsync<K, V> {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
            ttl: self.ttl,
        }
    }
}

// ---------------------------------------------------------------------------
// LRU-based memoization
// ---------------------------------------------------------------------------

struct LruEntry<V> {
    value: V,
    /// Monotonically increasing counter for LRU eviction.
    last_access: u64,
}

/// A thread-safe, bounded LRU memoization cache.
///
/// When the cache reaches `max_size`, the least recently used entry is evicted.
/// This prevents unbounded memory growth that was observed with the TS
/// lodash.memoize approach.
///
/// # Examples
/// ```
/// use crate::utils::memoize::MemoizeWithLru;
///
/// let memo = MemoizeWithLru::new(100);
/// let val = memo.get_or_insert("key".to_string(), || 42);
/// assert_eq!(val, 42);
/// ```
pub struct MemoizeWithLru<K, V> {
    cache: Mutex<LruInner<K, V>>,
    max_size: usize,
}

struct LruInner<K, V> {
    map: HashMap<K, LruEntry<V>>,
    counter: u64,
}

impl<K, V> MemoizeWithLru<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    /// Create a new LRU cache that holds at most `max_size` entries.
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Mutex::new(LruInner {
                map: HashMap::new(),
                counter: 0,
            }),
            max_size,
        }
    }

    /// Get a cached value or compute it with `f`. Evicts the LRU entry if the
    /// cache is full.
    pub fn get_or_insert(&self, key: K, f: impl FnOnce() -> V) -> V {
        let mut inner = self.cache.lock().unwrap();

        inner.counter += 1;
        let tick = inner.counter;

        if let Some(entry) = inner.map.get_mut(&key) {
            entry.last_access = tick;
            return entry.value.clone();
        }

        let value = f();

        // Evict LRU if at capacity
        if inner.map.len() >= self.max_size {
            let lru_key = inner
                .map
                .iter()
                .min_by_key(|(_, e)| e.last_access)
                .map(|(k, _)| k.clone());
            if let Some(k) = lru_key {
                inner.map.remove(&k);
            }
        }

        inner.map.insert(
            key,
            LruEntry {
                value: value.clone(),
                last_access: tick,
            },
        );

        value
    }

    /// Peek at a cached value without updating its recency.
    pub fn peek(&self, key: &K) -> Option<V> {
        let inner = self.cache.lock().unwrap();
        inner.map.get(key).map(|e| e.value.clone())
    }

    /// Check if the cache contains a key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.cache.lock().unwrap().map.contains_key(key)
    }

    /// Remove a specific key.
    pub fn remove(&self, key: &K) -> bool {
        self.cache.lock().unwrap().map.remove(key).is_some()
    }

    /// Remove all entries.
    pub fn clear(&self) {
        let mut inner = self.cache.lock().unwrap();
        inner.map.clear();
        inner.counter = 0;
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().map.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.lock().unwrap().map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttl_cache_basic() {
        let memo = MemoizeWithTtl::new(Duration::from_secs(60));
        let mut call_count = 0;

        let v1 = memo.get_or_insert("a", || {
            call_count += 1;
            call_count
        });
        assert_eq!(v1, 1);

        // Second call should return cached value
        let v2 = memo.get_or_insert("a", || {
            call_count += 1;
            call_count
        });
        assert_eq!(v2, 1); // still 1, cached

        assert_eq!(memo.len(), 1);
    }

    #[test]
    fn test_ttl_cache_clear() {
        let memo = MemoizeWithTtl::new(Duration::from_secs(60));
        memo.get_or_insert("a", || 1);
        memo.get_or_insert("b", || 2);
        assert_eq!(memo.len(), 2);

        memo.clear();
        assert!(memo.is_empty());
    }

    #[test]
    fn test_lru_cache_basic() {
        let memo = MemoizeWithLru::new(2);

        let v = memo.get_or_insert("a".to_string(), || 1);
        assert_eq!(v, 1);

        let v = memo.get_or_insert("b".to_string(), || 2);
        assert_eq!(v, 2);

        assert_eq!(memo.len(), 2);
    }

    #[test]
    fn test_lru_eviction() {
        let memo = MemoizeWithLru::new(2);

        memo.get_or_insert("a".to_string(), || 1);
        memo.get_or_insert("b".to_string(), || 2);

        // Access "a" so it's more recent than "b"
        memo.get_or_insert("a".to_string(), || 999);

        // Insert "c" -- "b" should be evicted (least recently used)
        memo.get_or_insert("c".to_string(), || 3);

        assert_eq!(memo.len(), 2);
        assert!(memo.contains_key(&"a".to_string()));
        assert!(!memo.contains_key(&"b".to_string()));
        assert!(memo.contains_key(&"c".to_string()));
    }

    #[test]
    fn test_lru_peek_does_not_update_recency() {
        let memo = MemoizeWithLru::new(2);

        memo.get_or_insert("a".to_string(), || 1);
        memo.get_or_insert("b".to_string(), || 2);

        // Peek at "a" -- should NOT update recency
        assert_eq!(memo.peek(&"a".to_string()), Some(1));

        // Insert "c" -- "a" should be evicted since peek didn't update recency
        memo.get_or_insert("c".to_string(), || 3);

        assert!(!memo.contains_key(&"a".to_string()));
        assert!(memo.contains_key(&"b".to_string()));
        assert!(memo.contains_key(&"c".to_string()));
    }

    #[test]
    fn test_lru_remove() {
        let memo = MemoizeWithLru::new(10);
        memo.get_or_insert("a".to_string(), || 1);
        assert!(memo.remove(&"a".to_string()));
        assert!(!memo.remove(&"a".to_string()));
        assert!(memo.is_empty());
    }

    #[tokio::test]
    async fn test_ttl_async_basic() {
        let memo = MemoizeWithTtlAsync::new(Duration::from_secs(60));

        let v = memo.get_or_insert("a", || async { 42 }).await;
        assert_eq!(v, 42);

        // Second call returns cached value
        let v = memo.get_or_insert("a", || async { 99 }).await;
        assert_eq!(v, 42);
    }

    #[tokio::test]
    async fn test_ttl_async_clear() {
        let memo = MemoizeWithTtlAsync::new(Duration::from_secs(60));
        memo.get_or_insert("a", || async { 1 }).await;
        memo.clear();
        assert!(memo.is_empty());

        // After clear, should recompute
        let v = memo.get_or_insert("a", || async { 99 }).await;
        assert_eq!(v, 99);
    }
}
