//! Hostile serialized state.
//!
//! Each input below is accepted by serde's field-by-field decoding but breaks
//! an invariant of the structure. In 1.0.0 these decoded successfully and
//! then panicked, looped for as long as the input asked, or answered every
//! query wrongly. Deserializing must now fail with an error.

#![cfg(all(feature = "std", feature = "serde"))]
#![allow(clippy::unwrap_used)]

use bloom_lib::{BloomFilter, CountMinSketch, CuckooFilter, HyperLogLog, MinHash, TopK};

fn rejects<T: serde::de::DeserializeOwned>(json: &str, expect: &str) {
    match serde_json::from_str::<T>(json) {
        Ok(_) => panic!("accepted hostile state: {json}"),
        Err(err) => {
            let text = err.to_string();
            assert!(
                text.contains(expect),
                "error {text:?} does not mention {expect:?}"
            );
        }
    }
}

#[test]
fn test_bloom_rejects_word_count_mismatch() {
    rejects::<BloomFilter<u32>>(
        r#"{"bits":{"words":[],"num_bits":1000},"num_hashes":3}"#,
        "words",
    );
    rejects::<BloomFilter<u32>>(
        r#"{"bits":{"words":[0,0,0],"num_bits":64},"num_hashes":3}"#,
        "words",
    );
}

#[test]
fn test_bloom_rejects_zero_bits() {
    rejects::<BloomFilter<u32>>(
        r#"{"bits":{"words":[],"num_bits":0},"num_hashes":3}"#,
        "num_bits",
    );
}

#[test]
fn test_bloom_rejects_bits_beyond_num_bits() {
    rejects::<BloomFilter<u32>>(
        r#"{"bits":{"words":[4294967296],"num_bits":32},"num_hashes":3}"#,
        "beyond",
    );
}

#[test]
fn test_bloom_rejects_zero_hashes() {
    // 1.0.0 accepted this and then reported every item as present.
    rejects::<BloomFilter<u32>>(
        r#"{"bits":{"words":[0],"num_bits":64},"num_hashes":0}"#,
        "num_hashes",
    );
}

#[test]
fn test_bloom_rejects_huge_hash_count() {
    // 1.0.0 accepted this and then spent seconds per query.
    rejects::<BloomFilter<u32>>(
        r#"{"bits":{"words":[18446744073709551615],"num_bits":64},"num_hashes":4294967295}"#,
        "num_hashes",
    );
}

