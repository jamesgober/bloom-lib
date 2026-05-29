//! Estimating per-item frequencies in a stream with a Count-Min Sketch.
//!
//! A Count-Min Sketch answers "how often have I seen this?" in fixed memory,
//! trading a small, one-sided overcount for not storing a counter per key. This
//! example feeds a skewed stream (a few hot keys, a long tail) and compares the
//! estimates against the exact counts.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example frequency --release
//! ```

use bloom_lib::CountMinSketch;

fn main() {
    // ~0.01% error with 99.99% confidence.
    let mut sketch = CountMinSketch::new(0.0001, 0.0001).expect("valid parameters");

    // Exact counts for a handful of tracked keys, to check the estimates.
    let mut exact = [0u64; 5];

    // A skewed stream: key 0 is hottest, key 4 is rarest, plus a noisy tail.
    for round in 0..100_000u64 {
        let key = round % 5; // 0..4, uniform here, weighted below
        let weight = 5 - key; // key 0 gets weight 5, key 4 gets weight 1
        sketch.add(&key, weight);
        exact[key as usize] += weight;

        // Long tail of unique keys that should not disturb the hot estimates.
        sketch.increment(&(1_000 + round));
    }

    println!("total weight added: {}", sketch.total_count());
    println!("key | exact | estimate | overcount");
    for key in 0..5u64 {
        let estimate = sketch.estimate(&key);
        let truth = exact[key as usize];
        println!(
            "  {key} | {truth:>5} | {estimate:>8} | {}",
            estimate - truth
        );
        // Count-Min never undercounts.
        assert!(estimate >= truth);
    }
}
