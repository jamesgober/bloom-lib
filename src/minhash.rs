//! A MinHash sketch for estimating Jaccard similarity between sets.

use core::{hash::BuildHasher, marker::PhantomData};

use alloc::{vec, vec::Vec};

use crate::{
    hash::{DefaultHashBuilder, HashPair},
    Error,
};

/// A fixed-size signature that estimates the Jaccard similarity of the set it
/// summarises against another sketch.
///
/// MinHash represents a set by the minimum hash value each of `k` hash functions
/// produces over the set's members. The fraction of signature slots two sketches
/// agree on is an unbiased estimate of the
/// [Jaccard similarity](https://en.wikipedia.org/wiki/Jaccard_index)
/// `|A ∩ B| / |A ∪ B|` of the two sets — computed in `O(k)` time and `O(k)`
/// space, independent of how large the sets are. More hash functions tighten the
/// estimate (standard error ≈ `1 / sqrt(k)`).
///
/// The sketch is generic over the item type `T` and a
/// [`BuildHasher`](core::hash::BuildHasher) `S`, defaulting to the deterministic
/// [`DefaultHashBuilder`](crate::hash::DefaultHashBuilder). Two sketches are only
/// comparable when built with the same number of hash functions and the same
/// hasher.
///
/// # Examples
///
/// ```
/// use bloom_lib::MinHash;
///
/// let mut a = MinHash::new(256).unwrap();
/// let mut b = MinHash::new(256).unwrap();
///
/// for w in ["the", "quick", "brown", "fox"] {
///     a.insert(w);
/// }
/// for w in ["the", "quick", "red", "fox"] {
///     b.insert(w);
/// }
///
/// // True Jaccard similarity is 3/5 = 0.6 (shared: the, quick, fox).
/// let similarity = a.similarity(&b).unwrap();
/// assert!((0.45..=0.75).contains(&similarity));
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MinHash<T: ?Sized, S = DefaultHashBuilder> {
    signature: Vec<u64>,
    #[cfg_attr(feature = "serde", serde(skip))]
    hasher: S,
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: PhantomData<fn(&T)>,
}

impl<T: ?Sized> MinHash<T, DefaultHashBuilder> {
    /// Creates a sketch with `num_hashes` hash functions, using the default
    /// hasher.
    ///
    /// More hash functions give a more precise similarity estimate at
    /// proportional memory and update cost. A few hundred is typical; the
    /// standard error of the estimate is about `1 / sqrt(num_hashes)`.
    ///
    /// # Parameters
    ///
    /// - `num_hashes`: the signature length. Must be non-zero.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `num_hashes` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::MinHash;
    ///
    /// let sketch = MinHash::<&str>::new(128).unwrap();
    /// assert_eq!(sketch.num_hashes(), 128);
    /// assert!(sketch.is_empty());
    /// ```
    pub fn new(num_hashes: usize) -> Result<Self, Error> {
        Self::with_hasher(num_hashes, DefaultHashBuilder)
    }
}

impl<T: ?Sized, S: BuildHasher> MinHash<T, S> {
    /// Creates a sketch with `num_hashes` hash functions and a caller-supplied
    /// hasher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `num_hashes` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] {
    /// use std::collections::hash_map::RandomState;
    /// use bloom_lib::MinHash;
    ///
    /// let sketch: MinHash<&str, RandomState> =
    ///     MinHash::with_hasher(128, RandomState::new()).unwrap();
    /// # }
    /// ```
    pub fn with_hasher(num_hashes: usize, hasher: S) -> Result<Self, Error> {
        if num_hashes == 0 {
            return Err(Error::InvalidParameter {
                param: "num_hashes",
                reason: "must be greater than zero",
            });
        }
        Ok(Self {
            signature: vec![u64::MAX; num_hashes],
            hasher,
            _marker: PhantomData,
        })
    }

    /// Adds `item` to the set this sketch summarises.
    ///
    /// Each of the `k` hash functions is evaluated on `item` and the
    /// corresponding signature slot keeps the running minimum. Re-inserting an
    /// item already present has no effect, so the sketch depends only on the
    /// distinct members of the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::MinHash;
    ///
    /// let mut sketch = MinHash::new(64).unwrap();
    /// sketch.insert("member");
    /// assert!(!sketch.is_empty());
    /// ```
    pub fn insert(&mut self, item: &T)
    where
        T: core::hash::Hash,
    {
        let pair = HashPair::new(item, &self.hasher);
        for (i, slot) in self.signature.iter_mut().enumerate() {
            let value = pair.nth(i as u64);
            if value < *slot {
                *slot = value;
            }
        }
    }

