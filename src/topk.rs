//! A Top-K tracker for the most frequent items in a stream.

use core::hash::{BuildHasher, Hash};

use alloc::vec::Vec;

use crate::{count_min::CountMinSketch, hash::DefaultHashBuilder, util::TOO_LARGE, Error};

/// One monitored item and its estimated frequency.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Entry<T> {
    key: T,
    count: u64,
}

/// Tracks the `k` most frequent items in a stream using bounded memory.
///
/// A Top-K tracker combines a [`CountMinSketch`](crate::CountMinSketch) — which
/// estimates how often *any* item has been seen — with a small list of the `k`
/// highest-frequency items observed so far. Every insertion updates the sketch
/// and, when the item's estimated count is high enough, promotes it into the
/// monitored list, evicting the current minimum. Memory is bounded by the sketch
/// size plus `k` stored keys, regardless of how many distinct items flow
/// through.
///
/// The estimate of *which* items are in the top set is approximate: under heavy
/// churn a true heavy hitter can be missed if it never accumulates a high enough
/// estimate while resident, but frequent, stable items are reported reliably.
///
/// Unlike the other structures, `TopK` stores the keys themselves, so the item
/// type must be `Eq + Hash` and sized. A key is moved into the monitored list
/// only when it is promoted, so non-promoted insertions store nothing.
///
/// # Examples
///
/// ```
/// use bloom_lib::TopK;
///
/// let mut top = TopK::new(3, 0.001, 0.001).unwrap();
///
/// for _ in 0..100 { top.insert("apple"); }
/// for _ in 0..50  { top.insert("banana"); }
/// for _ in 0..10  { top.insert("cherry"); }
/// for _ in 0..1   { top.insert("date"); }
///
/// let ranked = top.top();
/// assert_eq!(ranked[0].0, &"apple");
/// assert_eq!(ranked[1].0, &"banana");
/// assert_eq!(ranked.len(), 3); // only k items are kept
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(serialize = "CountMinSketch<T, S>: serde::Serialize, T: serde::Serialize"))
)]
pub struct TopK<T, S = DefaultHashBuilder> {
    sketch: CountMinSketch<T, S>,
    entries: Vec<Entry<T>>,
    k: usize,
}

impl<T> TopK<T, DefaultHashBuilder>
where
    T: Hash + Eq,
{
    /// Creates a tracker for the top `k` items, with an underlying sketch sized
    /// for error `epsilon` at confidence `1 - delta`, using the default hasher.
    ///
    /// # Parameters
    ///
    /// - `k`: how many top items to keep. Must be non-zero.
    /// - `epsilon`: the sketch error factor, in `(0.0, 1.0)`. Smaller is more
    ///   accurate and uses more memory.
    /// - `delta`: the sketch failure probability, in `(0.0, 1.0)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `k` is zero, or if `epsilon` or
    /// `delta` is not a finite value in `(0.0, 1.0)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::TopK;
    ///
    /// let top = TopK::<&str>::new(10, 0.001, 0.001).unwrap();
    /// assert_eq!(top.k(), 10);
    /// assert!(top.is_empty());
    /// ```
    pub fn new(k: usize, epsilon: f64, delta: f64) -> Result<Self, Error> {
        Self::with_hasher(k, epsilon, delta, DefaultHashBuilder)
    }
}

