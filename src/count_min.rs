//! A Count-Min Sketch for approximate frequency estimation.

use core::{hash::BuildHasher, marker::PhantomData};

use alloc::vec::Vec;

use crate::{
    hash::{reduce, DefaultHashBuilder, HashPair},
    util::{filled_vec, TOO_LARGE},
    Error,
};

/// Euler's number, used to size the sketch width from a target error.
const E: f64 = core::f64::consts::E;

/// A sublinear-space frequency estimator.
///
/// A Count-Min Sketch counts how many times it has seen each item using a fixed
/// grid of counters, far smaller than a real `HashMap<T, u64>` would be. Each
/// item is hashed into one counter per row and those counters are incremented;
/// the estimate for an item is the *minimum* across its rows. The estimate never
/// undercounts and overcounts by a bounded amount with high probability — the
/// width controls the error magnitude `epsilon`, the depth controls the
/// confidence `delta`.
///
/// The sketch is generic over the item type `T` and a
/// [`BuildHasher`](core::hash::BuildHasher) `S`, defaulting to the deterministic
/// [`DefaultHashBuilder`](crate::hash::DefaultHashBuilder).
///
/// # Examples
///
/// ```
/// use bloom_lib::CountMinSketch;
///
/// // ~0.1% error with 99.9% confidence.
/// let mut sketch = CountMinSketch::new(0.001, 0.001).unwrap();
///
/// sketch.increment("apple");
/// sketch.add("apple", 4);
/// sketch.increment("banana");
///
/// assert!(sketch.estimate("apple") >= 5); // never undercounts
/// assert_eq!(sketch.estimate("cherry"), 0);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CountMinSketch<T: ?Sized, S = DefaultHashBuilder> {
    counters: Vec<u64>,
    width: usize,
    depth: usize,
    total: u64,
    #[cfg_attr(feature = "serde", serde(skip))]
    hasher: S,
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: PhantomData<fn(&T)>,
}

impl<T: ?Sized> CountMinSketch<T, DefaultHashBuilder> {
    /// Creates a sketch sized for an error factor `epsilon` at confidence
    /// `1 - delta`, using the default hasher.
    ///
    /// The estimate for any item never undercounts and, with probability at
    /// least `1 - delta`, overcounts by at most `epsilon * N`, where `N` is the
    /// total of all counts added. Smaller `epsilon` widens the grid; smaller
    /// `delta` deepens it.
    ///
    /// # Parameters
    ///
    /// - `epsilon`: the relative error factor. Must be in `(0.0, 1.0)`. The
    ///   width is `ceil(e / epsilon)`.
    /// - `delta`: the failure probability. Must be in `(0.0, 1.0)`. The depth is
    ///   `ceil(ln(1 / delta))`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if either argument is not a finite
    /// value in the open interval `(0.0, 1.0)`, or the counter grid they imply
    /// cannot be allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CountMinSketch;
    ///
    /// let sketch = CountMinSketch::<&str>::new(0.01, 0.01).unwrap();
    /// assert!(sketch.width() >= 271);
    /// assert!(sketch.depth() >= 4);
    /// ```
    pub fn new(epsilon: f64, delta: f64) -> Result<Self, Error> {
        Self::with_hasher(epsilon, delta, DefaultHashBuilder)
    }

    /// Creates a sketch with an explicit `width` and `depth`, using the default
    /// hasher.
    ///
    /// # Parameters
    ///
    /// - `width`: counters per row. Must be non-zero.
    /// - `depth`: number of rows (independent hashes). Must be non-zero.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if either argument is zero, or
    /// `width * depth` overflows or cannot be allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CountMinSketch;
    ///
    /// let sketch = CountMinSketch::<u32>::with_dimensions(2_048, 5).unwrap();
    /// assert_eq!(sketch.width(), 2_048);
    /// assert_eq!(sketch.depth(), 5);
    /// ```
    pub fn with_dimensions(width: usize, depth: usize) -> Result<Self, Error> {
        Self::with_dimensions_and_hasher(width, depth, DefaultHashBuilder)
    }
}

