//! Array/collection helpers: count, unique, chunk, interleave, reservoir sampling.
//!
//! Ported from ref/utils/array.ts` and extended with common collection
//! utilities needed throughout the codebase.

use std::collections::HashSet;
use std::hash::Hash;

/// Count elements in a slice that satisfy a predicate.
///
/// # Examples
/// ```
/// use crate::utils::array::count;
/// assert_eq!(count(&[1, 2, 3, 4, 5], |x| *x > 3), 2);
/// assert_eq!(count(&[true, false, true], |x| *x), 2);
/// ```
pub fn count<T>(items: &[T], pred: impl Fn(&T) -> bool) -> usize {
    items.iter().filter(|x| pred(x)).count()
}

/// Return unique elements preserving first-occurrence order.
///
/// # Examples
/// ```
/// use crate::utils::array::unique;
/// assert_eq!(unique(&[1, 2, 2, 3, 1]), vec![1, 2, 3]);
/// ```
pub fn unique<T: Eq + Hash + Clone>(items: &[T]) -> Vec<T> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|x| seen.insert((*x).clone()))
        .cloned()
        .collect()
}

/// Split a slice into chunks of at most `size` elements.
///
/// The last chunk may be smaller than `size`.
///
/// # Panics
/// Panics if `size` is 0.
///
/// # Examples
/// ```
/// use crate::utils::array::chunk;
/// assert_eq!(chunk(&[1, 2, 3, 4, 5], 2), vec![vec![1, 2], vec![3, 4], vec![5]]);
/// ```
pub fn chunk<T: Clone>(items: &[T], size: usize) -> Vec<Vec<T>> {
    assert!(size > 0, "chunk size must be > 0");
    items.chunks(size).map(|c| c.to_vec()).collect()
}

/// Interleave elements of a slice with a separator produced by `sep_fn`.
///
/// `sep_fn` receives the 0-based index of the element that *follows* the
/// separator (matching the TS `intersperse` signature).
///
/// # Examples
/// ```
/// use crate::utils::array::interleave;
/// let result = interleave(&["a", "b", "c"], |_| ",");
/// assert_eq!(result, vec!["a", ",", "b", ",", "c"]);
/// ```
pub fn interleave<T: Clone>(items: &[T], sep_fn: impl Fn(usize) -> T) -> Vec<T> {
    let mut result = Vec::with_capacity(items.len().saturating_mul(2).saturating_sub(1));
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            result.push(sep_fn(i));
        }
        result.push(item.clone());
    }
    result
}

/// Reservoir sampling: uniformly sample `k` items from an iterator of unknown
/// length, using O(k) memory.
///
/// Uses the standard Algorithm R (Vitter, 1985). The `rng` closure should
/// return a `f64` in `[0, 1)` (e.g. from `rand::random::<f64>()`).
///
/// # Examples
/// ```
/// use crate::utils::array::reservoir_sample;
///
/// let mut counter: u64 = 42;
/// let cheap_rng = || {
///     // Simple LCG for deterministic tests -- NOT for crypto.
///     counter = counter.wrapping_mul(6364136223846793005).wrapping_add(1);
///     (counter >> 33) as f64 / (1u64 << 31) as f64
/// };
///
/// let data: Vec<i32> = (0..1000).collect();
/// let sample = reservoir_sample(data.into_iter(), 10, cheap_rng);
/// assert_eq!(sample.len(), 10);
/// ```
pub fn reservoir_sample<T>(
    iter: impl Iterator<Item = T>,
    k: usize,
    mut rng: impl FnMut() -> f64,
) -> Vec<T> {
    let mut reservoir: Vec<T> = Vec::with_capacity(k);

    for (i, item) in iter.enumerate() {
        if i < k {
            reservoir.push(item);
        } else {
            let j = (rng() * (i + 1) as f64) as usize;
            if j < k {
                reservoir[j] = item;
            }
        }
    }

    reservoir
}

