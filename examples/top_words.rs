//! Finding the most frequent words in a text with a Top-K tracker.
//!
//! `TopK` keeps only the `k` heaviest hitters plus a small sketch, so it scales
//! to streams far larger than memory while still naming the top items. This
//! example tokenises a passage and reports the five most common words.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example top_words --release
//! ```

use bloom_lib::TopK;

const PASSAGE: &str = "
the quick brown fox jumps over the lazy dog
the dog was not amused but the fox was quick
the fox the fox the quick quick brown fox
a dog a dog a lazy lazy dog over the moon
";

fn main() {
    // Track the top 5 words with a high-accuracy sketch.
    let mut top = TopK::new(5, 0.0001, 0.0001).expect("valid parameters");

    let mut total = 0u64;
    for word in PASSAGE.split_whitespace() {
        top.insert(word);
        total += 1;
    }

    println!("total words: {total}");
    println!("top 5 words by frequency:");
    for (rank, (word, count)) in top.top().into_iter().enumerate() {
        println!("  {}. {:<8} (~{} occurrences)", rank + 1, word, count);
    }

    // "the" is by far the most common token in the passage.
    let ranked = top.top();
    assert_eq!(ranked[0].0, &"the");
}
