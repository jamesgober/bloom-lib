//! A classic Bloom filter with a tunable false-positive rate.

use core::{hash::BuildHasher, marker::PhantomData};

use crate::{
    bit_set::BitSet,
    hash::{reduce, DefaultHashBuilder, HashPair},
    Error,
};

/// Natural logarithm of 2, reused by the sizing formulas.
const LN_2: f64 = core::f64::consts::LN_2;

/// Largest accepted hash count.
///
/// Each query probes `num_hashes` bits, so the count bounds the cost of a
/// query. The sizing formulas never produce more than about 1,075 (the hash
/// count for the smallest positive `f64` rate), so this cap only rejects
/// geometries no constructor would choose and hostile serialized state.
pub(crate) const MAX_NUM_HASHES: u32 = 2048;

/// A space-efficient probabilistic set membership test.
///
/// A Bloom filter answers "have I seen this item?" using a fraction of the
/// memory a real set would need. The trade-off is one-sided error:
/// [`contains`](Self::contains) never reports a false negative (an inserted
/// item always tests positive) but may report a false positive (an item that
/// was never inserted may test positive). The probability of a false positive
/// is tunable at construction time.
///
/// Items are not stored, so a Bloom filter cannot enumerate or remove its
/// contents. When deletion is required, reach for
/// [`CuckooFilter`](crate::CuckooFilter) instead.
///
/// The filter is generic over the item type `T` and a
/// [`BuildHasher`](core::hash::BuildHasher) `S`, which defaults to the
/// deterministic [`DefaultHashBuilder`](crate::hash::DefaultHashBuilder).
///
/// # Examples
///
/// ```
/// use bloom_lib::BloomFilter;
///
/// // Size for 10,000 items at a 1% false-positive rate.
/// let mut filter = BloomFilter::new(10_000, 0.01).unwrap();
///
/// filter.insert("alice");
/// filter.insert("bob");
///
/// assert!(filter.contains("alice"));
/// assert!(filter.contains("bob"));
/// assert!(!filter.contains("carol")); // very likely absent
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct BloomFilter<T: ?Sized, S = DefaultHashBuilder> {
    bits: BitSet,
    num_hashes: u32,
    #[cfg_attr(feature = "serde", serde(skip))]
    hasher: S,
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: PhantomData<fn(&T)>,
}

impl<T: ?Sized> BloomFilter<T, DefaultHashBuilder> {
    /// Creates a filter sized for `capacity` items at the target false-positive
    /// `rate`, using the default hasher.
    ///
    /// `capacity` is the number of distinct items you expect to insert; the
    /// false-positive rate is honoured at that fill level and degrades
    /// gracefully beyond it. The bit count and hash count are derived from the
    /// standard Bloom filter formulas.
    ///
    /// # Parameters
    ///
    /// - `capacity`: expected number of distinct insertions. Must be non-zero.
    /// - `rate`: desired false-positive probability. Must lie in `(0.0, 1.0)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `capacity` is zero, `rate` is
    /// not a finite value strictly between `0.0` and `1.0`, or the bit array
    /// the two imply cannot be allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let filter = BloomFilter::<&str>::new(1_000, 0.001).unwrap();
    /// assert!(filter.num_bits() >= 1_000);
    /// assert!(filter.num_hashes() >= 1);
    /// ```
    pub fn new(capacity: usize, rate: f64) -> Result<Self, Error> {
        Self::with_hasher(capacity, rate, DefaultHashBuilder)
    }

    /// Creates a filter with an explicit bit count and hash count, using the
    /// default hasher.
    ///
    /// Use this when you want direct control over the filter's geometry rather
    /// than deriving it from a capacity and rate.
    ///
    /// # Parameters
    ///
    /// - `num_bits`: size of the bit array. Must be non-zero.
    /// - `num_hashes`: number of hash positions probed per item. Must be in
    ///   `1..=2048`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if either argument is zero,
    /// `num_hashes` is above 2048, or the bit array cannot be allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let filter = BloomFilter::<u64>::with_dimensions(8_192, 5).unwrap();
    /// assert_eq!(filter.num_bits(), 8_192);
    /// assert_eq!(filter.num_hashes(), 5);
    /// ```
    pub fn with_dimensions(num_bits: u64, num_hashes: u32) -> Result<Self, Error> {
        Self::with_dimensions_and_hasher(num_bits, num_hashes, DefaultHashBuilder)
    }
}