impl<T, S> TopK<T, S>
where
    T: Hash + Eq,
    S: BuildHasher,
{
    /// Creates a tracker with a caller-supplied hasher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `k` is zero, or if `epsilon` or
    /// `delta` is not a finite value in `(0.0, 1.0)`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] {
    /// use std::collections::hash_map::RandomState;
    /// use bloom_lib::TopK;
    ///
    /// let top: TopK<&str, RandomState> =
    ///     TopK::with_hasher(10, 0.001, 0.001, RandomState::new()).unwrap();
    /// # }
    /// ```
    pub fn with_hasher(k: usize, epsilon: f64, delta: f64, hasher: S) -> Result<Self, Error> {
        if k == 0 {
            return Err(Error::InvalidParameter {
                param: "k",
                reason: "must be greater than zero",
            });
        }
        let sketch = CountMinSketch::with_hasher(epsilon, delta, hasher)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(k)
            .map_err(|_| Error::InvalidParameter {
                param: "k",
                reason: TOO_LARGE,
            })?;
        Ok(Self { sketch, entries, k })
    }

    /// Records one occurrence of `item`, updating the sketch and the top set.
    ///
    /// The key is moved into the monitored list only if it is promoted into the
    /// top `k`; a non-promoted insertion stores nothing beyond the sketch
    /// update.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::TopK;
    ///
    /// let mut top = TopK::new(2, 0.01, 0.01).unwrap();
    /// top.insert("x");
    /// top.insert("x");
    /// assert_eq!(top.estimate(&"x"), 2);
    /// ```
    pub fn insert(&mut self, item: T) {
        self.sketch.increment(&item);
        let estimate = self.sketch.estimate(&item);

        // Already monitored: refresh its count.
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == item) {
            entry.count = estimate;
            return;
        }

        // Room to spare: admit unconditionally.
        if self.entries.len() < self.k {
            self.entries.push(Entry {
                key: item,
                count: estimate,
            });
            return;
        }

        // Full: replace the current minimum if this item now outranks it.
        if let Some((min_index, min_count)) = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (index, entry.count))
            .min_by_key(|&(_, count)| count)
        {
            if estimate > min_count {
                self.entries[min_index] = Entry {
                    key: item,
                    count: estimate,
                };
            }
        }
    }

    /// Returns the estimated frequency of `item` from the underlying sketch.
    ///
    /// This works for any item, whether or not it is in the top set.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::TopK;
    ///
    /// let mut top = TopK::new(5, 0.001, 0.001).unwrap();
    /// for _ in 0..7 { top.insert("seven"); }
    /// assert!(top.estimate(&"seven") >= 7);
    /// assert_eq!(top.estimate(&"absent"), 0);
    /// ```
    #[must_use]
    pub fn estimate(&self, item: &T) -> u64 {
        self.sketch.estimate(item)
    }

    /// Returns the monitored items paired with their estimated counts, ordered
    /// from most to least frequent.
    ///
    /// The returned vector borrows the keys, so no key cloning occurs. Its
    /// length is at most [`k`](Self::k).
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::TopK;
    ///
    /// let mut top = TopK::new(2, 0.01, 0.01).unwrap();
    /// for _ in 0..5 { top.insert("a"); }
    /// for _ in 0..3 { top.insert("b"); }
    ///
    /// let ranked = top.top();
    /// assert_eq!(ranked[0].0, &"a");
    /// assert_eq!(ranked[1].0, &"b");
    /// ```
    #[must_use]
    pub fn top(&self) -> Vec<(&T, u64)> {
        let mut ranked: Vec<(&T, u64)> = self
            .entries
            .iter()
            .map(|entry| (&entry.key, entry.count))
            .collect();
        // Highest count first; `sort_unstable` avoids an allocation.
        ranked.sort_unstable_by_key(|&(_, count)| core::cmp::Reverse(count));
        ranked
    }

    /// The configured number of top items to track.
    #[inline]
    #[must_use]
    pub fn k(&self) -> usize {
        self.k
    }

    /// The number of items currently monitored (at most [`k`](Self::k)).
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no items are monitored yet.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears the sketch and the monitored set.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::TopK;
    ///
    /// let mut top = TopK::new(3, 0.01, 0.01).unwrap();
    /// top.insert("x");
    /// top.clear();
    /// assert!(top.is_empty());
    /// assert_eq!(top.estimate(&"x"), 0);
    /// ```
    pub fn clear(&mut self) {
        self.sketch.clear();
        self.entries.clear();
    }
}

impl<T, S> TopK<T, S> {
    /// Returns the same tracker with its hasher replaced by `hasher`.
    ///
    /// Serialized state does not include the hasher, so this is how a
    /// tracker built with a keyed hasher is restored: deserialize it with the
    /// default hasher type, then attach the original key. Nothing is
    /// rehashed. `hasher` must hash exactly like the one the tracker was
    /// built with, or its frequency estimates become meaningless.
    #[must_use]
    pub fn replace_hasher<S2: BuildHasher>(self, hasher: S2) -> TopK<T, S2> {
        TopK {
            sketch: self.sketch.replace_hasher(hasher),
            entries: self.entries,
            k: self.k,
        }
    }
}

