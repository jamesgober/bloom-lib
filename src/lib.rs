//! # bloom-lib
//!
//! Probabilistic data structures for Rust.
//!
//! This crate provides space-efficient structures that answer set-membership,
//! cardinality, frequency, and similarity questions with bounded, tunable
//! error in a fraction of the memory an exact structure would require. They are
//! built for streaming workloads: insertions are allocation-free, state is
//! serializable, and compatible structures can be merged.
//!
//! ## Available structures
//!
//! - [`BloomFilter`] — probabilistic set membership with a tunable
//!   false-positive rate.
//! - [`CuckooFilter`] — approximate membership that also supports deletion.
//! - [`CountMinSketch`] — approximate frequency estimation for a stream.
//! - [`HyperLogLog`] — distinct-count (cardinality) estimation in tiny memory.
//! - [`MinHash`] — Jaccard similarity estimation between sets.
//! - [`TopK`] — the most frequent items (heavy hitters) in a stream.
//!
//! ## Example
//!
//! ```
//! # #[cfg(feature = "alloc")] {
//! use bloom_lib::BloomFilter;
//!
//! // A filter sized for 100,000 items at a 0.1% false-positive rate.
//! let mut filter = BloomFilter::new(100_000, 0.001).unwrap();
//!
//! filter.insert("session-token");
//! assert!(filter.contains("session-token"));
//! assert!(!filter.contains("never-seen"));
//! # }
//! ```
//!
//! ## Hashing
//!
//! Every structure is generic over [`core::hash::BuildHasher`] and defaults to
//! the deterministic [`hash::DefaultHashBuilder`]. Determinism makes filters
//! reproducible, mergeable, and stable across serialization, and the hash of a
//! given key is the same on every target. When an adversary can choose the
//! inputs, use [`hash::KeyedHashBuilder`] with a secret key. See the [`hash`]
//! module for details, including how to persist a keyed structure.
//!
//! ## Untrusted serialized state
//!
//! With the `serde` feature, deserializing checks every field against the
//! others and against the size of the data (bit count against word count,
//! register count against precision, table size against the bucket mask, and
//! so on) and fails with an error instead of producing a structure that
//! panics or loops on its first query. The work one query does is bounded by
//! a fixed cap on the hash count or by the size of the data. Validation cannot tell whether the content
//! is honest: a peer can still send a filter that says every item is present.
//!
//! ## Feature flags
//!
//! - `std` *(default)*: enables every structure and the
//!   [`std::error::Error`] implementation for [`Error`].
//! - `alloc`: enables every structure without requiring `std`, for
//!   heap-capable `no_std` targets. Implied by `std`.
//! - `serde`: implements `Serialize` and a validating `Deserialize` for every
//!   structure. Implies `alloc`.
//!
//! With none of these features the crate exposes only [`VERSION`] and [`Error`].
//!
//! ## License
//!
//! Dual-licensed under Apache-2.0 OR MIT.

#![doc(html_root_url = "https://docs.rs/bloom-lib")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::unreachable)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::missing_safety_doc)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod error;
pub mod hash;
#[cfg(feature = "alloc")]
mod util;

pub use crate::error::Error;

#[cfg(feature = "alloc")]
mod bit_set;
#[cfg(feature = "alloc")]
mod bloom;
#[cfg(feature = "alloc")]
mod count_min;
#[cfg(feature = "alloc")]
mod cuckoo;
#[cfg(feature = "alloc")]
mod hyperloglog;
#[cfg(feature = "alloc")]
mod minhash;
#[cfg(feature = "alloc")]
mod topk;

#[cfg(feature = "alloc")]
pub use crate::bloom::BloomFilter;
#[cfg(feature = "alloc")]
pub use crate::count_min::CountMinSketch;
#[cfg(feature = "alloc")]
pub use crate::cuckoo::CuckooFilter;
#[cfg(feature = "alloc")]
pub use crate::hyperloglog::HyperLogLog;
#[cfg(feature = "alloc")]
pub use crate::minhash::MinHash;
#[cfg(feature = "alloc")]
pub use crate::topk::TopK;

/// Convenient re-exports for typical usage.
///
/// Glob-importing the prelude brings the structures, the hashing types, and the
/// error type into scope:
///
/// ```
/// # #[cfg(feature = "alloc")] {
/// use bloom_lib::prelude::*;
///
/// let mut filter = BloomFilter::new(1_000, 0.01).unwrap();
/// filter.insert("hello");
/// assert!(filter.contains("hello"));
/// # }
/// ```
pub mod prelude {
    pub use crate::hash::{DefaultHashBuilder, DefaultHasher, KeyedHashBuilder, KeyedHasher};
    pub use crate::Error;

    #[cfg(feature = "alloc")]
    pub use crate::{BloomFilter, CountMinSketch, CuckooFilter, HyperLogLog, MinHash, TopK};
}

/// Crate version string, populated by Cargo at build time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
