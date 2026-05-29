//! End-to-end smoke tests exercised through the public crate API.

#[test]
fn version_is_set() {
    assert!(!bloom_lib::VERSION.is_empty());
}

#[cfg(feature = "alloc")]
#[test]
fn bloom_filter_end_to_end() {
    use bloom_lib::BloomFilter;

    let mut filter = BloomFilter::new(1_000, 0.01).expect("valid parameters");

    // No false negatives: every inserted item must test positive.
    for i in 0..1_000u32 {
        let newly = filter.insert(&i);
        assert!(
            newly,
            "item {i} reported as already present on first insert"
        );
    }
    for i in 0..1_000u32 {
        assert!(filter.contains(&i), "inserted item {i} reported absent");
    }

    // Re-inserting reports the item as already present.
    assert!(!filter.insert(&0u32));

    // Estimated cardinality lands close to the true count.
    let estimate = filter.estimated_len();
    assert!(
        (900..=1_100).contains(&estimate),
        "estimated_len {estimate} not within 10% of 1000"
    );

    // Clearing empties the filter while keeping its geometry.
    let bits = filter.num_bits();
    let hashes = filter.num_hashes();
    filter.clear();
    assert!(filter.is_empty());
    assert_eq!(filter.num_bits(), bits);
    assert_eq!(filter.num_hashes(), hashes);
}

#[cfg(feature = "alloc")]
#[test]
fn bloom_filter_merge_unions_membership() {
    use bloom_lib::BloomFilter;

    let mut a = BloomFilter::new(5_000, 0.01).expect("valid parameters");
    let mut b = BloomFilter::new(5_000, 0.01).expect("valid parameters");

    for i in 0..2_000u32 {
        let _ = a.insert(&i);
    }
    for i in 2_000..4_000u32 {
        let _ = b.insert(&i);
    }

    a.merge(&b).expect("identical geometry merges cleanly");

    for i in 0..4_000u32 {
        assert!(a.contains(&i), "merged filter missing item {i}");
    }
}

#[cfg(feature = "serde")]
#[test]
fn bloom_filter_round_trips_through_serde_json() {
    use bloom_lib::BloomFilter;

    let mut filter = BloomFilter::new(1_000, 0.01).expect("valid parameters");
    for i in 0..500u32 {
        let _ = filter.insert(&i);
    }

    let encoded = serde_json::to_vec(&filter).expect("serialization succeeds");
    let decoded: BloomFilter<u32> =
        serde_json::from_slice(&encoded).expect("deserialization succeeds");

    assert_eq!(decoded.num_bits(), filter.num_bits());
    assert_eq!(decoded.num_hashes(), filter.num_hashes());
    for i in 0..500u32 {
        assert!(
            decoded.contains(&i),
            "round-tripped filter missing item {i}"
        );
    }
}
