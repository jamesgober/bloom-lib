//! Estimating document similarity with MinHash.
//!
//! MinHash summarises each set as a fixed-length signature and estimates the
//! Jaccard similarity of two sets by comparing their signatures — without ever
//! intersecting the sets directly. This example splits two short documents into
//! word sets and estimates how alike they are.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example similarity --release
//! ```

use bloom_lib::MinHash;

const DOC_A: &str = "the quick brown fox jumps over the lazy dog";
const DOC_B: &str = "the quick brown cat jumps over the sleepy dog";
const DOC_C: &str = "completely unrelated text about distributed systems";

fn sketch(text: &str) -> MinHash<&str> {
    let mut sketch = MinHash::new(256).expect("valid signature length");
    for word in text.split_whitespace() {
        sketch.insert(&word);
    }
    sketch
}

fn main() {
    let a = sketch(DOC_A);
    let b = sketch(DOC_B);
    let c = sketch(DOC_C);

    let ab = a.similarity(&b).expect("equal signature lengths");
    let ac = a.similarity(&c).expect("equal signature lengths");

    println!("A vs B (near-duplicate): {:.2}", ab);
    println!("A vs C (unrelated):      {:.2}", ac);

    // Near-duplicates score far higher than unrelated text.
    assert!(ab > ac);
    assert!(
        ab > 0.4,
        "expected the near-duplicates to be clearly similar"
    );
}