impl<T: ?Sized, S: BuildHasher> BloomFilter<T, S> {
    /// Creates a filter sized for `capacity` items at the target false-positive
    /// `rate`, using a caller-supplied hasher.
    ///
    /// Identical to [`new`](Self::new) but lets you plug in any
    /// [`BuildHasher`](core::hash::BuildHasher) — for example a randomly-seeded
    /// one for resistance against adversarial inputs.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `capacity` is zero, `rate` is
    /// not a finite value strictly between `0.0` and `1.0`, or the bit array
    /// the two imply cannot be allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] {
    /// use std::collections::hash_map::RandomState;
    /// use bloom_lib::BloomFilter;
    ///
    /// let filter: BloomFilter<&str, RandomState> =
    ///     BloomFilter::with_hasher(1_000, 0.01, RandomState::new()).unwrap();
    /// # }
    /// ```
    pub fn with_hasher(capacity: usize, rate: f64, hasher: S) -> Result<Self, Error> {
        if capacity == 0 {
            return Err(Error::InvalidParameter {
                param: "capacity",
                reason: "must be greater than zero",
            });
        }
        if !(rate.is_finite() && rate > 0.0 && rate < 1.0) {
            return Err(Error::InvalidParameter {
                param: "rate",
                reason: "must be a finite value in the open interval (0.0, 1.0)",
            });
        }

        let num_bits = optimal_num_bits(capacity, rate);
        let num_hashes = optimal_num_hashes(num_bits, capacity);
        Self::with_dimensions_and_hasher(num_bits, num_hashes, hasher)
    }

    /// Creates a filter with an explicit geometry and a caller-supplied hasher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `num_bits` or `num_hashes` is
    /// zero, `num_hashes` is above 2048, or the bit array cannot be
    /// allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::{hash::DefaultHashBuilder, BloomFilter};
    ///
    /// let filter = BloomFilter::<u64>::with_dimensions_and_hasher(
    ///     4_096,
    ///     4,
    ///     DefaultHashBuilder,
    /// )
    /// .unwrap();
    /// assert_eq!(filter.num_hashes(), 4);
    /// ```
    pub fn with_dimensions_and_hasher(
        num_bits: u64,
        num_hashes: u32,
        hasher: S,
    ) -> Result<Self, Error> {
        if num_bits == 0 {
            return Err(Error::InvalidParameter {
                param: "num_bits",
                reason: "must be greater than zero",
            });
        }
        check_num_hashes(num_hashes)?;

        Ok(Self {
            bits: BitSet::try_new(num_bits)?,
            num_hashes,
            hasher,
            _marker: PhantomData,
        })
    }

    /// Inserts `item`, returning `true` if it was probably not present before.
    ///
    /// A return of `true` means at least one of the item's bits was previously
    /// unset, so the item had definitely not been inserted. A return of `false`
    /// means every bit was already set, so the item was *probably* already
    /// present (subject to the filter's false-positive rate). This makes
    /// `insert` a convenient deduplicating primitive for streaming input.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let mut filter = BloomFilter::new(100, 0.01).unwrap();
    /// assert!(filter.insert("first")); // newly added
    /// assert!(!filter.insert("first")); // already present
    /// ```
    pub fn insert(&mut self, item: &T) -> bool
    where
        T: core::hash::Hash,
    {
        let pair = HashPair::new(item, &self.hasher);
        let num_bits = self.bits.len();
        let mut newly_added = false;
        for i in 0..u64::from(self.num_hashes) {
            let index = reduce(pair.nth(i), num_bits);
            // `set` returns the previous value; a `false` means this bit was
            // unset, so the item is definitely new.
            if !self.bits.set(index) {
                newly_added = true;
            }
        }
        newly_added
    }

    /// Tests whether `item` is in the filter.
    ///
    /// Returns `false` only if the item was definitely never inserted. A return
    /// of `true` means the item is *probably* present, with the filter's
    /// configured false-positive probability.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let mut filter = BloomFilter::new(100, 0.01).unwrap();
    /// filter.insert(&7u32);
    /// assert!(filter.contains(&7u32));
    /// assert!(!filter.contains(&99u32));
    /// ```
    #[must_use]
    pub fn contains(&self, item: &T) -> bool
    where
        T: core::hash::Hash,
    {
        let pair = HashPair::new(item, &self.hasher);
        let num_bits = self.bits.len();
        (0..u64::from(self.num_hashes)).all(|i| self.bits.get(reduce(pair.nth(i), num_bits)))
    }