impl<T: ?Sized, S: BuildHasher> CountMinSketch<T, S> {
    /// Creates a sketch from `epsilon`/`delta` with a caller-supplied hasher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if either argument is not a finite
    /// value in `(0.0, 1.0)`, or the counter grid they imply cannot be
    /// allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] {
    /// use std::collections::hash_map::RandomState;
    /// use bloom_lib::CountMinSketch;
    ///
    /// let sketch: CountMinSketch<&str, RandomState> =
    ///     CountMinSketch::with_hasher(0.01, 0.01, RandomState::new()).unwrap();
    /// # }
    /// ```
    pub fn with_hasher(epsilon: f64, delta: f64, hasher: S) -> Result<Self, Error> {
        if !(epsilon.is_finite() && epsilon > 0.0 && epsilon < 1.0) {
            return Err(Error::InvalidParameter {
                param: "epsilon",
                reason: "must be a finite value in the open interval (0.0, 1.0)",
            });
        }
        if !(delta.is_finite() && delta > 0.0 && delta < 1.0) {
            return Err(Error::InvalidParameter {
                param: "delta",
                reason: "must be a finite value in the open interval (0.0, 1.0)",
            });
        }

        let width = libm::ceil(E / epsilon) as usize;
        let depth = libm::ceil(libm::log(1.0 / delta)) as usize;
        Self::with_dimensions_and_hasher(width.max(1), depth.max(1), hasher)
    }

    /// Creates a sketch with an explicit geometry and a caller-supplied hasher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `width` or `depth` is zero, or
    /// `width * depth` overflows or cannot be allocated.
    pub fn with_dimensions_and_hasher(
        width: usize,
        depth: usize,
        hasher: S,
    ) -> Result<Self, Error> {
        if width == 0 {
            return Err(Error::InvalidParameter {
                param: "width",
                reason: "must be greater than zero",
            });
        }
        if depth == 0 {
            return Err(Error::InvalidParameter {
                param: "depth",
                reason: "must be greater than zero",
            });
        }

        let cells = width.checked_mul(depth).ok_or(Error::InvalidParameter {
            param: "width",
            reason: TOO_LARGE,
        })?;

        Ok(Self {
            counters: filled_vec(0u64, cells, "width")?,
            width,
            depth,
            total: 0,
            hasher,
            _marker: PhantomData,
        })
    }

    /// Records `count` additional occurrences of `item`.
    ///
    /// Counters saturate at [`u64::MAX`] rather than overflowing, so an
    /// adversarial or runaway stream degrades gracefully instead of panicking or
    /// wrapping.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CountMinSketch;
    ///
    /// let mut sketch = CountMinSketch::new(0.01, 0.01).unwrap();
    /// sketch.add("page-view", 250);
    /// assert!(sketch.estimate("page-view") >= 250);
    /// ```
    pub fn add(&mut self, item: &T, count: u64)
    where
        T: core::hash::Hash,
    {
        let pair = HashPair::new(item, &self.hasher);
        let width = self.width as u64;
        for row in 0..self.depth {
            let column = reduce(pair.nth(row as u64), width) as usize;
            let cell = &mut self.counters[row * self.width + column];
            *cell = cell.saturating_add(count);
        }
        self.total = self.total.saturating_add(count);
    }

    /// Records a single occurrence of `item`. Shorthand for `add(item, 1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CountMinSketch;
    ///
    /// let mut sketch = CountMinSketch::new(0.01, 0.01).unwrap();
    /// sketch.increment("hit");
    /// sketch.increment("hit");
    /// assert!(sketch.estimate("hit") >= 2);
    /// ```
    #[inline]
    pub fn increment(&mut self, item: &T)
    where
        T: core::hash::Hash,
    {
        self.add(item, 1);
    }

