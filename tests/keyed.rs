//! The keyed hasher against crafted collisions, and persisting keyed state.

#![cfg(feature = "std")]
#![allow(unused_results, clippy::unwrap_used)]

use bloom_lib::hash::{DefaultHashBuilder, KeyedHashBuilder};
use bloom_lib::{BloomFilter, CountMinSketch, CuckooFilter, HyperLogLog};
use core::hash::BuildHasher;

fn fold_mul(a: u64, b: u64) -> u64 {
    let p = (a as u128).wrapping_mul(b as u128);
    (p as u64) ^ ((p >> 64) as u64)
}

/// Builds 1,000 distinct 16-byte keys that collide with `target` under the
/// default hasher, with no search: the second block cancels the difference
/// in the first. This uses only the public constants of the default hasher.
fn colliders(target: &[u8; 16]) -> Vec<[u8; 16]> {
    const SEED: u64 = 0x2545_f491_4f6c_dd1d;
    const MIX_A: u64 = 0xff51_afd7_ed55_8ccd;
    // State after the 64-bit length prefix (16) of a [u8] key.
    let s0 = fold_mul(SEED ^ 16, MIX_A);
    let d1 = u64::from_le_bytes(target[..8].try_into().unwrap());
    let d2 = u64::from_le_bytes(target[8..].try_into().unwrap());
    let s1 = fold_mul(s0 ^ d1, MIX_A);
    (0..1000u64)
        .map(|n| {
            let d1p = 0x4141_4141_0000_0000 | n;
            let s1p = fold_mul(s0 ^ d1p, MIX_A);
            let d2p = d2 ^ s1 ^ s1p;
            let mut k = [0u8; 16];
            k[..8].copy_from_slice(&d1p.to_le_bytes());
            k[8..].copy_from_slice(&d2p.to_le_bytes());
            k
        })
        .collect()
}

#[test]
fn test_default_hasher_collisions_are_constructible() {
    // Documents why the keyed hasher exists. Holds on every target now that
    // the length prefix is always 64-bit.
    let target = *b"victim-key-00001";
    let want = DefaultHashBuilder.hash_one(&target[..]);
    assert!(colliders(&target)
        .iter()
        .all(|k| DefaultHashBuilder.hash_one(&k[..]) == want));
}

#[test]
fn test_keyed_hasher_defeats_crafted_collisions() {
    let target = *b"victim-key-00001";
    let keys = colliders(&target);
    let hasher = KeyedHashBuilder::new(*b"0123456789abcdef");

    let want = hasher.hash_one(&target[..]);
    assert!(keys.iter().all(|k| hasher.hash_one(&k[..]) != want));

    // Cuckoo: with the default hasher a filter sized for 1M refused inserts
    // after 9 of these keys. With a key it takes all of them.
    let mut cuckoo: CuckooFilter<[u8], _> = CuckooFilter::with_hasher(1_000_000, hasher).unwrap();
    for k in &keys {
        cuckoo.insert(&k[..]).unwrap();
    }
    assert_eq!(cuckoo.len(), keys.len());

    // HyperLogLog: 1,000 distinct keys no longer count as one.
    let mut hll: HyperLogLog<[u8], _> = HyperLogLog::with_hasher(14, hasher).unwrap();
    for k in &keys {
        hll.insert(&k[..]);
    }
    assert!(hll.count() > 900, "count {}", hll.count());

    // Count-Min: the colliders no longer inflate the target.
    let mut cms: CountMinSketch<[u8], _> =
        CountMinSketch::with_hasher(0.001, 0.0001, hasher).unwrap();
    for k in &keys {
        cms.increment(&k[..]);
    }
    assert!(cms.estimate(&target[..]) < 10);

    // Bloom: the colliders are not reported present.
    let mut bloom: BloomFilter<[u8], _> = BloomFilter::with_hasher(1000, 0.0001, hasher).unwrap();
    bloom.insert(&target[..]);
    assert!(keys.iter().filter(|k| bloom.contains(&k[..])).count() < 5);
}

#[cfg(feature = "serde")]
#[test]
fn test_keyed_state_persists_with_replace_hasher() {
    let key = KeyedHashBuilder::random().key();
    let mut bloom: BloomFilter<str, _> =
        BloomFilter::with_hasher(1_000, 0.01, KeyedHashBuilder::new(key)).unwrap();
    let mut cuckoo: CuckooFilter<str, _> =
        CuckooFilter::with_hasher(1_000, KeyedHashBuilder::new(key)).unwrap();
    for i in 0..500 {
        let item = format!("item-{i}");
        bloom.insert(&item);
        cuckoo.insert(&item).unwrap();
    }

    let bloom_json = serde_json::to_string(&bloom).unwrap();
    let cuckoo_json = serde_json::to_string(&cuckoo).unwrap();
    let key_json = serde_json::to_string(&KeyedHashBuilder::new(key)).unwrap();

    let hasher: KeyedHashBuilder = serde_json::from_str(&key_json).unwrap();
    let bloom2 = serde_json::from_str::<BloomFilter<str>>(&bloom_json)
        .unwrap()
        .replace_hasher(hasher);
    let cuckoo2 = serde_json::from_str::<CuckooFilter<str>>(&cuckoo_json)
        .unwrap()
        .replace_hasher(hasher);
    for i in 0..500 {
        let item = format!("item-{i}");
        assert!(bloom2.contains(&item));
        assert!(cuckoo2.contains(&item));
    }
}

#[cfg(feature = "serde")]
#[test]
fn test_random_state_does_not_survive_a_round_trip() {
    // Documented limitation: `RandomState::default()` draws a new key, so a
    // restored filter hashes differently. Kept so the docs stay honest.
    use std::collections::hash_map::RandomState;
    let mut bloom: BloomFilter<u32, RandomState> =
        BloomFilter::with_hasher(1_000, 0.01, RandomState::new()).unwrap();
    for i in 0..1_000 {
        bloom.insert(&i);
    }
    let json = serde_json::to_string(&bloom).unwrap();
    let restored: BloomFilter<u32, RandomState> = serde_json::from_str(&json).unwrap();
    let kept = (0..1_000).filter(|i| restored.contains(i)).count();
    assert!(kept < 1_000);
}