#[test]
fn test_count_min_rejects_zero_width_or_depth() {
    // 1.0.0 accepted the first and then looped 2^64 times per estimate. On
    // 32-bit targets the depth already fails to decode as `usize`.
    rejects::<CountMinSketch<u32>>(
        r#"{"counters":[0],"width":0,"depth":18446744073709551615,"total":0}"#,
        "",
    );
    rejects::<CountMinSketch<u32>>(r#"{"counters":[0],"width":0,"depth":5,"total":0}"#, "width");
    rejects::<CountMinSketch<u32>>(r#"{"counters":[0],"width":1,"depth":0,"total":0}"#, "depth");
}

#[test]
fn test_count_min_rejects_counter_count_mismatch() {
    rejects::<CountMinSketch<u32>>(
        r#"{"counters":[],"width":10,"depth":3,"total":0}"#,
        "counters",
    );
    rejects::<CountMinSketch<u32>>(
        r#"{"counters":[0],"width":1,"depth":1000000,"total":0}"#,
        "counters",
    );
    rejects::<CountMinSketch<u32>>(
        r#"{"counters":[0],"width":4294967296,"depth":4294967296,"total":0}"#,
        "",
    );
}

#[test]
fn test_hll_rejects_precision_out_of_range() {
    rejects::<HyperLogLog<u32>>(r#"{"registers":[0],"precision":0}"#, "precision");
    rejects::<HyperLogLog<u32>>(r#"{"registers":[0],"precision":19}"#, "precision");
    rejects::<HyperLogLog<u32>>(r#"{"registers":[0],"precision":255}"#, "precision");
}

#[test]
fn test_hll_rejects_register_count_mismatch() {
    rejects::<HyperLogLog<u32>>(r#"{"registers":[0,0],"precision":14}"#, "registers");
}

#[test]
fn test_hll_rejects_impossible_rank() {
    let mut registers = vec![0u8; 16];
    registers[3] = 62; // the largest rank at precision 4 is 61
    let json = format!(r#"{{"registers":{registers:?},"precision":4}}"#);
    rejects::<HyperLogLog<u32>>(&json, "rank");
    registers[3] = 61;
    let json = format!(r#"{{"registers":{registers:?},"precision":4}}"#);
    let _: HyperLogLog<u32> = serde_json::from_str(&json).unwrap();
}

#[test]
fn test_cuckoo_rejects_mask_larger_than_table() {
    rejects::<CuckooFilter<u32>>(
        r#"{"buckets":[0,0,0,0],"mask":1023,"len":0,"victim":null,"rng_state":1}"#,
        "buckets",
    );
}

#[test]
fn test_cuckoo_rejects_mask_not_power_of_two_minus_one() {
    rejects::<CuckooFilter<u32>>(
        r#"{"buckets":[0,0,0,0,0,0,0,0,0,0,0,0],"mask":2,"len":0,"victim":null,"rng_state":1}"#,
        "mask",
    );
    rejects::<CuckooFilter<u32>>(
        r#"{"buckets":[],"mask":18446744073709551615,"len":0,"victim":null,"rng_state":1}"#,
        "",
    );
}

#[test]
fn test_cuckoo_rejects_len_mismatch() {
    // 1.0.0 accepted this; `remove` then underflowed `len`.
    let mut filter: CuckooFilter<u32> = CuckooFilter::new(16).unwrap();
    filter.insert(&5).unwrap();
    let mut value = serde_json::to_value(&filter).unwrap();
    value["len"] = 0.into();
    rejects::<CuckooFilter<u32>>(&value.to_string(), "len");
}

#[test]
fn test_cuckoo_rejects_bad_victim_and_rng_state() {
    let empty = "[0,0,0,0,0,0,0,0]";
    rejects::<CuckooFilter<u32>>(
        &format!(
            r#"{{"buckets":{empty},"mask":1,"len":1,"victim":{{"fingerprint":7,"index":2}},"rng_state":1}}"#
        ),
        "victim",
    );
    rejects::<CuckooFilter<u32>>(
        &format!(
            r#"{{"buckets":{empty},"mask":1,"len":1,"victim":{{"fingerprint":0,"index":0}},"rng_state":1}}"#
        ),
        "victim",
    );
    rejects::<CuckooFilter<u32>>(
        &format!(r#"{{"buckets":{empty},"mask":1,"len":0,"victim":null,"rng_state":0}}"#),
        "rng_state",
    );
    let ok: CuckooFilter<u32> = serde_json::from_str(&format!(
        r#"{{"buckets":{empty},"mask":1,"len":1,"victim":{{"fingerprint":7,"index":1}},"rng_state":1}}"#
    ))
    .unwrap();
    assert_eq!(ok.len(), 1);
}

#[test]
fn test_minhash_rejects_empty_signature() {
    rejects::<MinHash<u32>>(r#"{"signature":[]}"#, "signature");
}

#[test]
fn test_topk_rejects_bad_k_and_inner_sketch() {
    let sketch = r#"{"counters":[0,0],"width":2,"depth":1,"total":0}"#;
    rejects::<TopK<String>>(
        &format!(r#"{{"sketch":{sketch},"entries":[],"k":0}}"#),
        "`k`",
    );
    rejects::<TopK<String>>(
        &format!(
            r#"{{"sketch":{sketch},"entries":[{{"key":"a","count":1}},{{"key":"b","count":1}}],"k":1}}"#
        ),
        "entries",
    );
    rejects::<TopK<String>>(
        r#"{"sketch":{"counters":[0],"width":0,"depth":5,"total":0},"entries":[],"k":1}"#,
        "width",
    );
}

#[test]
fn test_valid_state_round_trips() {
    let mut bloom: BloomFilter<str> = BloomFilter::new(1_000, 0.01).unwrap();
    let _ = bloom.insert("x");
    let mut cuckoo: CuckooFilter<str> = CuckooFilter::new(100).unwrap();
    // Fill past the relocation budget so a victim is held.
    let mut i = 0u32;
    while cuckoo.insert(&i.to_string()).is_ok() {
        i += 1;
    }
    let mut cms: CountMinSketch<str> = CountMinSketch::new(0.01, 0.01).unwrap();
    cms.add("y", 3);
    let mut hll: HyperLogLog<u32> = HyperLogLog::new(18).unwrap();
    for i in 0..10_000 {
        hll.insert(&i);
    }

    let json = serde_json::to_string(&bloom).unwrap();
    let bloom2: BloomFilter<str> = serde_json::from_str(&json).unwrap();
    assert!(bloom2.contains("x"));
    let json = serde_json::to_string(&cuckoo).unwrap();
    let cuckoo2: CuckooFilter<str> = serde_json::from_str(&json).unwrap();
    assert_eq!(cuckoo2.len(), cuckoo.len());
    let json = serde_json::to_string(&cms).unwrap();
    let cms2: CountMinSketch<str> = serde_json::from_str(&json).unwrap();
    assert!(cms2.estimate("y") >= 3);
    let json = serde_json::to_string(&hll).unwrap();
    let hll2: HyperLogLog<u32> = serde_json::from_str(&json).unwrap();
    assert_eq!(hll2.count(), hll.count());
}

#[test]
fn test_item_type_need_not_be_deserializable() {
    // The item type only appears as a marker, so it needs no serde impls.
    struct Opaque;
    let _: BloomFilter<Opaque> =
        serde_json::from_str(r#"{"bits":{"words":[0],"num_bits":64},"num_hashes":3}"#).unwrap();
}
