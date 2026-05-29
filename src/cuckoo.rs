//! A Cuckoo filter: approximate set membership with deletion support.

use core::{hash::BuildHasher, marker::PhantomData};

use alloc::{vec, vec::Vec};

use crate::{
    hash::{mix64, DefaultHashBuilder},
    Error,
};

/// Fingerprints per bucket. Four is the value analysed by the original paper
/// for a good balance of load factor and lookup cost.
const BUCKET_SIZE: usize = 4;

/// Maximum number of relocations attempted before declaring an insertion
/// failure. The paper shows 500 keeps failure probability negligible below the
/// load threshold.
const MAX_KICKS: usize = 500;

/// Sentinel fingerprint value meaning "empty slot". Real fingerprints are
/// remapped away from zero, so this never collides with stored data.
const EMPTY: u16 = 0;

/// The held-out element produced when a relocation chain exhausts its budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Victim {
    fingerprint: u16,
    index: usize,
}

/// A probabilistic set that supports deletion.
///
/// Like a [`BloomFilter`](crate::BloomFilter), a Cuckoo filter answers
/// approximate membership queries in a fraction of the memory an exact set would
/// need, and never reports a false negative for an item that is present.
/// Unlike a Bloom filter it supports [`remove`](Self::remove): items can be
/// deleted without rebuilding the structure.
///
/// The filter stores a short *fingerprint* of each item (16 bits here) in one of
/// two candidate buckets, relocating existing fingerprints as needed to make
/// room — the "cuckoo" behaviour. The false-positive rate is fixed by the
/// fingerprint width at roughly 0.012%; for a tunable rate without deletion, use
/// a [`BloomFilter`](crate::BloomFilter).
///
/// The filter is generic over the item type `T` and a
/// [`BuildHasher`](core::hash::BuildHasher) `S`, defaulting to the deterministic
/// [`DefaultHashBuilder`](crate::hash::DefaultHashBuilder).
///
/// # Examples
///
/// ```
/// use bloom_lib::CuckooFilter;
///
/// let mut filter = CuckooFilter::new(1_000).unwrap();
///
/// filter.insert("alice").unwrap();
/// assert!(filter.contains("alice"));
///
/// assert!(filter.remove("alice"));
/// assert!(!filter.contains("alice"));
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CuckooFilter<T: ?Sized, S = DefaultHashBuilder> {
    buckets: Vec<u16>,
    mask: usize,
    len: usize,
    victim: Option<Victim>,
    rng_state: u64,
    #[cfg_attr(feature = "serde", serde(skip))]
    hasher: S,
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: PhantomData<fn(&T)>,
}

impl<T: ?Sized> CuckooFilter<T, DefaultHashBuilder> {
    /// Creates a filter able to hold roughly `capacity` items, using the default
    /// hasher.
    ///
    /// The number of buckets is rounded up to a power of two with headroom, so
    /// the reported [`capacity`](Self::capacity) is at least `capacity` and
    /// usually larger. Inserting close to the reported capacity may begin to
    /// fail with [`Error::CapacityExceeded`]; size with margin for write-heavy
    /// workloads.
    ///
    /// # Parameters
    ///
    /// - `capacity`: the expected number of distinct items. Must be non-zero.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `capacity` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CuckooFilter;
    ///
    /// let filter = CuckooFilter::<&str>::new(10_000).unwrap();
    /// assert!(filter.capacity() >= 10_000);
    /// assert!(filter.is_empty());
    /// ```
    pub fn new(capacity: usize) -> Result<Self, Error> {
        Self::with_hasher(capacity, DefaultHashBuilder)
    }
}