    /// Removes every item, leaving an empty filter with the same geometry.
    ///
    /// The bit allocation is retained, so reusing a cleared filter avoids a
    /// fresh allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let mut filter = BloomFilter::new(100, 0.01).unwrap();
    /// filter.insert("x");
    /// filter.clear();
    /// assert!(filter.is_empty());
    /// assert!(!filter.contains("x"));
    /// ```
    pub fn clear(&mut self) {
        self.bits.clear();
    }

    /// The size of the underlying bit array.
    #[inline]
    #[must_use]
    pub fn num_bits(&self) -> u64 {
        self.bits.len()
    }

    /// The number of hash positions probed per item.
    #[inline]
    #[must_use]
    pub fn num_hashes(&self) -> u32 {
        self.num_hashes
    }

    /// The number of set bits in the filter.
    ///
    /// This is the raw population count of the bit array, useful for monitoring
    /// fill level. See [`estimated_len`](Self::estimated_len) for an estimate of
    /// the number of distinct items inserted.
    #[inline]
    #[must_use]
    pub fn count_ones(&self) -> u64 {
        self.bits.count_ones()
    }

    /// Returns `true` if no bits are set, i.e. nothing has been inserted.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let mut filter = BloomFilter::new(100, 0.01).unwrap();
    /// assert!(filter.is_empty());
    /// filter.insert("x");
    /// assert!(!filter.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bits.count_ones() == 0
    }

    /// Estimates the number of distinct items inserted so far.
    ///
    /// Derived from the fraction of set bits using the standard estimator
    /// `n ≈ -(m / k) · ln(1 − X / m)`, where `m` is the bit count, `k` the hash
    /// count, and `X` the number of set bits. The estimate is approximate and
    /// loses accuracy as the filter saturates.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let mut filter = BloomFilter::new(10_000, 0.01).unwrap();
    /// for i in 0..1_000u32 {
    ///     filter.insert(&i);
    /// }
    /// let estimate = filter.estimated_len();
    /// // Within a few percent of the true count of 1,000.
    /// assert!((900..=1_100).contains(&estimate));
    /// ```
    #[must_use]
    pub fn estimated_len(&self) -> u64 {
        let m = self.bits.len() as f64;
        let k = f64::from(self.num_hashes);
        let x = self.bits.count_ones() as f64;
        if x == 0.0 {
            return 0;
        }
        if x >= m {
            // Saturated: the estimator diverges, so report the bit count as a
            // conservative floor rather than infinity.
            return self.bits.len();
        }
        let estimate = -(m / k) * libm::log(1.0 - x / m);
        // `estimate` is finite and non-negative here; round to the nearest item.
        libm::round(estimate) as u64
    }

    /// Estimates the current false-positive probability at the present fill
    /// level.
    ///
    /// Computed as `(X / m)^k`, where `X` is the number of set bits, `m` the bit
    /// count, and `k` the hash count. This reflects the *actual* fill, so it
    /// rises above the configured target rate once the filter holds more than
    /// its design capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let filter = BloomFilter::<u32>::new(1_000, 0.01).unwrap();
    /// // An empty filter cannot produce a false positive.
    /// assert_eq!(filter.estimated_false_positive_rate(), 0.0);
    /// ```
    #[must_use]
    pub fn estimated_false_positive_rate(&self) -> f64 {
        let m = self.bits.len() as f64;
        let k = f64::from(self.num_hashes);
        let fill = self.bits.count_ones() as f64 / m;
        libm::pow(fill, k)
    }

    /// Merges `other` into `self` by unioning their bit arrays.
    ///
    /// After a successful merge, the filter reports positive for every item
    /// that was in either operand. Both filters must have been built with
    /// identical dimensions (bit count and hash count); the hasher is assumed to
    /// match, which holds automatically for the default hasher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IncompatibleParameters`] if the two filters differ in
    /// bit count or hash count.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::BloomFilter;
    ///
    /// let mut a = BloomFilter::new(1_000, 0.01).unwrap();
    /// let mut b = BloomFilter::new(1_000, 0.01).unwrap();
    /// a.insert("from-a");
    /// b.insert("from-b");
    ///
    /// a.merge(&b).unwrap();
    /// assert!(a.contains("from-a"));
    /// assert!(a.contains("from-b"));
    /// ```
    pub fn merge(&mut self, other: &Self) -> Result<(), Error> {
        if self.num_hashes != other.num_hashes || !self.bits.is_compatible(&other.bits) {
            return Err(Error::IncompatibleParameters);
        }
        self.bits.union_with(&other.bits);
        Ok(())
    }
}