    /// Estimates the number of times `item` has been added.
    ///
    /// The estimate is the minimum counter across all rows. It never undercounts
    /// the true total and overcounts only by the sketch's error bound.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CountMinSketch;
    ///
    /// let mut sketch = CountMinSketch::new(0.001, 0.001).unwrap();
    /// for _ in 0..100 {
    ///     sketch.increment("frequent");
    /// }
    /// let estimate = sketch.estimate("frequent");
    /// assert!((100..=110).contains(&estimate));
    /// ```
    #[must_use]
    pub fn estimate(&self, item: &T) -> u64
    where
        T: core::hash::Hash,
    {
        let pair = HashPair::new(item, &self.hasher);
        let width = self.width as u64;
        let mut min = u64::MAX;
        for row in 0..self.depth {
            let column = reduce(pair.nth(row as u64), width) as usize;
            let value = self.counters[row * self.width + column];
            if value < min {
                min = value;
            }
        }
        min
    }

    /// The sum of every count added (saturating).
    ///
    /// Unlike per-item estimates, this is exact up to saturation, because every
    /// `add` contributes to it directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CountMinSketch;
    ///
    /// let mut sketch = CountMinSketch::new(0.01, 0.01).unwrap();
    /// sketch.add("a", 3);
    /// sketch.add("b", 7);
    /// assert_eq!(sketch.total_count(), 10);
    /// ```
    #[inline]
    #[must_use]
    pub fn total_count(&self) -> u64 {
        self.total
    }

    /// The number of counters per row.
    #[inline]
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// The number of rows (independent hash functions).
    #[inline]
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Resets every counter to zero, retaining the allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CountMinSketch;
    ///
    /// let mut sketch = CountMinSketch::new(0.01, 0.01).unwrap();
    /// sketch.increment("x");
    /// sketch.clear();
    /// assert_eq!(sketch.estimate("x"), 0);
    /// assert_eq!(sketch.total_count(), 0);
    /// ```
    pub fn clear(&mut self) {
        self.counters.iter_mut().for_each(|cell| *cell = 0);
        self.total = 0;
    }

    /// Merges `other` into `self` by summing counters cell by cell (saturating).
    ///
    /// After the merge, the sketch estimates frequencies as if every item from
    /// both sketches had been added to one. Both sketches must share their
    /// geometry.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IncompatibleParameters`] if the two sketches differ in
    /// width or depth.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CountMinSketch;
    ///
    /// let mut a = CountMinSketch::with_dimensions(512, 4).unwrap();
    /// let mut b = CountMinSketch::with_dimensions(512, 4).unwrap();
    /// a.add("shared", 2);
    /// b.add("shared", 3);
    ///
    /// a.merge(&b).unwrap();
    /// assert!(a.estimate("shared") >= 5);
    /// ```
    pub fn merge(&mut self, other: &Self) -> Result<(), Error> {
        if self.width != other.width || self.depth != other.depth {
            return Err(Error::IncompatibleParameters);
        }
        for (dst, src) in self.counters.iter_mut().zip(other.counters.iter()) {
            *dst = dst.saturating_add(*src);
        }
        self.total = self.total.saturating_add(other.total);
        Ok(())
    }
}

impl<T: ?Sized, S> CountMinSketch<T, S> {
    /// Returns the same sketch with its hasher replaced by `hasher`.
    ///
    /// Serialized state does not include the hasher, so this is how a sketch
    /// built with a keyed hasher is restored: deserialize it with the default
    /// hasher type, then attach the original key. The counters are kept as
    /// they are; nothing is rehashed. `hasher` must hash exactly like the one
    /// the sketch was built with, or estimates become meaningless.
    #[must_use]
    pub fn replace_hasher<S2: BuildHasher>(self, hasher: S2) -> CountMinSketch<T, S2> {
        CountMinSketch {
            counters: self.counters,
            width: self.width,
            depth: self.depth,
            total: self.total,
            hasher,
            _marker: PhantomData,
        }
    }
}

#[cfg(feature = "serde")]
impl<'de, T: ?Sized, S: Default> serde::Deserialize<'de> for CountMinSketch<T, S> {
    /// Decodes a sketch and checks it: `width` and `depth` must be non-zero
    /// and the counter count must equal `width * depth`. The depth, and so
    /// the work per query, is therefore bounded by the size of the input.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use crate::util::invalid;

