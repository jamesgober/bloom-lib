//! Tracking a changing set with a Cuckoo filter.
//!
//! A Cuckoo filter is a Bloom filter that supports deletion, which makes it the
//! right tool for a membership set whose contents come and go — active sessions,
//! cached keys, items in flight. This example simulates a small access-control
//! list that grants and revokes membership over time.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example membership_with_deletion --release
//! ```

use bloom_lib::CuckooFilter;

fn main() {
    let mut allowed = CuckooFilter::new(10_000).expect("valid capacity");

    // Grant access to a batch of tokens.
    for token in 0..5_000u64 {
        allowed.insert(&token).expect("filter has room");
    }
    println!("granted: {} tokens", allowed.len());

    // Revoke the even-numbered tokens.
    let mut revoked = 0u64;
    for token in (0..5_000u64).step_by(2) {
        if allowed.remove(&token) {
            revoked += 1;
        }
    }
    println!("revoked: {revoked} tokens");
    println!("remaining: {}", allowed.len());
    println!("load factor: {:.1}%", allowed.load_factor() * 100.0);

    // Odd tokens still pass; revoked even tokens are gone (no false negatives).
    assert!(allowed.contains(&1));
    assert!(allowed.contains(&4_999));
    assert!(!allowed.contains(&0));
    assert!(!allowed.contains(&2));

    println!("access checks behaved as expected");
}