impl<T: ?Sized, S> BloomFilter<T, S> {
    /// Returns the same filter with its hasher replaced by `hasher`.
    ///
    /// Serialized state does not include the hasher, so this is how a filter
    /// built with a keyed hasher is restored: deserialize it with the default
    /// hasher type, then attach the original key. The bits are kept as they
    /// are; nothing is rehashed. `hasher` must hash exactly like the one the
    /// filter was built with, or lookups give false negatives.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::{hash::KeyedHashBuilder, BloomFilter};
    ///
    /// let key = [3u8; 16];
    /// let mut filter = BloomFilter::<str, _>::with_hasher(100, 0.01, KeyedHashBuilder::new(key))
    ///     .unwrap();
    /// filter.insert("kept");
    ///
    /// // For example after a serde round trip into `BloomFilter<str>`:
    /// let plain: BloomFilter<str> = filter.replace_hasher(Default::default());
    /// let restored = plain.replace_hasher(KeyedHashBuilder::new(key));
    /// assert!(restored.contains("kept"));
    /// ```
    #[must_use]
    pub fn replace_hasher<S2: BuildHasher>(self, hasher: S2) -> BloomFilter<T, S2> {
        BloomFilter {
            bits: self.bits,
            num_hashes: self.num_hashes,
            hasher,
            _marker: PhantomData,
        }
    }
}

/// Checks a hash count against `1..=MAX_NUM_HASHES`.
fn check_num_hashes(num_hashes: u32) -> Result<(), Error> {
    if num_hashes == 0 {
        return Err(Error::InvalidParameter {
            param: "num_hashes",
            reason: "must be greater than zero",
        });
    }
    if num_hashes > MAX_NUM_HASHES {
        return Err(Error::InvalidParameter {
            param: "num_hashes",
            reason: "must be at most 2048",
        });
    }
    Ok(())
}

#[cfg(feature = "serde")]
impl<'de, T: ?Sized, S: Default> serde::Deserialize<'de> for BloomFilter<T, S> {
    /// Decodes a filter and checks it: the bit set's word count must match
    /// its bit count, no bit beyond the bit count may be set, and the hash
    /// count must be in `1..=2048`. Inconsistent input is an error, never a
    /// filter that panics or stalls on its first query.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// The serialized form, decoded before the hash count is checked.
        #[derive(serde::Deserialize)]
        #[serde(rename = "BloomFilter")]
        struct Raw {
            bits: BitSet,
            num_hashes: u32,
        }

        let raw = Raw::deserialize(deserializer)?;
        check_num_hashes(raw.num_hashes).map_err(serde::de::Error::custom)?;
        Ok(Self {
            bits: raw.bits,
            num_hashes: raw.num_hashes,
            hasher: S::default(),
            _marker: PhantomData,
        })
    }
}

/// Optimal bit count `m = ceil(-n·ln(p) / (ln2)^2)`, clamped to at least one.
fn optimal_num_bits(capacity: usize, rate: f64) -> u64 {
    let n = capacity as f64;
    let m = -(n * libm::log(rate)) / (LN_2 * LN_2);
    let rounded = libm::ceil(m);
    if rounded < 1.0 {
        1
    } else {
        rounded as u64
    }
}

/// Optimal hash count `k = round((m/n)·ln2)`, clamped to at least one.
fn optimal_num_hashes(num_bits: u64, capacity: usize) -> u32 {
    let k = (num_bits as f64 / capacity as f64) * LN_2;
    let rounded = libm::round(k);
    if rounded < 1.0 {
        1
    } else {
        // `m` is bounded by realistic memory, so `k` never approaches u32::MAX.
        rounded as u32
    }
}

