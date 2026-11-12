//! A HyperLogLog estimator for set cardinality.

use core::{hash::BuildHasher, marker::PhantomData};

use alloc::{vec, vec::Vec};

use crate::{hash::DefaultHashBuilder, Error};

/// Smallest supported precision (16 registers).
const MIN_PRECISION: u8 = 4;

/// Largest supported precision (262,144 registers, ~256 KiB).
const MAX_PRECISION: u8 = 18;

/// Estimates the number of *distinct* items in a stream in fixed, tiny memory.
///
/// HyperLogLog counts unique items without storing them. It hashes each item,
/// uses the leading bits to pick one of `2^p` registers, and records the longest
/// run of leading zeros seen in the remaining bits — a quantity that grows
/// logarithmically with the number of distinct items hitting that register.
/// Combining the registers with a bias-corrected harmonic mean yields a
/// cardinality estimate with a standard error of about `1.04 / sqrt(2^p)`.
///
/// Memory is `2^p` bytes regardless of how many items are inserted, so a
/// precision-14 estimator counts billions of distinct items in 16 KiB with a
/// typical error under 1%.
///
/// The estimator is generic over the item type `T` and a
/// [`BuildHasher`](core::hash::BuildHasher) `S`, defaulting to the deterministic
/// [`DefaultHashBuilder`](crate::hash::DefaultHashBuilder).
///
/// # Examples
///
/// ```
/// use bloom_lib::HyperLogLog;
///
/// let mut hll = HyperLogLog::new(14).unwrap();
/// for i in 0..100_000u32 {
///     hll.insert(&i);
/// }
///
/// // Within a couple of percent of the true cardinality of 100,000.
/// let estimate = hll.count();
/// assert!((97_000..=103_000).contains(&estimate));
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct HyperLogLog<T: ?Sized, S = DefaultHashBuilder> {
    registers: Vec<u8>,
    precision: u8,
    #[cfg_attr(feature = "serde", serde(skip))]
    hasher: S,
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: PhantomData<fn(&T)>,
}

impl<T: ?Sized> HyperLogLog<T, DefaultHashBuilder> {
    /// Creates an estimator with the given `precision`, using the default
    /// hasher.
    ///
    /// The estimator uses `2^precision` one-byte registers and has a standard
    /// error of roughly `1.04 / sqrt(2^precision)`. Precision 14 (16 KiB, ~0.8%
    /// error) is a common production choice.
    ///
    /// # Parameters
    ///
    /// - `precision`: the log2 of the register count. Must be in `4..=18`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `precision` is outside `4..=18`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::HyperLogLog;
    ///
    /// let hll = HyperLogLog::<&str>::new(12).unwrap();
    /// assert_eq!(hll.precision(), 12);
    /// assert!(hll.is_empty());
    /// ```
    pub fn new(precision: u8) -> Result<Self, Error> {
        Self::with_hasher(precision, DefaultHashBuilder)
    }

    /// Creates an estimator sized for a target relative `error`, using the
    /// default hasher.
    ///
    /// The precision is chosen as the smallest value whose standard error does
    /// not exceed `error`, then clamped to the supported `4..=18` range.
    ///
    /// # Parameters
    ///
    /// - `error`: the desired standard error, in `(0.0, 1.0)`. For example
    ///   `0.01` targets ~1% error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `error` is not a finite value in
    /// `(0.0, 1.0)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::HyperLogLog;
    ///
    /// let hll = HyperLogLog::<&str>::with_error_rate(0.01).unwrap();
    /// // ~1% error needs about 2^14 registers.
    /// assert_eq!(hll.precision(), 14);
    /// ```
    pub fn with_error_rate(error: f64) -> Result<Self, Error> {
        if !(error.is_finite() && error > 0.0 && error < 1.0) {
            return Err(Error::InvalidParameter {
                param: "error",
                reason: "must be a finite value in the open interval (0.0, 1.0)",
            });
        }
        let registers = libm::pow(1.04 / error, 2.0);
        let raw = libm::ceil(libm::log2(registers)) as i64;
        let precision = raw.clamp(i64::from(MIN_PRECISION), i64::from(MAX_PRECISION)) as u8;
        Self::new(precision)
    }
}

impl<T: ?Sized, S: BuildHasher> HyperLogLog<T, S> {
    /// Creates an estimator with the given `precision` and a caller-supplied
    /// hasher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `precision` is outside `4..=18`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] {
    /// use std::collections::hash_map::RandomState;
    /// use bloom_lib::HyperLogLog;
    ///
    /// let hll: HyperLogLog<&str, RandomState> =
    ///     HyperLogLog::with_hasher(14, RandomState::new()).unwrap();
    /// # }
    /// ```
    pub fn with_hasher(precision: u8, hasher: S) -> Result<Self, Error> {
        if !(MIN_PRECISION..=MAX_PRECISION).contains(&precision) {
            return Err(Error::InvalidParameter {
                param: "precision",
                reason: "must be in the range 4..=18",
            });
        }
        let num_registers = 1usize << precision;
        Ok(Self {
            registers: vec![0u8; num_registers],
            precision,
            hasher,
            _marker: PhantomData,
        })
    }