    /// Estimates the Jaccard similarity between this sketch and `other`.
    ///
    /// The result is the fraction of signature slots whose minimum hash agrees,
    /// which is an unbiased estimator of `|A ∩ B| / |A ∪ B|`. Identical sets
    /// estimate `1.0`; disjoint sets estimate near `0.0`. Two empty sketches
    /// agree on every slot and so report `1.0`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IncompatibleParameters`] if the two sketches have
    /// different signature lengths.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::MinHash;
    ///
    /// let mut a = MinHash::new(256).unwrap();
    /// let mut b = MinHash::new(256).unwrap();
    /// for i in 0..1_000u32 {
    ///     a.insert(&i);
    /// }
    /// for i in 0..1_000u32 {
    ///     b.insert(&i);
    /// }
    /// // Identical sets.
    /// assert_eq!(a.similarity(&b).unwrap(), 1.0);
    /// ```
    pub fn similarity(&self, other: &Self) -> Result<f64, Error> {
        if self.signature.len() != other.signature.len() {
            return Err(Error::IncompatibleParameters);
        }
        let matches = self
            .signature
            .iter()
            .zip(other.signature.iter())
            .filter(|(a, b)| a == b)
            .count();
        Ok(matches as f64 / self.signature.len() as f64)
    }

    /// Merges `other` into `self`, producing the sketch of the *union* of the
    /// two sets.
    ///
    /// Each signature slot becomes the minimum of the two sketches' slots, which
    /// is exactly the slot the union would have produced. Both sketches must
    /// have the same signature length.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IncompatibleParameters`] if the signature lengths differ.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::MinHash;
    ///
    /// let mut a = MinHash::new(128).unwrap();
    /// let mut b = MinHash::new(128).unwrap();
    /// a.insert("x");
    /// b.insert("y");
    ///
    /// a.merge(&b).unwrap();
    /// // `a` now summarises {x, y}.
    /// ```
    pub fn merge(&mut self, other: &Self) -> Result<(), Error> {
        if self.signature.len() != other.signature.len() {
            return Err(Error::IncompatibleParameters);
        }
        for (dst, src) in self.signature.iter_mut().zip(other.signature.iter()) {
            *dst = (*dst).min(*src);
        }
        Ok(())
    }

    /// The number of hash functions in the signature.
    #[inline]
    #[must_use]
    pub fn num_hashes(&self) -> usize {
        self.signature.len()
    }

    /// Returns `true` if no items have been inserted.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.signature.iter().all(|&slot| slot == u64::MAX)
    }

    /// Resets the sketch to empty, retaining the allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::MinHash;
    ///
    /// let mut sketch = MinHash::new(64).unwrap();
    /// sketch.insert("x");
    /// sketch.clear();
    /// assert!(sketch.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.signature.iter_mut().for_each(|slot| *slot = u64::MAX);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_new_rejects_zero() {
        assert!(matches!(
            MinHash::<&str>::new(0),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_identical_sets_are_fully_similar() {
        let mut a = MinHash::new(256).unwrap();
        let mut b = MinHash::new(256).unwrap();
        for i in 0..1_000u32 {
            a.insert(&i);
            b.insert(&i);
        }
        assert_eq!(a.similarity(&b).unwrap(), 1.0);
    }

    #[test]
    fn test_disjoint_sets_are_dissimilar() {
        let mut a = MinHash::new(256).unwrap();
        let mut b = MinHash::new(256).unwrap();
        for i in 0..1_000u32 {
            a.insert(&i);
        }
        for i in 10_000..11_000u32 {
            b.insert(&i);
        }
        let similarity = a.similarity(&b).unwrap();
        assert!(similarity < 0.1, "disjoint sets too similar: {similarity}");
    }

    #[test]
    fn test_partial_overlap_estimate() {
        // A = [0, 1000), B = [500, 1500). |A∩B| = 500, |A∪B| = 1500 -> J = 1/3.
        let mut a = MinHash::new(512).unwrap();
        let mut b = MinHash::new(512).unwrap();
        for i in 0..1_000u32 {
            a.insert(&i);
        }
        for i in 500..1_500u32 {
            b.insert(&i);
        }
        let similarity = a.similarity(&b).unwrap();
        assert!(
            (0.27..=0.40).contains(&similarity),
            "estimate {similarity} far from 1/3"
        );
    }

    #[test]
    fn test_similarity_rejects_mismatched_lengths() {
        let a = MinHash::<u32>::new(128).unwrap();
        let b = MinHash::<u32>::new(256).unwrap();
        assert_eq!(a.similarity(&b), Err(Error::IncompatibleParameters));
    }

    #[test]
    fn test_merge_forms_union() {
        let mut a = MinHash::new(256).unwrap();
        let mut b = MinHash::new(256).unwrap();
        let mut union = MinHash::new(256).unwrap();

        for i in 0..500u32 {
            a.insert(&i);
            union.insert(&i);
        }
        for i in 500..1_000u32 {
            b.insert(&i);
            union.insert(&i);
        }

        a.merge(&b).unwrap();
        // The merged sketch should match the directly-built union exactly.
        assert_eq!(a.similarity(&union).unwrap(), 1.0);
    }

    #[test]
    fn test_merge_rejects_mismatched_lengths() {
        let mut a = MinHash::<u32>::new(128).unwrap();
        let b = MinHash::<u32>::new(64).unwrap();
        assert_eq!(a.merge(&b), Err(Error::IncompatibleParameters));
    }

    #[test]
    fn test_clear() {
        let mut sketch = MinHash::new(64).unwrap();
        sketch.insert("x");
        assert!(!sketch.is_empty());
        sketch.clear();
        assert!(sketch.is_empty());
    }
}
