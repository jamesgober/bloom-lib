//! Constructors return errors for parameters too large to allocate.
//!
//! In 1.0.0 these aborted the process or panicked even though every
//! constructor returns `Result`.

#![cfg(feature = "std")]
#![allow(clippy::unwrap_used)]

use bloom_lib::{BloomFilter, CountMinSketch, CuckooFilter, Error, MinHash, TopK};

fn is_invalid<T>(result: Result<T, Error>, param: &str) {
    match result {
        Err(Error::InvalidParameter { param: got, .. }) => assert_eq!(got, param),
        Err(other) => panic!("unexpected error {other:?}"),
        Ok(_) => panic!("oversized parameter accepted"),
    }
}

#[test]
fn test_bloom_huge_dimensions_return_error() {
    is_invalid(BloomFilter::<u32>::with_dimensions(u64::MAX, 1), "num_bits");
}

#[test]
fn test_bloom_huge_capacity_returns_error() {
    is_invalid(BloomFilter::<u32>::new(usize::MAX, 1e-300), "num_bits");
}

#[test]
fn test_bloom_hash_count_is_capped() {
    is_invalid(BloomFilter::<u32>::with_dimensions(64, 2049), "num_hashes");
    let filter = BloomFilter::<u32>::with_dimensions(64, 2048).unwrap();
    assert_eq!(filter.num_hashes(), 2048);
    // The smallest positive rate stays within the cap.
    let tiny = BloomFilter::<u32>::new(10, f64::MIN_POSITIVE).unwrap();
    assert!(tiny.num_hashes() <= 2048);
}

#[test]
fn test_count_min_width_times_depth_overflow_returns_error() {
    is_invalid(
        CountMinSketch::<u32>::with_dimensions(usize::MAX / 2 + 2, 2),
        "width",
    );
    is_invalid(CountMinSketch::<u32>::new(1e-300, 0.5), "width");
}

#[test]
fn test_count_min_huge_grid_returns_error() {
    is_invalid(
        CountMinSketch::<u32>::with_dimensions(usize::MAX / 8, 1),
        "width",
    );
}

#[test]
fn test_cuckoo_huge_capacity_returns_error() {
    is_invalid(CuckooFilter::<u32>::new(usize::MAX / 4), "capacity");
    is_invalid(CuckooFilter::<u32>::new(usize::MAX), "capacity");
}

#[test]
fn test_minhash_huge_signature_returns_error() {
    is_invalid(MinHash::<u32>::new(usize::MAX / 4), "num_hashes");
}

#[test]
fn test_topk_huge_k_returns_error() {
    is_invalid(TopK::<u64>::new(usize::MAX / 4, 0.1, 0.1), "k");
}