    /// Adds `item` to the estimator.
    ///
    /// Inserting an item already counted has no effect on the estimate, so the
    /// operation is idempotent with respect to distinct values.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::HyperLogLog;
    ///
    /// let mut hll = HyperLogLog::new(14).unwrap();
    /// hll.insert("first");
    /// hll.insert("first"); // no additional effect
    /// hll.insert("second");
    /// assert_eq!(hll.count(), 2);
    /// ```
    pub fn insert(&mut self, item: &T)
    where
        T: core::hash::Hash,
    {
        let hash = self.hasher.hash_one(item);
        let p = u32::from(self.precision);
        // Top `p` bits select the register.
        let index = (hash >> (64 - p)) as usize;
        // Rank = 1 + leading zeros of the remaining bits. Setting the low `p`
        // bits ensures an all-zero remainder yields the maximum rank `64-p+1`
        // rather than counting the shifted-in zeros.
        let remainder = (hash << p) | ((1u64 << p) - 1);
        let rank = (remainder.leading_zeros() + 1) as u8;
        if rank > self.registers[index] {
            self.registers[index] = rank;
        }
    }

    /// Returns the estimated number of distinct items inserted.
    ///
    /// Uses the bias-corrected HyperLogLog estimator, falling back to linear
    /// counting at low cardinalities where the raw estimate is unreliable. The
    /// result is approximate, with the standard error implied by the precision.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::HyperLogLog;
    ///
    /// let mut hll = HyperLogLog::new(14).unwrap();
    /// assert_eq!(hll.count(), 0);
    /// for i in 0..1_000u32 {
    ///     hll.insert(&i);
    /// }
    /// let estimate = hll.count();
    /// assert!((960..=1_040).contains(&estimate));
    /// ```
    #[must_use]
    pub fn count(&self) -> u64 {
        let m = self.registers.len() as f64;
        let mut sum = 0.0f64;
        let mut zeros = 0u64;
        for &register in &self.registers {
            sum += libm::exp2(-f64::from(register));
            if register == 0 {
                zeros += 1;
            }
        }

        let raw = alpha(self.registers.len()) * m * m / sum;

        // Linear counting is more accurate than the raw estimate when the table
        // is sparsely populated.
        if raw <= 2.5 * m && zeros > 0 {
            let linear = m * libm::log(m / zeros as f64);
            return libm::round(linear) as u64;
        }

        libm::round(raw) as u64
    }

    /// Returns `true` if no items have been inserted.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registers.iter().all(|&register| register == 0)
    }

    /// The configured precision (log2 of the register count).
    #[inline]
    #[must_use]
    pub fn precision(&self) -> u8 {
        self.precision
    }

    /// Resets the estimator to empty, retaining the allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::HyperLogLog;
    ///
    /// let mut hll = HyperLogLog::new(14).unwrap();
    /// hll.insert("x");
    /// hll.clear();
    /// assert!(hll.is_empty());
    /// assert_eq!(hll.count(), 0);
    /// ```
    pub fn clear(&mut self) {
        self.registers.iter_mut().for_each(|register| *register = 0);
    }

    /// Merges `other` into `self` by taking the register-wise maximum.
    ///
    /// The result estimates the cardinality of the *union* of the two streams.
    /// Both estimators must share the same precision.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IncompatibleParameters`] if the precisions differ.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::HyperLogLog;
    ///
    /// let mut a = HyperLogLog::new(14).unwrap();
    /// let mut b = HyperLogLog::new(14).unwrap();
    /// for i in 0..1_000u32 {
    ///     a.insert(&i);
    /// }
    /// for i in 500..1_500u32 {
    ///     b.insert(&i);
    /// }
    ///
    /// a.merge(&b).unwrap();
    /// // Union of [0,1000) and [500,1500) is [0,1500): ~1,500 distinct.
    /// let estimate = a.count();
    /// assert!((1_400..=1_600).contains(&estimate));
    /// ```
    pub fn merge(&mut self, other: &Self) -> Result<(), Error> {
        if self.precision != other.precision {
            return Err(Error::IncompatibleParameters);
        }
        for (dst, src) in self.registers.iter_mut().zip(other.registers.iter()) {
            *dst = (*dst).max(*src);
        }
        Ok(())
    }
}

impl<T: ?Sized, S> HyperLogLog<T, S> {
    /// Returns the same estimator with its hasher replaced by `hasher`.
    ///
    /// Serialized state does not include the hasher, so this is how an
    /// estimator built with a keyed hasher is restored: deserialize it with
    /// the default hasher type, then attach the original key. The registers
    /// are kept as they are; nothing is rehashed. `hasher` must hash exactly
    /// like the one the estimator was built with, or items already counted
    /// are counted again.
    #[must_use]
    pub fn replace_hasher<S2: BuildHasher>(self, hasher: S2) -> HyperLogLog<T, S2> {
        HyperLogLog {
            registers: self.registers,
            precision: self.precision,
            hasher,
            _marker: PhantomData,
        }
    }
}