        /// The serialized form, decoded before any invariant is checked.
        #[derive(serde::Deserialize)]
        #[serde(rename = "CountMinSketch")]
        struct Raw {
            counters: Vec<u64>,
            width: usize,
            depth: usize,
            total: u64,
        }

        let raw = Raw::deserialize(deserializer)?;
        if raw.width == 0 {
            return Err(invalid("width", "must be greater than zero"));
        }
        if raw.depth == 0 {
            return Err(invalid("depth", "must be greater than zero"));
        }
        if raw.width.checked_mul(raw.depth) != Some(raw.counters.len()) {
            return Err(invalid(
                "counters",
                "counter count does not match width * depth",
            ));
        }
        Ok(Self {
            counters: raw.counters,
            width: raw.width,
            depth: raw.depth,
            total: raw.total,
            hasher: S::default(),
            _marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_new_rejects_out_of_range() {
        assert!(matches!(
            CountMinSketch::<&str>::new(0.0, 0.1),
            Err(Error::InvalidParameter { .. })
        ));
        assert!(matches!(
            CountMinSketch::<&str>::new(0.1, 1.0),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_with_dimensions_rejects_zero() {
        assert!(matches!(
            CountMinSketch::<u8>::with_dimensions(0, 4),
            Err(Error::InvalidParameter { .. })
        ));
        assert!(matches!(
            CountMinSketch::<u8>::with_dimensions(64, 0),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_estimate_never_undercounts() {
        let mut sketch = CountMinSketch::new(0.001, 0.001).unwrap();
        for i in 0..1_000u32 {
            let count = u64::from(i % 7) + 1;
            sketch.add(&i, count);
        }
        for i in 0..1_000u32 {
            let truth = u64::from(i % 7) + 1;
            assert!(
                sketch.estimate(&i) >= truth,
                "estimate undercounted item {i}"
            );
        }
    }

    #[test]
    fn test_absent_item_estimates_low() {
        let mut sketch = CountMinSketch::new(0.001, 0.001).unwrap();
        for i in 0..100u32 {
            sketch.increment(&i);
        }
        // An item never added estimates zero in a lightly-loaded sketch.
        assert_eq!(sketch.estimate(&9_999u32), 0);
    }

    #[test]
    fn test_total_count_is_exact() {
        let mut sketch = CountMinSketch::new(0.01, 0.01).unwrap();
        sketch.add("a", 10);
        sketch.add("b", 20);
        sketch.increment("c");
        assert_eq!(sketch.total_count(), 31);
    }

    #[test]
    fn test_saturating_add() {
        let mut sketch = CountMinSketch::<str>::with_dimensions(16, 2).unwrap();
        sketch.add("x", u64::MAX);
        sketch.add("x", 5);
        assert_eq!(sketch.estimate("x"), u64::MAX);
        assert_eq!(sketch.total_count(), u64::MAX);
    }

    #[test]
    fn test_clear() {
        let mut sketch = CountMinSketch::new(0.01, 0.01).unwrap();
        sketch.add("x", 9);
        sketch.clear();
        assert_eq!(sketch.estimate("x"), 0);
        assert_eq!(sketch.total_count(), 0);
    }

    #[test]
    fn test_merge_sums_counts() {
        let mut a = CountMinSketch::with_dimensions(512, 4).unwrap();
        let mut b = CountMinSketch::with_dimensions(512, 4).unwrap();
        a.add("shared", 2);
        b.add("shared", 3);
        a.merge(&b).unwrap();
        assert!(a.estimate("shared") >= 5);
        assert_eq!(a.total_count(), 5);
    }

    #[test]
    fn test_merge_rejects_incompatible() {
        let mut a = CountMinSketch::<&str>::with_dimensions(512, 4).unwrap();
        let b = CountMinSketch::<&str>::with_dimensions(256, 4).unwrap();
        assert_eq!(a.merge(&b), Err(Error::IncompatibleParameters));
    }
}