impl<T: ?Sized, S: BuildHasher> CuckooFilter<T, S> {
    /// Creates a filter for roughly `capacity` items with a caller-supplied
    /// hasher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidParameter`] if `capacity` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] {
    /// use std::collections::hash_map::RandomState;
    /// use bloom_lib::CuckooFilter;
    ///
    /// let filter: CuckooFilter<&str, RandomState> =
    ///     CuckooFilter::with_hasher(1_000, RandomState::new()).unwrap();
    /// # }
    /// ```
    pub fn with_hasher(capacity: usize, hasher: S) -> Result<Self, Error> {
        if capacity == 0 {
            return Err(Error::InvalidParameter {
                param: "capacity",
                reason: "must be greater than zero",
            });
        }

        let num_buckets = capacity_to_buckets(capacity);
        Ok(Self {
            buckets: vec![EMPTY; num_buckets * BUCKET_SIZE],
            mask: num_buckets - 1,
            len: 0,
            victim: None,
            // Non-zero xorshift seed derived from the table size.
            rng_state: 0x9E37_79B9_7F4A_7C15 ^ num_buckets as u64,
            hasher,
            _marker: PhantomData,
        })
    }

    /// Inserts `item` into the filter.
    ///
    /// Duplicates are permitted: inserting the same item twice stores two
    /// fingerprints and requires two [`remove`](Self::remove) calls to fully
    /// evict. This matches the semantics of the original Cuckoo filter and keeps
    /// `remove` exact with respect to insertions.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CapacityExceeded`] when the filter is too full to place
    /// the item within its relocation budget. The filter is unchanged in that
    /// case and remains usable for lookups and deletions.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CuckooFilter;
    ///
    /// let mut filter = CuckooFilter::new(100).unwrap();
    /// filter.insert(&42u32).unwrap();
    /// assert!(filter.contains(&42u32));
    /// ```
    pub fn insert(&mut self, item: &T) -> Result<(), Error>
    where
        T: core::hash::Hash,
    {
        // A held-out victim means the table is already at its practical limit.
        if self.victim.is_some() {
            return Err(Error::CapacityExceeded);
        }

        let hash = self.hasher.hash_one(item);
        let fingerprint = fingerprint_of(hash);
        let i1 = (hash as usize) & self.mask;
        let i2 = self.alt_index(i1, fingerprint);

        if self.try_put(i1, fingerprint) || self.try_put(i2, fingerprint) {
            self.len += 1;
            return Ok(());
        }

        // Both candidate buckets are full: relocate a resident fingerprint.
        let mut index = if self.next_rng() & 1 == 0 { i1 } else { i2 };
        let mut moving = fingerprint;
        for _ in 0..MAX_KICKS {
            let slot = (self.next_rng() as usize) & (BUCKET_SIZE - 1);
            let pos = index * BUCKET_SIZE + slot;
            core::mem::swap(&mut moving, &mut self.buckets[pos]);
            index = self.alt_index(index, moving);
            if self.try_put(index, moving) {
                self.len += 1;
                return Ok(());
            }
        }

        // Budget exhausted. `moving` is now homeless; stash it in the victim
        // slot so no previously-inserted element is lost, and count the new
        // element we successfully placed during the first swap.
        self.victim = Some(Victim {
            fingerprint: moving,
            index,
        });
        self.len += 1;
        Ok(())
    }

    /// Tests whether `item` is in the filter.
    ///
    /// Returns `false` only if the item was definitely never inserted (or has
    /// since been removed). A return of `true` means the item is *probably*
    /// present, with a false-positive probability of roughly 0.012%.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CuckooFilter;
    ///
    /// let mut filter = CuckooFilter::new(100).unwrap();
    /// filter.insert("present").unwrap();
    /// assert!(filter.contains("present"));
    /// assert!(!filter.contains("absent"));
    /// ```
    #[must_use]
    pub fn contains(&self, item: &T) -> bool
    where
        T: core::hash::Hash,
    {
        let hash = self.hasher.hash_one(item);
        let fingerprint = fingerprint_of(hash);
        let i1 = (hash as usize) & self.mask;
        let i2 = self.alt_index(i1, fingerprint);
        self.bucket_contains(i1, fingerprint)
            || self.bucket_contains(i2, fingerprint)
            || self.victim_matches(fingerprint, i1, i2)
    }