#[cfg(test)]
mod tests {
    // `insert` returns a novelty flag that most call sites here intentionally
    // discard; the crate denies `unused_results` for shipping code.
    #![allow(unused_results)]
    // Unwrapping known-valid constructor results keeps the tests readable.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_new_rejects_zero_capacity() {
        let err = BloomFilter::<&str>::new(0, 0.01).unwrap_err();
        assert_eq!(
            err,
            Error::InvalidParameter {
                param: "capacity",
                reason: "must be greater than zero"
            }
        );
    }

    #[test]
    fn test_new_rejects_out_of_range_rate() {
        assert!(matches!(
            BloomFilter::<&str>::new(10, 0.0),
            Err(Error::InvalidParameter { .. })
        ));
        assert!(matches!(
            BloomFilter::<&str>::new(10, 1.0),
            Err(Error::InvalidParameter { .. })
        ));
        assert!(matches!(
            BloomFilter::<&str>::new(10, f64::NAN),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_with_dimensions_rejects_zeros() {
        assert!(matches!(
            BloomFilter::<u8>::with_dimensions(0, 3),
            Err(Error::InvalidParameter { .. })
        ));
        assert!(matches!(
            BloomFilter::<u8>::with_dimensions(64, 0),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_no_false_negatives() {
        let mut filter = BloomFilter::new(1_000, 0.01).unwrap();
        for i in 0..1_000u32 {
            filter.insert(&i);
        }
        for i in 0..1_000u32 {
            assert!(filter.contains(&i), "inserted item {i} reported absent");
        }
    }

    #[test]
    fn test_insert_reports_novelty() {
        let mut filter = BloomFilter::new(100, 0.01).unwrap();
        assert!(filter.insert("alpha"));
        assert!(!filter.insert("alpha"));
    }

    #[test]
    fn test_false_positive_rate_is_near_target() {
        let capacity = 10_000;
        let target = 0.01;
        let mut filter = BloomFilter::new(capacity, target).unwrap();
        for i in 0..capacity as u64 {
            filter.insert(&i);
        }
        // Query a disjoint range and measure the observed false-positive rate.
        let trials = 100_000u64;
        let mut hits = 0u64;
        for i in capacity as u64..capacity as u64 + trials {
            if filter.contains(&i) {
                hits += 1;
            }
        }
        let observed = hits as f64 / trials as f64;
        // Allow generous headroom; the point is it is the right order of magnitude.
        assert!(
            observed < target * 3.0,
            "observed FP rate {observed} far exceeds target {target}"
        );
    }

    #[test]
    fn test_clear_empties_filter() {
        let mut filter = BloomFilter::new(100, 0.01).unwrap();
        filter.insert("x");
        assert!(!filter.is_empty());
        filter.clear();
        assert!(filter.is_empty());
        assert!(!filter.contains("x"));
    }

    #[test]
    fn test_merge_unions_membership() {
        let mut a = BloomFilter::new(1_000, 0.01).unwrap();
        let mut b = BloomFilter::new(1_000, 0.01).unwrap();
        a.insert("a");
        b.insert("b");
        a.merge(&b).unwrap();
        assert!(a.contains("a"));
        assert!(a.contains("b"));
    }

    #[test]
    fn test_merge_rejects_incompatible() {
        let mut a = BloomFilter::<u32>::with_dimensions(1_024, 3).unwrap();
        let b = BloomFilter::<u32>::with_dimensions(2_048, 3).unwrap();
        assert_eq!(a.merge(&b), Err(Error::IncompatibleParameters));

        let c = BloomFilter::<u32>::with_dimensions(1_024, 4).unwrap();
        assert_eq!(a.merge(&c), Err(Error::IncompatibleParameters));
    }

    #[test]
    fn test_estimated_len_is_reasonable() {
        let mut filter = BloomFilter::new(10_000, 0.01).unwrap();
        for i in 0..1_000u32 {
            filter.insert(&i);
        }
        let estimate = filter.estimated_len();
        assert!(
            (900..=1_100).contains(&estimate),
            "estimate {estimate} not within 10% of 1000"
        );
    }

    #[test]
    fn test_estimated_len_empty_is_zero() {
        let filter = BloomFilter::<u32>::new(1_000, 0.01).unwrap();
        assert_eq!(filter.estimated_len(), 0);
    }

    #[test]
    fn test_sizing_formulas() {
        // 10k items at 1% -> ~95,851 bits, k = 7 (textbook values).
        let bits = optimal_num_bits(10_000, 0.01);
        assert!(
            (95_000..=96_500).contains(&bits),
            "unexpected bit count {bits}"
        );
        let k = optimal_num_hashes(bits, 10_000);
        assert_eq!(k, 7);
    }
}
