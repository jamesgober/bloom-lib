//! Property-based tests for the probabilistic guarantees of each structure.
//!
//! These exercise the invariants that must hold for *every* input, not just the
//! hand-picked cases in the unit tests: Bloom and Cuckoo filters never report a
//! false negative, the Count-Min Sketch never undercounts, HyperLogLog estimates
//! distinct counts within a wide envelope, and MinHash similarity is bounded and
//! symmetric.

#![cfg(feature = "alloc")]

use bloom_lib::{BloomFilter, CountMinSketch, CuckooFilter, HyperLogLog, MinHash};
use proptest::collection::vec;
use proptest::prelude::*;

proptest! {
    /// A Bloom filter never reports an inserted item as absent.
    #[test]
    fn bloom_has_no_false_negatives(items in vec(any::<u64>(), 0..500)) {
        let mut filter = BloomFilter::new(1_000, 0.01).unwrap();
        for item in &items {
            let _ = filter.insert(item);
        }
        for item in &items {
            prop_assert!(filter.contains(item));
        }
    }

    /// Merging two Bloom filters yields a superset of both.
    #[test]
    fn bloom_merge_is_union(
        left in vec(any::<u64>(), 0..200),
        right in vec(any::<u64>(), 0..200),
    ) {
        let mut a = BloomFilter::new(1_000, 0.01).unwrap();
        let mut b = BloomFilter::new(1_000, 0.01).unwrap();
        for item in &left {
            let _ = a.insert(item);
        }
        for item in &right {
            let _ = b.insert(item);
        }
        a.merge(&b).unwrap();
        for item in left.iter().chain(right.iter()) {
            prop_assert!(a.contains(item));
        }
    }

    /// A Cuckoo filter never reports an inserted item as absent (within capacity).
    #[test]
    fn cuckoo_has_no_false_negatives(items in vec(any::<u64>(), 0..400)) {
        let mut filter = CuckooFilter::new(2_000).unwrap();
        let mut accepted = Vec::new();
        for item in &items {
            if filter.insert(item).is_ok() {
                accepted.push(*item);
            }
        }
        for item in &accepted {
            prop_assert!(filter.contains(item));
        }
    }

    /// After removing an item, one of its insertions is gone; re-checking a
    /// freshly inserted-then-removed unique item reports absent.
    #[test]
    fn cuckoo_remove_round_trip(item in any::<u64>()) {
        let mut filter = CuckooFilter::new(1_000).unwrap();
        filter.insert(&item).unwrap();
        prop_assert!(filter.contains(&item));
        prop_assert!(filter.remove(&item));
        // A single insert/remove of a unique value leaves it absent.
        prop_assert!(!filter.contains(&item));
    }

    /// The Count-Min Sketch never undercounts an item's true frequency.
    #[test]
    fn count_min_never_undercounts(items in vec(0u32..50, 0..1_000)) {
        let mut sketch = CountMinSketch::new(0.001, 0.001).unwrap();
        let mut truth = [0u64; 50];
        for &item in &items {
            sketch.increment(&item);
            truth[item as usize] += 1;
        }
        for value in 0u32..50 {
            prop_assert!(sketch.estimate(&value) >= truth[value as usize]);
        }
    }

    /// HyperLogLog estimates distinct counts within a generous relative envelope.
    #[test]
    fn hyperloglog_estimate_within_envelope(n in 0u32..20_000) {
        let mut hll = HyperLogLog::new(14).unwrap();
        for i in 0..n {
            hll.insert(&i);
        }
        let estimate = hll.count();
        if n == 0 {
            prop_assert_eq!(estimate, 0);
        } else {
            let error = (estimate as f64 - f64::from(n)).abs() / f64::from(n);
            // Precision 14 has ~0.8% standard error; 5% is a very safe envelope.
            prop_assert!(error < 0.05, "n={} estimate={} error={}", n, estimate, error);
        }
    }

    /// MinHash similarity is always in [0, 1] and an identical set scores 1.0.
    #[test]
    fn minhash_similarity_bounds(items in vec(any::<u64>(), 0..300)) {
        let mut a = MinHash::new(128).unwrap();
        let mut b = MinHash::new(128).unwrap();
        for item in &items {
            a.insert(item);
            b.insert(item);
        }
        let similarity = a.similarity(&b).unwrap();
        prop_assert!((0.0..=1.0).contains(&similarity));
        // Two sketches of the same set agree on every slot.
        prop_assert_eq!(similarity, 1.0);
    }

    /// MinHash similarity is symmetric.
    #[test]
    fn minhash_similarity_is_symmetric(
        left in vec(any::<u64>(), 0..200),
        right in vec(any::<u64>(), 0..200),
    ) {
        let mut a = MinHash::new(128).unwrap();
        let mut b = MinHash::new(128).unwrap();
        for item in &left {
            a.insert(item);
        }
        for item in &right {
            b.insert(item);
        }
        prop_assert_eq!(a.similarity(&b).unwrap(), b.similarity(&a).unwrap());
    }
}