#[cfg(feature = "serde")]
impl<'de, T, S> serde::Deserialize<'de> for TopK<T, S>
where
    CountMinSketch<T, S>: serde::Deserialize<'de>,
    T: serde::Deserialize<'de>,
{
    /// Decodes a tracker and checks it: the sketch is validated as a
    /// [`CountMinSketch`], `k` must be non-zero, and at most `k` items may be
    /// monitored.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use crate::util::invalid;

        let raw = RawTopK::<T, S>::deserialize(deserializer)?;
        if raw.k == 0 {
            return Err(invalid("k", "must be greater than zero"));
        }
        if raw.entries.len() > raw.k {
            return Err(invalid("entries", "more than k items are monitored"));
        }
        Ok(Self {
            sketch: raw.sketch,
            entries: raw.entries,
            k: raw.k,
        })
    }
}

/// The serialized form of [`TopK`], decoded before any invariant is checked.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(
    rename = "TopK",
    bound(
        deserialize = "CountMinSketch<T, S>: serde::Deserialize<'de>, T: serde::Deserialize<'de>"
    )
)]
struct RawTopK<T, S> {
    sketch: CountMinSketch<T, S>,
    entries: Vec<Entry<T>>,
    k: usize,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_new_rejects_zero_k() {
        assert!(matches!(
            TopK::<&str>::new(0, 0.01, 0.01),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_new_rejects_bad_sketch_params() {
        assert!(matches!(
            TopK::<&str>::new(5, 0.0, 0.01),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_tracks_most_frequent() {
        let mut top = TopK::new(3, 0.001, 0.001).unwrap();
        for _ in 0..100 {
            top.insert("apple");
        }
        for _ in 0..50 {
            top.insert("banana");
        }
        for _ in 0..10 {
            top.insert("cherry");
        }
        for _ in 0..1 {
            top.insert("date");
        }

        let ranked = top.top();
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].0, &"apple");
        assert_eq!(ranked[1].0, &"banana");
        assert_eq!(ranked[2].0, &"cherry");
    }

    #[test]
    fn test_counts_are_estimated() {
        let mut top = TopK::new(5, 0.0001, 0.0001).unwrap();
        for _ in 0..42 {
            top.insert("x");
        }
        assert!(top.estimate(&"x") >= 42);
        assert_eq!(top.estimate(&"never-seen"), 0);
    }

    #[test]
    fn test_len_capped_at_k() {
        let mut top = TopK::new(2, 0.01, 0.01).unwrap();
        for i in 0..100u32 {
            top.insert(i);
        }
        assert_eq!(top.len(), 2);
        assert!(top.len() <= top.k());
    }

    #[test]
    fn test_eviction_replaces_minimum() {
        let mut top = TopK::new(2, 0.0001, 0.0001).unwrap();
        for _ in 0..10 {
            top.insert("a");
        }
        for _ in 0..5 {
            top.insert("b");
        }
        // "c" arrives with a higher count than the minimum ("b").
        for _ in 0..8 {
            top.insert("c");
        }
        let ranked = top.top();
        let keys: Vec<&str> = ranked.iter().map(|&(key, _)| *key).collect();
        assert!(keys.contains(&"a"));
        assert!(keys.contains(&"c"));
        assert!(!keys.contains(&"b"), "low-frequency item should be evicted");
    }

    #[test]
    fn test_clear() {
        let mut top = TopK::new(3, 0.01, 0.01).unwrap();
        top.insert("x");
        assert!(!top.is_empty());
        top.clear();
        assert!(top.is_empty());
        assert_eq!(top.estimate(&"x"), 0);
    }

    #[test]
    fn test_top_is_sorted_descending() {
        let mut top = TopK::new(4, 0.0001, 0.0001).unwrap();
        for _ in 0..3 {
            top.insert("low");
        }
        for _ in 0..30 {
            top.insert("high");
        }
        for _ in 0..15 {
            top.insert("mid");
        }
        let ranked = top.top();
        for pair in ranked.windows(2) {
            assert!(pair[0].1 >= pair[1].1, "top() not sorted descending");
        }
    }
}
