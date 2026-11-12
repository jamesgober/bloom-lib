//! Golden compatibility tests.
//!
//! Every constant below was produced by bloom-lib 1.0.0 on x86_64. Hash values
//! and serialized state must stay identical on every target and in every 1.x
//! release, because callers persist serialized filters (for example as
//! on-disk sidecars) and query them later, possibly on another machine. A
//! changed hash turns every stored item into a false negative.
//!
//! CI runs this file on 64-bit and 32-bit targets and on a big-endian target.

#![cfg(all(feature = "std", feature = "serde"))]
#![allow(unused_results, clippy::unwrap_used)]

use bloom_lib::hash::DefaultHashBuilder;
use bloom_lib::{BloomFilter, CountMinSketch, CuckooFilter, HyperLogLog, MinHash, TopK};
use core::hash::BuildHasher;

/// Deterministic test bytes: `(i * 31 + 7) mod 256`.
fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 + 7) as u8).collect()
}

#[test]
fn test_hash_values_match_1_0_0() {
    let b = DefaultHashBuilder;
    let cases: &[(&str, u64, u64)] = &[
        ("[u8; 0]", b.hash_one(&bytes(0)[..]), 0x715e_a512_fc24_1769),
        ("[u8; 1]", b.hash_one(&bytes(1)[..]), 0x2cee_4a65_a799_2cdd),
        ("[u8; 7]", b.hash_one(&bytes(7)[..]), 0xfaf8_a43f_a100_66bc),
        ("[u8; 8]", b.hash_one(&bytes(8)[..]), 0x6655_ad53_4b63_139d),
        ("[u8; 9]", b.hash_one(&bytes(9)[..]), 0x5ecd_1cb3_87f7_081e),
        (
            "[u8; 16]",
            b.hash_one(&bytes(16)[..]),
            0xfbd5_27d2_28ef_e841,
        ),
        (
            "[u8; 33]",
            b.hash_one(&bytes(33)[..]),
            0x1ef1_4eeb_e3d7_6320,
        ),
        ("str empty", b.hash_one(""), 0xdcf4_3ccc_7021_091c),
        ("str a", b.hash_one("a"), 0x2e45_f48c_b5a0_3465),
        ("str user:42", b.hash_one("user:42"), 0xc512_bdca_6b28_25f8),
        (
            "str long",
            b.hash_one("probabilistic data structures"),
            0x6918_5ad0_c49d_bb14,
        ),
        ("u8", b.hash_one(0xabu8), 0xa297_a8ef_5dfa_9619),
        ("u16", b.hash_one(0xbeefu16), 0xbb01_9da5_d236_c5f0),
        ("u32", b.hash_one(0xdead_beefu32), 0x40b9_cf28_71be_5b91),
        (
            "u64",
            b.hash_one(0x0123_4567_89ab_cdefu64),
            0xb9a9_58c0_0fa7_f3d1,
        ),
        (
            "u128",
            b.hash_one(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210u128),
            0x0a34_dcb9_572a_a7cf,
        ),
        ("usize", b.hash_one(42usize), 0xde0c_b7ca_5c8f_90fe),
        ("isize", b.hash_one(-42isize), 0x4f8d_51e9_d574_5028),
        ("i8", b.hash_one(-1i8), 0xdcf4_3ccc_7021_091c),
        ("i16", b.hash_one(-2i16), 0x4563_74ad_b406_750f),
        ("i32", b.hash_one(-3i32), 0xeb49_35db_4f49_5efe),
        ("i64", b.hash_one(-4i64), 0xa520_9ca4_9ab9_dba9),
        ("i128", b.hash_one(-5i128), 0xe5e6_480e_4d06_1434),
        ("char", b.hash_one('\u{e9}'), 0x9d66_00ad_31b5_609c),
        ("bool", b.hash_one(true), 0x34de_bf25_438b_5187),
        ("tuple", b.hash_one((7u32, "seven")), 0xde5d_86d7_1af2_5761),
        ("Vec<u8>", b.hash_one(bytes(5)), 0xc0d7_4756_81cc_f471),
        ("Option<u64>", b.hash_one(Some(9u64)), 0x6858_6ed7_313a_e5b7),
        (
            "String",
            b.hash_one(String::from("owned")),
            0x43fc_b481_baa0_2b23,
        ),
    ];
    for &(name, got, want) in cases {
        assert_eq!(
            got, want,
            "hash of {name} changed: got {got:#018x}, want {want:#018x}"
        );
    }
}

const BLOOM: &str = r#"{"bits":{"words":[2311369154705240464,158467919786637,2308459849035285038,226095608266624544],"num_bits":256},"num_hashes":3}"#;
const CUCKOO: &str = r#"{"buckets":[0,0,0,0,61900,50635,0,0,13497,0,0,0,20574,0,0,0,51968,0,0,0,33316,0,0,0,0,0,0,0,0,0,0,0,6242,0,0,0,19488,12048,59016,0,51526,0,0,0,17329,0,0,0,43986,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11606,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,25963,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,33104,27864,0,0,0,0,0,0,10601,0,0,0,0,0,0,0,33569,0,0,0,23898,0,0,0],"mask":31,"len":20,"victim":null,"rng_state":11400714819323198517}"#;
const CMS: &str = r#"{"counters":[35,27,8,34,0,0,0,3,19,9,44,0,7,19,5,0,19,20,15,29,12,18,15,9,0,46,17,0,7,0,3,0,21,12,0,9,21,3,5,19,37,0,0,35,15,13,0,20,30,9,2,15,0,14,13,0,28,12,0,0,20,0,48,19],"width":16,"depth":4,"total":210}"#;
const HLL: &str = r#"{"registers":[3,1,4,2,4,2,5,4,4,5,6,5,4,3,5,1],"precision":4}"#;
const MINHASH: &str = r#"{"signature":[41696843600282850,126715314596376374,225162145250802453,45059234631396217,403212609750462638,1093539859634459832,586712198581745179,3524337910490618546]}"#;
const TOPK: &str = r#"{"sketch":{"counters":[0,0,15,0,8,0,0,0,0,0,0,0,0,3,0,0,9,0,0,0,0,7,0,2,6,4,1,0,0,0,0,0,0,0,5,2,10,0,0,9,14,1,0,0,0,4,0,0,0,7,0,0,0,3,0,0,1,0,0,0,0,0,9,0,4,3,0,5,0,0,0,10,0,0,0,0,2,15,0,0,0,0,0,6],"width":28,"depth":3,"total":55},"entries":[{"key":"key-9","count":10},{"key":"key-7","count":8},{"key":"key-8","count":9}],"k":3}"#;