    /// Removes one occurrence of `item`, returning `true` if it was present.
    ///
    /// Only a single stored fingerprint is removed per call, mirroring `insert`.
    /// Because fingerprints collide, removing an item that was never inserted
    /// but collides with a present one can delete the wrong fingerprint; this is
    /// the same false-positive trade-off that governs `contains`. Only delete
    /// items you previously inserted.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CuckooFilter;
    ///
    /// let mut filter = CuckooFilter::new(100).unwrap();
    /// filter.insert("temp").unwrap();
    /// assert!(filter.remove("temp"));
    /// assert!(!filter.remove("temp")); // already gone
    /// ```
    pub fn remove(&mut self, item: &T) -> bool
    where
        T: core::hash::Hash,
    {
        let hash = self.hasher.hash_one(item);
        let fingerprint = fingerprint_of(hash);
        let i1 = (hash as usize) & self.mask;
        let i2 = self.alt_index(i1, fingerprint);

        if self.try_take(i1, fingerprint) || self.try_take(i2, fingerprint) {
            self.len -= 1;
            self.reinsert_victim();
            return true;
        }

        if self.victim_matches(fingerprint, i1, i2) {
            self.victim = None;
            self.len -= 1;
            return true;
        }

        false
    }

    /// Removes every item, retaining the allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CuckooFilter;
    ///
    /// let mut filter = CuckooFilter::new(100).unwrap();
    /// filter.insert("x").unwrap();
    /// filter.clear();
    /// assert!(filter.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.buckets.iter_mut().for_each(|slot| *slot = EMPTY);
        self.len = 0;
        self.victim = None;
    }

    /// The number of fingerprints currently stored.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CuckooFilter;
    ///
    /// let mut filter = CuckooFilter::new(100).unwrap();
    /// filter.insert("a").unwrap();
    /// filter.insert("b").unwrap();
    /// assert_eq!(filter.len(), 2);
    /// ```
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the filter holds no items.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The maximum number of fingerprints the table can hold.
    ///
    /// This is `num_buckets * 4`. Practical fill tops out a little below this
    /// value before insertions start to fail.
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.buckets.len()
    }

    /// The current load factor in `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom_lib::CuckooFilter;
    ///
    /// let filter = CuckooFilter::<u32>::new(100).unwrap();
    /// assert_eq!(filter.load_factor(), 0.0);
    /// ```
    #[must_use]
    pub fn load_factor(&self) -> f64 {
        self.len as f64 / self.buckets.len() as f64
    }

    /// Computes the alternate bucket index for a fingerprint via the XOR trick.
    ///
    /// Because XOR is its own inverse, `alt_index(alt_index(i, fp), fp) == i`,
    /// which lets `contains` and relocation recover either candidate from the
    /// other without storing it.
    #[inline]
    fn alt_index(&self, index: usize, fingerprint: u16) -> usize {
        index ^ ((mix64(u64::from(fingerprint)) as usize) & self.mask)
    }

    /// Attempts to place `fingerprint` in a free slot of bucket `index`.
    #[inline]
    fn try_put(&mut self, index: usize, fingerprint: u16) -> bool {
        let base = index * BUCKET_SIZE;
        for slot in &mut self.buckets[base..base + BUCKET_SIZE] {
            if *slot == EMPTY {
                *slot = fingerprint;
                return true;
            }
        }
        false
    }

    /// Attempts to remove one `fingerprint` from bucket `index`.
    #[inline]
    fn try_take(&mut self, index: usize, fingerprint: u16) -> bool {
        let base = index * BUCKET_SIZE;
        for slot in &mut self.buckets[base..base + BUCKET_SIZE] {
            if *slot == fingerprint {
                *slot = EMPTY;
                return true;
            }
        }
        false
    }

    /// Returns `true` if bucket `index` holds `fingerprint`.
    #[inline]
    fn bucket_contains(&self, index: usize, fingerprint: u16) -> bool {
        let base = index * BUCKET_SIZE;
        self.buckets[base..base + BUCKET_SIZE].contains(&fingerprint)
    }

    /// Returns `true` if the held-out victim matches the query.
    #[inline]
    fn victim_matches(&self, fingerprint: u16, i1: usize, i2: usize) -> bool {
        matches!(
            self.victim,
            Some(Victim { fingerprint: vf, index: vi })
                if vf == fingerprint && (vi == i1 || vi == i2)
        )
    }

    /// After a deletion frees a slot, try to settle the victim back into the
    /// table so it stops blocking future insertions.
    fn reinsert_victim(&mut self) {
        let Some(victim) = self.victim else {
            return;
        };
        let alt = self.alt_index(victim.index, victim.fingerprint);
        if self.try_put(victim.index, victim.fingerprint) || self.try_put(alt, victim.fingerprint) {
            self.victim = None;
        }
    }

    /// Advances the internal xorshift PRNG used to pick relocation targets.
    #[inline]
    fn next_rng(&mut self) -> u64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        x
    }
}