/// Flatten a slice of slices into a single `Vec`.
///
/// Equivalent to `items.iter().flatten().cloned().collect()` but more readable.
pub fn flatten<T: Clone>(items: &[Vec<T>]) -> Vec<T> {
    items.iter().flat_map(|v| v.iter().cloned()).collect()
}

/// Group elements by a key function, preserving insertion order within groups.
///
/// Returns a `Vec<(K, Vec<T>)>` instead of a `HashMap` to preserve the order
/// of first occurrence of each key.
pub fn group_by<T, K: Eq + Hash + Clone>(
    items: impl IntoIterator<Item = T>,
    key_fn: impl Fn(&T) -> K,
) -> Vec<(K, Vec<T>)> {
    let mut map: std::collections::HashMap<K, usize> = std::collections::HashMap::new();
    let mut groups: Vec<(K, Vec<T>)> = Vec::new();

    for item in items {
        let k = key_fn(&item);
        if let Some(&idx) = map.get(&k) {
            groups[idx].1.push(item);
        } else {
            let idx = groups.len();
            map.insert(k.clone(), idx);
            groups.push((k, vec![item]));
        }
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count() {
        assert_eq!(count(&[1, 2, 3, 4, 5], |x| *x > 3), 2);
        assert_eq!(count(&[1, 2, 3, 4, 5], |x| *x > 10), 0);
        assert_eq!(count::<i32>(&[], |_| true), 0);
    }

    #[test]
    fn test_unique() {
        assert_eq!(unique(&[1, 2, 2, 3, 1, 3]), vec![1, 2, 3]);
        assert_eq!(unique::<i32>(&[]), Vec::<i32>::new());
        assert_eq!(unique(&[1]), vec![1]);
    }

    #[test]
    fn test_chunk() {
        assert_eq!(
            chunk(&[1, 2, 3, 4, 5], 2),
            vec![vec![1, 2], vec![3, 4], vec![5]]
        );
        assert_eq!(chunk(&[1, 2, 3], 3), vec![vec![1, 2, 3]]);
        assert_eq!(chunk(&[1, 2, 3], 5), vec![vec![1, 2, 3]]);
        assert_eq!(chunk::<i32>(&[], 2), Vec::<Vec<i32>>::new());
    }

    #[test]
    #[should_panic(expected = "chunk size must be > 0")]
    fn test_chunk_zero_panics() {
        chunk(&[1], 0);
    }

    #[test]
    fn test_interleave() {
        let result = interleave(&[1, 2, 3], |_| 0);
        assert_eq!(result, vec![1, 0, 2, 0, 3]);

        let empty_result = interleave::<i32>(&[], |_| 0);
        assert!(empty_result.is_empty());

        let single = interleave(&[1], |_| 0);
        assert_eq!(single, vec![1]);
    }

    #[test]
    fn test_reservoir_sample_small_input() {
        let data = vec![1, 2, 3];
        let sample = reservoir_sample(data.into_iter(), 10, || 0.5);
        assert_eq!(sample.len(), 3); // input smaller than k
    }

    #[test]
    fn test_reservoir_sample_size() {
        let data: Vec<i32> = (0..1000).collect();
        let mut counter: u64 = 42;
        let sample = reservoir_sample(data.into_iter(), 10, || {
            counter = counter.wrapping_mul(6364136223846793005).wrapping_add(1);
            (counter >> 33) as f64 / (1u64 << 31) as f64
        });
        assert_eq!(sample.len(), 10);
    }

    #[test]
    fn test_flatten() {
        assert_eq!(flatten(&[vec![1, 2], vec![3], vec![4, 5]]), vec![1, 2, 3, 4, 5]);
        assert_eq!(flatten::<i32>(&[]), Vec::<i32>::new());
    }

    #[test]
    fn test_group_by() {
        let items = vec![1, 2, 3, 4, 5, 6];
        let groups = group_by(items, |x| x % 2);
        // First group seen is odd (1), then even (2)
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, 1); // odd
        assert_eq!(groups[0].1, vec![1, 3, 5]);
        assert_eq!(groups[1].0, 0); // even
        assert_eq!(groups[1].1, vec![2, 4, 6]);
    }
}