#[cfg(feature = "serde")]
impl<'de, T: ?Sized, S: Default> serde::Deserialize<'de> for HyperLogLog<T, S> {
    /// Decodes an estimator and checks it: `precision` must be in `4..=18`,
    /// there must be exactly `2^precision` registers, and no register may
    /// exceed the largest rank an insertion can produce (`65 - precision`).
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use crate::util::invalid;

        /// The serialized form, decoded before any invariant is checked.
        #[derive(serde::Deserialize)]
        #[serde(rename = "HyperLogLog")]
        struct Raw {
            registers: Vec<u8>,
            precision: u8,
        }

        let raw = Raw::deserialize(deserializer)?;
        if !(MIN_PRECISION..=MAX_PRECISION).contains(&raw.precision) {
            return Err(invalid("precision", "must be in the range 4..=18"));
        }
        if raw.registers.len() != 1usize << raw.precision {
            return Err(invalid(
                "registers",
                "register count does not match the precision (expected 2^precision)",
            ));
        }
        let max_rank = 65 - raw.precision;
        if raw.registers.iter().any(|&register| register > max_rank) {
            return Err(invalid(
                "registers",
                "a register exceeds the largest possible rank",
            ));
        }
        Ok(Self {
            registers: raw.registers,
            precision: raw.precision,
            hasher: S::default(),
            _marker: PhantomData,
        })
    }
}

/// The HyperLogLog bias-correction constant `alpha_m` for a register count `m`.
fn alpha(m: usize) -> f64 {
    match m {
        16 => 0.673,
        32 => 0.697,
        64 => 0.709,
        _ => 0.7213 / (1.0 + 1.079 / m as f64),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_new_rejects_out_of_range_precision() {
        assert!(matches!(
            HyperLogLog::<&str>::new(3),
            Err(Error::InvalidParameter { .. })
        ));
        assert!(matches!(
            HyperLogLog::<&str>::new(19),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_with_error_rate_picks_precision() {
        let hll = HyperLogLog::<&str>::with_error_rate(0.01).unwrap();
        assert_eq!(hll.precision(), 14);
        // A tiny target is clamped to the maximum precision.
        let tight = HyperLogLog::<&str>::with_error_rate(0.0001).unwrap();
        assert_eq!(tight.precision(), MAX_PRECISION);
    }

    #[test]
    fn test_empty_counts_zero() {
        let hll = HyperLogLog::<u32>::new(14).unwrap();
        assert!(hll.is_empty());
        assert_eq!(hll.count(), 0);
    }

    #[test]
    fn test_small_cardinality_is_exact_ish() {
        let mut hll = HyperLogLog::new(14).unwrap();
        for i in 0..10u32 {
            hll.insert(&i);
        }
        // Linear counting is very accurate at tiny cardinalities.
        let estimate = hll.count();
        assert!(
            (9..=11).contains(&estimate),
            "estimate {estimate} off for n=10"
        );
    }

    #[test]
    fn test_large_cardinality_within_error() {
        let mut hll = HyperLogLog::new(14).unwrap();
        let n = 100_000u32;
        for i in 0..n {
            hll.insert(&i);
        }
        let estimate = hll.count();
        let error = (estimate as f64 - f64::from(n)).abs() / f64::from(n);
        // Precision 14 has ~0.8% standard error; allow a 3% envelope.
        assert!(
            error < 0.03,
            "relative error {error} too high (est {estimate})"
        );
    }

    #[test]
    fn test_idempotent_inserts() {
        let mut hll = HyperLogLog::new(14).unwrap();
        for _ in 0..1_000 {
            hll.insert("same");
        }
        assert_eq!(hll.count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut hll = HyperLogLog::new(14).unwrap();
        for i in 0..100u32 {
            hll.insert(&i);
        }
        hll.clear();
        assert!(hll.is_empty());
        assert_eq!(hll.count(), 0);
    }

    #[test]
    fn test_merge_estimates_union() {
        let mut a = HyperLogLog::new(14).unwrap();
        let mut b = HyperLogLog::new(14).unwrap();
        for i in 0..1_000u32 {
            a.insert(&i);
        }
        for i in 500..1_500u32 {
            b.insert(&i);
        }
        a.merge(&b).unwrap();
        let estimate = a.count();
        assert!(
            (1_400..=1_600).contains(&estimate),
            "union estimate {estimate}"
        );
    }

    #[test]
    fn test_merge_rejects_incompatible() {
        let mut a = HyperLogLog::<u32>::new(14).unwrap();
        let b = HyperLogLog::<u32>::new(12).unwrap();
        assert_eq!(a.merge(&b), Err(Error::IncompatibleParameters));
    }
}