/// Derives a non-zero 16-bit fingerprint from the high bits of a 64-bit hash.
#[inline]
fn fingerprint_of(hash: u64) -> u16 {
    let fingerprint = (hash >> 48) as u16;
    if fingerprint == EMPTY {
        1
    } else {
        fingerprint
    }
}

/// Rounds a capacity up to a power-of-two bucket count with ~5% headroom.
fn capacity_to_buckets(capacity: usize) -> usize {
    let target_load = 0.95;
    let raw = libm::ceil(capacity as f64 / (BUCKET_SIZE as f64 * target_load)) as usize;
    raw.max(1).next_power_of_two()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_new_rejects_zero_capacity() {
        assert!(matches!(
            CuckooFilter::<&str>::new(0),
            Err(Error::InvalidParameter { .. })
        ));
    }

    #[test]
    fn test_insert_contains_remove() {
        let mut filter = CuckooFilter::new(1_000).unwrap();
        filter.insert("alice").unwrap();
        assert!(filter.contains("alice"));
        assert!(filter.remove("alice"));
        assert!(!filter.contains("alice"));
        assert!(!filter.remove("alice"));
    }

    #[test]
    fn test_no_false_negatives() {
        let mut filter = CuckooFilter::new(5_000).unwrap();
        for i in 0..2_000u32 {
            filter.insert(&i).unwrap();
        }
        for i in 0..2_000u32 {
            assert!(filter.contains(&i), "inserted item {i} reported absent");
        }
    }

    #[test]
    fn test_duplicates_need_matching_removes() {
        let mut filter = CuckooFilter::new(100).unwrap();
        filter.insert("dup").unwrap();
        filter.insert("dup").unwrap();
        assert_eq!(filter.len(), 2);
        assert!(filter.remove("dup"));
        assert!(filter.contains("dup")); // one copy remains
        assert!(filter.remove("dup"));
        assert!(!filter.contains("dup"));
    }

    #[test]
    fn test_len_and_clear() {
        let mut filter = CuckooFilter::new(100).unwrap();
        assert!(filter.is_empty());
        for i in 0..10u32 {
            filter.insert(&i).unwrap();
        }
        assert_eq!(filter.len(), 10);
        filter.clear();
        assert!(filter.is_empty());
        assert_eq!(filter.len(), 0);
    }

    #[test]
    fn test_capacity_has_headroom() {
        let filter = CuckooFilter::<u32>::new(1_000).unwrap();
        assert!(filter.capacity() >= 1_000);
        // Power-of-two bucket count means capacity is a multiple of 4.
        assert_eq!(filter.capacity() % BUCKET_SIZE, 0);
    }

    #[test]
    fn test_fill_to_capacity_eventually_errors() {
        // A deliberately small filter; insert distinct items until it refuses.
        let mut filter = CuckooFilter::<u64>::new(64).unwrap();
        let mut inserted = 0u64;
        let mut hit_limit = false;
        for i in 0..10_000u64 {
            match filter.insert(&i) {
                Ok(()) => inserted += 1,
                Err(Error::CapacityExceeded) => {
                    hit_limit = true;
                    break;
                }
                Err(other) => panic!("unexpected error: {other:?}"),
            }
        }
        assert!(hit_limit, "filter never reported CapacityExceeded");
        // Everything that was accepted must still be found (no false negatives).
        for i in 0..inserted {
            assert!(filter.contains(&i), "accepted item {i} reported absent");
        }
    }

    #[test]
    fn test_load_factor() {
        let mut filter = CuckooFilter::<u32>::new(100).unwrap();
        let cap = filter.capacity();
        for i in 0..(cap / 2) as u32 {
            filter.insert(&i).unwrap();
        }
        let lf = filter.load_factor();
        assert!((0.49..=0.51).contains(&lf), "unexpected load factor {lf}");
    }
}
