//! Counting unique items in a stream with HyperLogLog.
//!
//! Counting distinct values exactly means remembering every value seen. A
//! HyperLogLog estimates the count in a few kilobytes no matter how large the
//! stream is. This example streams values with heavy duplication and compares
//! the estimate against the known truth.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example cardinality --release
//! ```

use bloom_lib::HyperLogLog;

fn main() {
    // Precision 14: 16 KiB of registers, ~0.8% standard error.
    let mut hll = HyperLogLog::new(14).expect("valid precision");

    // 250,000 distinct values, but each emitted four times (1,000,000 events).
    const DISTINCT: u32 = 250_000;
    for _pass in 0..4 {
        for value in 0..DISTINCT {
            hll.insert(&value);
        }
    }

    let estimate = hll.count();
    let error = (estimate as f64 - f64::from(DISTINCT)).abs() / f64::from(DISTINCT);

    println!("events processed:   1,000,000");
    println!("true distinct count: {DISTINCT}");
    println!("estimated distinct:  {estimate}");
    println!("relative error:      {:.3}%", error * 100.0);
    println!("memory used:         {} bytes", 1usize << hll.precision());

    assert!(
        error < 0.03,
        "estimate drifted outside the expected envelope"
    );
}