fn key(i: u64) -> String {
    format!("key-{i}")
}

#[test]
fn test_fresh_state_serializes_like_1_0_0() {
    let mut bf = BloomFilter::<[u8]>::with_dimensions(256, 3).unwrap();
    for i in 0..20 {
        bf.insert(key(i).as_bytes());
    }
    assert_eq!(serde_json::to_string(&bf).unwrap(), BLOOM);

    let mut cf = CuckooFilter::<[u8]>::new(64).unwrap();
    for i in 0..20 {
        cf.insert(key(i).as_bytes()).unwrap();
    }
    assert_eq!(serde_json::to_string(&cf).unwrap(), CUCKOO);

    let mut cms = CountMinSketch::<str>::with_dimensions(16, 4).unwrap();
    for i in 0..20 {
        cms.add(&key(i), i + 1);
    }
    assert_eq!(serde_json::to_string(&cms).unwrap(), CMS);

    let mut hll = HyperLogLog::<u64>::new(4).unwrap();
    for i in 0..100u64 {
        hll.insert(&i);
    }
    assert_eq!(serde_json::to_string(&hll).unwrap(), HLL);

    let mut mh = MinHash::<str>::new(8).unwrap();
    for i in 0..20 {
        mh.insert(&key(i));
    }
    assert_eq!(serde_json::to_string(&mh).unwrap(), MINHASH);

    let mut tk = TopK::<String>::new(3, 0.1, 0.1).unwrap();
    for i in 0..10 {
        for _ in 0..=i {
            tk.insert(key(i));
        }
    }
    assert_eq!(serde_json::to_string(&tk).unwrap(), TOPK);
}

#[test]
fn test_1_0_0_state_decodes_and_answers_queries() {
    let bf: BloomFilter<[u8]> = serde_json::from_str(BLOOM).unwrap();
    let cf: CuckooFilter<[u8]> = serde_json::from_str(CUCKOO).unwrap();
    let cms: CountMinSketch<str> = serde_json::from_str(CMS).unwrap();
    for i in 0..20 {
        let k = key(i);
        assert!(bf.contains(k.as_bytes()), "bloom: false negative for {k}");
        assert!(cf.contains(k.as_bytes()), "cuckoo: false negative for {k}");
        assert!(cms.estimate(&k) > i, "count-min: undercount for {k}");
    }
    assert_eq!(serde_json::to_string(&bf).unwrap(), BLOOM);
    assert_eq!(serde_json::to_string(&cf).unwrap(), CUCKOO);
    assert_eq!(serde_json::to_string(&cms).unwrap(), CMS);

    let mut hll: HyperLogLog<u64> = serde_json::from_str(HLL).unwrap();
    let before = serde_json::to_string(&hll).unwrap();
    for i in 0..100u64 {
        hll.insert(&i);
    }
    // Re-inserting the same items leaves every register unchanged.
    assert_eq!(serde_json::to_string(&hll).unwrap(), before);
    assert_eq!(before, HLL);

    let mut mh: MinHash<str> = serde_json::from_str(MINHASH).unwrap();
    for i in 0..20 {
        mh.insert(&key(i));
    }
    assert_eq!(serde_json::to_string(&mh).unwrap(), MINHASH);

    let tk: TopK<String> = serde_json::from_str(TOPK).unwrap();
    assert_eq!(tk.top()[0].0, "key-9");
    assert_eq!(serde_json::to_string(&tk).unwrap(), TOPK);
}

#[test]
fn test_keyed_hash_values_are_portable() {
    // Recorded on x86_64 with 1.1.0, the release that added the keyed hasher.
    use bloom_lib::hash::KeyedHashBuilder;
    let b = KeyedHashBuilder::new(*b"0123456789abcdef");
    let got = [
        b.hash_one(&bytes(0)[..]),
        b.hash_one(&bytes(33)[..]),
        b.hash_one("user:42"),
        b.hash_one(0x0123_4567_89ab_cdefu64),
        b.hash_one(42usize),
        b.hash_one(-42isize),
        b.hash_one(0xbeefu16),
    ];
    assert_eq!(got, KEYED);
}

const KEYED: [u64; 7] = [
    0x8d9c_42c1_ab88_bf87,
    0xc413_c6c1_b3b4_578b,
    0xe123_1d10_e931_6e6d,
    0xe021_e2dc_52ac_43ed,
    0x3925_37a2_f448_3d5f,
    0xec93_bec3_a23f_e925,
    0xf52e_c348_b46c_53f6,
];
