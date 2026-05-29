//! Deduplicating a stream with a Bloom filter.
//!
//! A Bloom filter is a natural fit for "have I already processed this?" checks
//! over a large stream where keeping every key in a real set would be too
//! expensive. This example feeds a stream containing duplicates through a
//! filter and counts how many items it judged to be new.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example bloom_dedup --release
//! ```

use bloom_lib::BloomFilter;

fn main() {
    // Expect roughly 100,000 distinct keys, tolerate a 0.1% false-positive rate.
    let mut seen = BloomFilter::new(100_000, 0.001).expect("valid parameters");

    // A synthetic stream: the numbers 0..50,000, each emitted twice.
    let stream = (0..50_000u32).chain(0..50_000u32);

    let mut processed = 0u64;
    let mut skipped = 0u64;
    for key in stream {
        if seen.insert(&key) {
            processed += 1; // first time we have seen this key
        } else {
            skipped += 1; // probably a duplicate
        }
    }

    println!("processed (unique): {processed}");
    println!("skipped (duplicate): {skipped}");
    println!("estimated distinct keys: {}", seen.estimated_len());
    println!(
        "current false-positive rate: {:.4}%",
        seen.estimated_false_positive_rate() * 100.0
    );

    // The 50,000 second-pass items are all duplicates, so they should dominate
    // the skipped count.
    assert!(
        skipped >= 49_000,
        "expected most second-pass items to be skipped"
    );
}
