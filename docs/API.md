<h1 align="center">
    <b>bloom-lib</b>
    <br>
    <sub><sup>API REFERENCE</sup></sub>
</h1>
<div align="center">
    <sup>
        <a href="../README.md" title="Project Home"><b>HOME</b></a>
        <span>&nbsp;│&nbsp;</span>
        <span>API</span>
        <span>&nbsp;│&nbsp;</span>
        <a href="https://docs.rs/bloom-lib" title="docs.rs"><b>DOCS.RS</b></a>
    </sup>
</div>
<br>

This reference documents the complete public surface of `bloom-lib` as of
**v1.0.0**. Every public type, method, and parameter is described, with runnable
examples for each use case. The same content is available inline on
[docs.rs](https://docs.rs/bloom-lib).

## Table of Contents

- [Installation](#installation)
- [Feature flags](#feature-flags)
- [Quick start](#quick-start)
- [Public API](#public-api)
  - [`BloomFilter`](#bloomfilter)
    - [Construction](#bloomfilter-construction)
    - [Membership operations](#bloomfilter-membership)
    - [Introspection & estimation](#bloomfilter-introspection)
    - [Merging](#bloomfilter-merging)
    - [Serialization](#bloomfilter-serialization)
  - [`CuckooFilter`](#cuckoofilter)
  - [`CountMinSketch`](#countminsketch)
  - [`HyperLogLog`](#hyperloglog)
  - [`MinHash`](#minhash)
  - [`TopK`](#topk)
  - [Hashing](#hashing)
    - [`DefaultHasher`](#defaulthasher)
    - [`DefaultHashBuilder`](#defaulthashbuilder)
  - [`Error`](#error)
  - [`VERSION`](#version)
  - [Prelude](#prelude)
- [Compatibility](#compatibility)
- [Notes](#notes)

<br>

## Installation

```toml
[dependencies]
bloom-lib = "1.0"
```

With serialization, or on a heap-capable `no_std` target:

```toml
bloom-lib = { version = "1.0", features = ["serde"] }
bloom-lib = { version = "1.0", default-features = false, features = ["alloc"] }
```

<br>

## Feature flags

| Feature | Default | Effect |
|---------|---------|--------|
| `std`   | yes | Enables every structure and `impl std::error::Error for Error`. Implies `alloc`. |
| `alloc` | no  | Enables every structure on heap-capable `no_std` targets. |
| `serde` | no  | Derives `Serialize`/`Deserialize` for every structure. Implies `alloc`. |

With no features the crate exposes only [`VERSION`](#version) and
[`Error`](#error).

<br>

## Quick start

```rust
use bloom_lib::BloomFilter;

let mut filter = BloomFilter::new(100_000, 0.001).unwrap();
filter.insert("session-token");
assert!(filter.contains("session-token"));
assert!(!filter.contains("never-seen"));
```

<br>

## Public API

<h3 id="bloomfilter"><code>BloomFilter&lt;T, S = DefaultHashBuilder&gt;</code></h3>

Source: `src/bloom.rs`

A space-efficient probabilistic set. `contains` never reports a false negative
(an inserted item always tests positive) but may report a false positive with a
probability you tune at construction time. Items are not stored, so a Bloom
filter cannot enumerate or delete its contents.

**Type parameters**

- `T` — the item type. Operations require `T: Hash`; `T` may be unsized (for
  example `str` or `[u8]`).
- `S` — the [`BuildHasher`](https://doc.rust-lang.org/core/hash/trait.BuildHasher.html)
  used to hash items. Defaults to [`DefaultHashBuilder`](#defaulthashbuilder).

**Derives:** `Debug`, `Clone`, and — under the `serde` feature —
`Serialize`/`Deserialize`.

<h4 id="bloomfilter-construction">Construction</h4>

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(capacity: usize, rate: f64) -> Result<Self, Error>` | Sizes the filter for `capacity` distinct items at false-positive `rate`, using the default hasher. |
| `with_dimensions` | `fn with_dimensions(num_bits: u64, num_hashes: u32) -> Result<Self, Error>` | Builds a filter with an explicit bit count and hash count. |
| `with_hasher` | `fn with_hasher(capacity: usize, rate: f64, hasher: S) -> Result<Self, Error>` | Like `new`, but with a caller-supplied hasher. |
| `with_dimensions_and_hasher` | `fn with_dimensions_and_hasher(num_bits: u64, num_hashes: u32, hasher: S) -> Result<Self, Error>` | Explicit geometry with a caller-supplied hasher. |

**Parameters**

- `capacity`: the number of distinct items you expect to insert. Must be
  non-zero. The false-positive rate is honoured at this fill level and degrades
  gracefully beyond it.
- `rate`: the target false-positive probability. Must be a finite value in the
  open interval `(0.0, 1.0)`.
- `num_bits`: the exact size of the bit array. Must be non-zero.
- `num_hashes`: the number of hash positions probed per item. Must be non-zero.
- `hasher`: any `BuildHasher`. Use a randomly-seeded one (such as
  `RandomState`) when inputs are adversarial.

**Errors:** [`Error::InvalidParameter`](#error) when `capacity`/`num_bits`/
`num_hashes` is zero or `rate` is outside `(0.0, 1.0)` or non-finite.

```rust
use bloom_lib::BloomFilter;

// Derive geometry from a capacity and rate.
let filter = BloomFilter::<&str>::new(10_000, 0.01).unwrap();
assert!(filter.num_bits() >= 10_000);
assert_eq!(filter.num_hashes(), 7);

// Or specify the geometry directly.
let exact = BloomFilter::<u64>::with_dimensions(8_192, 5).unwrap();
assert_eq!(exact.num_bits(), 8_192);
assert_eq!(exact.num_hashes(), 5);
```

```rust
// Adversary-resistant hashing with a randomly-seeded BuildHasher.
use std::collections::hash_map::RandomState;
use bloom_lib::BloomFilter;

let filter: BloomFilter<&str, RandomState> =
    BloomFilter::with_hasher(1_000, 0.01, RandomState::new()).unwrap();
```

<h4 id="bloomfilter-membership">Membership operations</h4>

| Method | Signature | Description |
|--------|-----------|-------------|
| `insert` | `fn insert(&mut self, item: &T) -> bool` | Inserts `item`; returns `true` if it was probably not present before. |
| `contains` | `fn contains(&self, item: &T) -> bool` | `false` ⇒ definitely absent; `true` ⇒ probably present. |
| `clear` | `fn clear(&mut self)` | Removes every item, keeping the allocation and geometry. |

`insert` returns a novelty flag, which makes it a one-line deduplicator: `true`
the first time an item is seen, `false` once its bits are all set.

```rust
use bloom_lib::BloomFilter;

let mut filter = BloomFilter::new(100, 0.01).unwrap();

assert!(filter.insert("alpha"));   // newly added
assert!(!filter.insert("alpha"));  // already present

assert!(filter.contains("alpha"));
assert!(!filter.contains("omega"));

filter.clear();
assert!(!filter.contains("alpha"));
```

<h4 id="bloomfilter-introspection">Introspection & estimation</h4>

| Method | Signature | Description |
|--------|-----------|-------------|
| `num_bits` | `fn num_bits(&self) -> u64` | Size of the underlying bit array (`m`). |
| `num_hashes` | `fn num_hashes(&self) -> u32` | Hash positions probed per item (`k`). |
| `count_ones` | `fn count_ones(&self) -> u64` | Number of set bits — raw fill level. |
| `is_empty` | `fn is_empty(&self) -> bool` | `true` when no bits are set. |
| `estimated_len` | `fn estimated_len(&self) -> u64` | Estimated number of distinct items inserted. |
| `estimated_false_positive_rate` | `fn estimated_false_positive_rate(&self) -> f64` | Estimated false-positive probability at the current fill. |

`estimated_len` uses the set-bit estimator `n ≈ -(m/k)·ln(1 − X/m)`;
`estimated_false_positive_rate` uses `(X/m)^k`, where `X` is the set-bit count.
Both are approximate and lose accuracy as the filter saturates.

```rust
use bloom_lib::BloomFilter;

let mut filter = BloomFilter::new(10_000, 0.01).unwrap();
for i in 0..1_000u32 {
    filter.insert(&i);
}

// Within a few percent of the true count of 1,000.
let estimate = filter.estimated_len();
assert!((900..=1_100).contains(&estimate));

// Well under 1% at a tenth of the design capacity.
assert!(filter.estimated_false_positive_rate() < 0.01);
```

<h4 id="bloomfilter-merging">Merging</h4>

| Method | Signature | Description |
|--------|-----------|-------------|
| `merge` | `fn merge(&mut self, other: &Self) -> Result<(), Error>` | Unions `other` into `self` (bitwise OR of the bit arrays). |

Both filters must share their geometry (bit count and hash count). After a
successful merge, the filter tests positive for every item that was in either
operand.

**Errors:** [`Error::IncompatibleParameters`](#error) when the two filters differ
in bit count or hash count.

```rust
use bloom_lib::BloomFilter;

let mut a = BloomFilter::new(1_000, 0.01).unwrap();
let mut b = BloomFilter::new(1_000, 0.01).unwrap();
a.insert("from-a");
b.insert("from-b");

a.merge(&b).unwrap();
assert!(a.contains("from-a"));
assert!(a.contains("from-b"));

// Mismatched geometry is rejected.
let c = BloomFilter::<&str>::with_dimensions(2_048, 3).unwrap();
let mut d = BloomFilter::<&str>::with_dimensions(4_096, 3).unwrap();
assert!(d.merge(&c).is_err());
```

<h4 id="bloomfilter-serialization">Serialization (<code>serde</code> feature)</h4>

With the `serde` feature, `BloomFilter` derives `Serialize` and `Deserialize`.
The bit array and geometry are serialized; the hasher is not (it is rebuilt with
`S::default()` on deserialization, which is exactly correct for the stateless
`DefaultHashBuilder`). Because the default hasher is deterministic, a filter
serialized on one machine queries identically on another.

```rust
# #[cfg(feature = "serde")] {
use bloom_lib::BloomFilter;

let mut filter = BloomFilter::new(1_000, 0.01).unwrap();
filter.insert(&7u32);

let bytes = serde_json::to_vec(&filter).unwrap();
let restored: BloomFilter<u32> = serde_json::from_slice(&bytes).unwrap();

assert!(restored.contains(&7u32));
assert_eq!(restored.num_bits(), filter.num_bits());
# }
```

<br>

<h3 id="cuckoofilter"><code>CuckooFilter&lt;T, S = DefaultHashBuilder&gt;</code></h3>

Source: `src/cuckoo.rs`

An approximate-membership filter that also supports **deletion**. It stores a
16-bit fingerprint of each item in one of two candidate buckets, relocating
residents as needed. Like a Bloom filter it never reports a false negative for a
present item; its false-positive rate is fixed by the fingerprint width at
roughly 0.012%. Use it instead of a `BloomFilter` when items must be removed; use
a `BloomFilter` when you need a tunable rate and never delete.

**Type parameters:** `T` (item type, `T: Hash`, may be unsized) and `S`
(`BuildHasher`, default `DefaultHashBuilder`).

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(capacity: usize) -> Result<Self, Error>` | Builds a filter able to hold roughly `capacity` items (default hasher). |
| `with_hasher` | `fn with_hasher(capacity: usize, hasher: S) -> Result<Self, Error>` | As `new`, with a caller-supplied hasher. |
| `insert` | `fn insert(&mut self, item: &T) -> Result<(), Error>` | Adds `item`; `Err(CapacityExceeded)` when the filter is too full. |
| `contains` | `fn contains(&self, item: &T) -> bool` | `false` ⇒ definitely absent; `true` ⇒ probably present. |
| `remove` | `fn remove(&mut self, item: &T) -> bool` | Removes one occurrence; `true` if present. |
| `len` / `is_empty` | `fn len(&self) -> usize` / `fn is_empty(&self) -> bool` | Number of stored fingerprints. |
| `capacity` | `fn capacity(&self) -> usize` | Maximum fingerprints (`num_buckets * 4`). |
| `load_factor` | `fn load_factor(&self) -> f64` | Current fill in `[0.0, 1.0]`. |
| `clear` | `fn clear(&mut self)` | Removes every item. |

**Parameters:** `capacity` must be non-zero; the bucket count is rounded up to a
power of two with headroom, so `capacity()` is at least `capacity`. Inserting
beyond the practical load can return `Error::CapacityExceeded` — size with
margin. Duplicates are stored separately and require one `remove` each.

**Errors:** `Error::InvalidParameter` (zero capacity), `Error::CapacityExceeded`
(insertion could not find a slot).

```rust
use bloom_lib::CuckooFilter;

let mut filter = CuckooFilter::new(10_000).unwrap();

filter.insert("alice").unwrap();
assert!(filter.contains("alice"));

assert!(filter.remove("alice"));
assert!(!filter.contains("alice"));
```

```rust
// Duplicates require matching removals.
use bloom_lib::CuckooFilter;

let mut filter = CuckooFilter::new(100).unwrap();
filter.insert(&7u32).unwrap();
filter.insert(&7u32).unwrap();
assert_eq!(filter.len(), 2);
assert!(filter.remove(&7u32));
assert!(filter.contains(&7u32)); // one copy remains
```

<br>

<h3 id="countminsketch"><code>CountMinSketch&lt;T, S = DefaultHashBuilder&gt;</code></h3>

Source: `src/count_min.rs`

A sublinear-space frequency estimator. Each item is hashed into one counter per
row; the estimate is the minimum across rows. It **never undercounts** and
overcounts by a bounded amount with high probability. Counters saturate at
`u64::MAX` rather than overflowing.

**Type parameters:** `T` (`T: Hash`, may be unsized) and `S` (`BuildHasher`).

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(epsilon: f64, delta: f64) -> Result<Self, Error>` | Sizes width `ceil(e/epsilon)` and depth `ceil(ln(1/delta))`. |
| `with_dimensions` | `fn with_dimensions(width: usize, depth: usize) -> Result<Self, Error>` | Explicit grid geometry. |
| `with_hasher` | `fn with_hasher(epsilon: f64, delta: f64, hasher: S) -> Result<Self, Error>` | As `new`, with a caller-supplied hasher. |
| `with_dimensions_and_hasher` | `fn with_dimensions_and_hasher(width, depth, hasher) -> Result<Self, Error>` | Explicit geometry + hasher. |
| `add` | `fn add(&mut self, item: &T, count: u64)` | Records `count` occurrences (saturating). |
| `increment` | `fn increment(&mut self, item: &T)` | Shorthand for `add(item, 1)`. |
| `estimate` | `fn estimate(&self, item: &T) -> u64` | Estimated frequency (never below the truth). |
| `total_count` | `fn total_count(&self) -> u64` | Exact sum of every count added (saturating). |
| `width` / `depth` | `fn width(&self) -> usize` / `fn depth(&self) -> usize` | Grid dimensions. |
| `merge` | `fn merge(&mut self, other: &Self) -> Result<(), Error>` | Cell-wise saturating sum; requires matching geometry. |
| `clear` | `fn clear(&mut self)` | Resets all counters. |

**Parameters:** `epsilon` (error factor) and `delta` (failure probability) must
each be finite and in `(0.0, 1.0)`; `width`/`depth` must be non-zero.

**Errors:** `Error::InvalidParameter` (bad arguments), `Error::IncompatibleParameters`
(merge with mismatched geometry).

```rust
use bloom_lib::CountMinSketch;

let mut sketch = CountMinSketch::new(0.001, 0.001).unwrap();
sketch.increment("apple");
sketch.add("apple", 4);
assert!(sketch.estimate("apple") >= 5); // never undercounts
assert_eq!(sketch.estimate("cherry"), 0);
```

```rust
// Merge two shards' counts.
use bloom_lib::CountMinSketch;

let mut a = CountMinSketch::with_dimensions(512, 4).unwrap();
let mut b = CountMinSketch::with_dimensions(512, 4).unwrap();
a.add("shared", 2);
b.add("shared", 3);
a.merge(&b).unwrap();
assert!(a.estimate("shared") >= 5);
```

<br>

<h3 id="hyperloglog"><code>HyperLogLog&lt;T, S = DefaultHashBuilder&gt;</code></h3>

Source: `src/hyperloglog.rs`

Estimates the number of **distinct** items in a stream using `2^precision`
one-byte registers, with a standard error of about `1.04 / sqrt(2^precision)`.
Memory is fixed regardless of cardinality. A bias-corrected estimator is used,
falling back to linear counting at low cardinalities.

**Type parameters:** `T` (`T: Hash`, may be unsized) and `S` (`BuildHasher`).

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(precision: u8) -> Result<Self, Error>` | `precision` in `4..=18`; uses `2^precision` registers. |
| `with_error_rate` | `fn with_error_rate(error: f64) -> Result<Self, Error>` | Picks the smallest precision meeting `error`, clamped to `4..=18`. |
| `with_hasher` | `fn with_hasher(precision: u8, hasher: S) -> Result<Self, Error>` | As `new`, with a caller-supplied hasher. |
| `insert` | `fn insert(&mut self, item: &T)` | Adds an item; idempotent for distinct values. |
| `count` | `fn count(&self) -> u64` | Estimated distinct count. |
| `is_empty` | `fn is_empty(&self) -> bool` | `true` if nothing inserted. |
| `precision` | `fn precision(&self) -> u8` | The configured precision. |
| `merge` | `fn merge(&mut self, other: &Self) -> Result<(), Error>` | Register-wise max (union); requires equal precision. |
| `clear` | `fn clear(&mut self)` | Resets the estimator. |

**Parameters:** `precision` must be in `4..=18`; `error` must be finite and in
`(0.0, 1.0)`.

**Errors:** `Error::InvalidParameter` (precision/error out of range),
`Error::IncompatibleParameters` (merge with different precision).

```rust
use bloom_lib::HyperLogLog;

let mut hll = HyperLogLog::new(14).unwrap();
for i in 0..100_000u32 {
    hll.insert(&i);
}
let estimate = hll.count();
assert!((97_000..=103_000).contains(&estimate)); // ~0.8% error at p=14
```

```rust
// Union two estimators by merging.
use bloom_lib::HyperLogLog;

let mut a = HyperLogLog::new(14).unwrap();
let mut b = HyperLogLog::new(14).unwrap();
for i in 0..1_000u32 { a.insert(&i); }
for i in 500..1_500u32 { b.insert(&i); }
a.merge(&b).unwrap();
assert!((1_400..=1_600).contains(&a.count())); // union ~1,500
```

<br>

<h3 id="minhash"><code>MinHash&lt;T, S = DefaultHashBuilder&gt;</code></h3>

Source: `src/minhash.rs`

Estimates the [Jaccard similarity](https://en.wikipedia.org/wiki/Jaccard_index)
`|A ∩ B| / |A ∪ B|` of two sets by comparing fixed-length signatures, in `O(k)`
time and space regardless of set size. More hash functions tighten the estimate
(standard error ≈ `1 / sqrt(num_hashes)`).

**Type parameters:** `T` (`T: Hash`, may be unsized) and `S` (`BuildHasher`).

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(num_hashes: usize) -> Result<Self, Error>` | Signature length `num_hashes` (non-zero). |
| `with_hasher` | `fn with_hasher(num_hashes: usize, hasher: S) -> Result<Self, Error>` | As `new`, with a caller-supplied hasher. |
| `insert` | `fn insert(&mut self, item: &T)` | Adds a member; idempotent for distinct values. |
| `similarity` | `fn similarity(&self, other: &Self) -> Result<f64, Error>` | Estimated Jaccard similarity in `[0, 1]`. |
| `merge` | `fn merge(&mut self, other: &Self) -> Result<(), Error>` | Slot-wise min (set union); requires equal length. |
| `num_hashes` | `fn num_hashes(&self) -> usize` | Signature length. |
| `is_empty` | `fn is_empty(&self) -> bool` | `true` if nothing inserted. |
| `clear` | `fn clear(&mut self)` | Resets the sketch. |

**Parameters:** `num_hashes` must be non-zero. Two sketches are comparable only
when built with the same length and hasher.

**Errors:** `Error::InvalidParameter` (zero length), `Error::IncompatibleParameters`
(`similarity`/`merge` with mismatched length).

```rust
use bloom_lib::MinHash;

let mut a = MinHash::new(256).unwrap();
let mut b = MinHash::new(256).unwrap();
for w in ["the", "quick", "brown", "fox"] { a.insert(w); }
for w in ["the", "quick", "red", "fox"] { b.insert(w); }

// True Jaccard is 3/5 = 0.6.
let similarity = a.similarity(&b).unwrap();
assert!((0.45..=0.75).contains(&similarity));
```

<br>

<h3 id="topk"><code>TopK&lt;T, S = DefaultHashBuilder&gt;</code></h3>

Source: `src/topk.rs`

Tracks the `k` most frequent items in a stream using a `CountMinSketch` plus a
monitored list of the current heavy hitters. Memory is bounded by the sketch
size plus `k` stored keys. Unlike the other structures it stores the keys, so
`T: Eq + Hash` and must be sized; a key is moved into the list only on promotion.

**Type parameters:** `T` (`T: Eq + Hash`, sized) and `S` (`BuildHasher`).

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(k: usize, epsilon: f64, delta: f64) -> Result<Self, Error>` | `k` heavy hitters; sketch sized by `epsilon`/`delta`. |
| `with_hasher` | `fn with_hasher(k, epsilon, delta, hasher: S) -> Result<Self, Error>` | As `new`, with a caller-supplied hasher. |
| `insert` | `fn insert(&mut self, item: T)` | Records one occurrence (by value). |
| `estimate` | `fn estimate(&self, item: &T) -> u64` | Estimated frequency from the sketch (any item). |
| `top` | `fn top(&self) -> Vec<(&T, u64)>` | Monitored items + counts, most frequent first. |
| `k` | `fn k(&self) -> usize` | The configured `k`. |
| `len` / `is_empty` | `fn len(&self) -> usize` / `fn is_empty(&self) -> bool` | Items currently monitored. |
| `clear` | `fn clear(&mut self)` | Clears the sketch and the monitored set. |

**Parameters:** `k` must be non-zero; `epsilon`/`delta` must be finite and in
`(0.0, 1.0)`. The top-set membership is approximate — stable heavy hitters are
reported reliably, but a hitter under heavy churn can be missed.

**Errors:** `Error::InvalidParameter` (zero `k` or bad sketch parameters).

```rust
use bloom_lib::TopK;

let mut top = TopK::new(3, 0.001, 0.001).unwrap();
for _ in 0..100 { top.insert("apple"); }
for _ in 0..50  { top.insert("banana"); }
for _ in 0..10  { top.insert("cherry"); }
for _ in 0..1   { top.insert("date"); }

let ranked = top.top();
assert_eq!(ranked[0].0, &"apple");
assert_eq!(ranked.len(), 3); // only k kept
```

<br>

<h3 id="hashing">Hashing</h3>

Source: `src/hash.rs`

Every structure is generic over [`core::hash::BuildHasher`] and defaults to the
deterministic [`DefaultHashBuilder`](#defaulthashbuilder). Deterministic hashing
makes filters reproducible, mergeable, and stable across serialization. Substitute
a randomly-seeded hasher when inputs are untrusted.

<h4 id="defaulthasher"><code>DefaultHasher</code></h4>

A fast, non-cryptographic 64-bit hasher implementing
[`core::hash::Hasher`](https://doc.rust-lang.org/core/hash/trait.Hasher.html). It
folds input through a 64×64-bit multiply-fold and mixes the input length into the
finalizer, so values differing only by trailing zero bytes do not collide. Not
suitable for authentication.

| Method | Signature | Description |
|--------|-----------|-------------|
| `with_seed` | `const fn with_seed(seed: u64) -> Self` | Creates a hasher with an explicit seed. |
| `default` | `fn default() -> Self` | Creates a hasher seeded with the fixed library constant. |

```rust
use core::hash::{Hash, Hasher};
use bloom_lib::hash::DefaultHasher;

let mut a = DefaultHasher::default();
"probabilistic".hash(&mut a);

let mut b = DefaultHasher::default();
"probabilistic".hash(&mut b);

assert_eq!(a.finish(), b.finish()); // deterministic
```

<h4 id="defaulthashbuilder"><code>DefaultHashBuilder</code></h4>

A zero-sized [`BuildHasher`] that yields `DefaultHasher` instances seeded with a
fixed constant. This is the default hasher for every structure. It derives
`Debug`, `Clone`, `Copy`, `Default`, and — under `serde` — `Serialize`/`Deserialize`.

```rust
use core::hash::BuildHasher;
use bloom_lib::hash::DefaultHashBuilder;

let builder = DefaultHashBuilder::default();
assert_eq!(builder.hash_one("key"), builder.hash_one("key"));
```

<br>

<h3 id="error"><code>Error</code></h3>

Source: `src/error.rs`

The crate-wide error type. It is `#[non_exhaustive]`, so a `match` must include a
wildcard arm. It implements `Debug`, `Clone`, `PartialEq`, `Eq`, `Display`, and —
with the `std` feature — `std::error::Error`. It holds only `&'static str` data,
so it is allocation-free and usable on `no_std`.

| Variant | Meaning |
|---------|---------|
| `InvalidParameter { param: &'static str, reason: &'static str }` | A tuning parameter fell outside its valid range. |
| `IncompatibleParameters` | Two structures could not be combined because their parameters differ. |
| `CapacityExceeded` | An insertion failed because the structure is full (Cuckoo filter). |

```rust
use bloom_lib::{BloomFilter, Error};

let err = BloomFilter::<&str>::new(1_000, 0.0).unwrap_err();
assert!(matches!(err, Error::InvalidParameter { .. }));
println!("{err}"); // invalid parameter `rate`: must be a finite value ...
```

<br>

<h3 id="version"><code>VERSION</code></h3>

```rust
pub const VERSION: &str;
```

The crate version string, populated by Cargo at build time. Available in every
feature configuration, including bare `no_std`.

```rust
assert!(!bloom_lib::VERSION.is_empty());
```

<br>

<h3 id="prelude">Prelude</h3>

`bloom_lib::prelude` re-exports the common types for glob import:

```rust
use bloom_lib::prelude::*;

let mut filter = BloomFilter::new(1_000, 0.01).unwrap();
filter.insert("hello");
assert!(filter.contains("hello"));
```

The prelude includes `Error`, `DefaultHasher`, `DefaultHashBuilder`, and (with
`alloc`) `BloomFilter`, `CuckooFilter`, `CountMinSketch`, `HyperLogLog`,
`MinHash`, and `TopK`.

<br>

## Compatibility

**1.0.0 freezes the public API.** From this release the crate follows
[Semantic Versioning](https://semver.org/) strictly: no breaking change to the
surface below ships without a major version bump.

The frozen surface is:

- **Structures** (under `alloc`): `BloomFilter`, `CuckooFilter`,
  `CountMinSketch`, `HyperLogLog`, `MinHash`, `TopK` — including their public
  methods and the default `S = DefaultHashBuilder` type parameter.
- **Error**: `Error` and its variants. The enum is `#[non_exhaustive]`, so new
  variants may be **added** in a minor release; downstream `match`es must already
  carry a wildcard arm.
- **Hashing**: `DefaultHasher`, `DefaultHashBuilder`.
- **Top level**: `VERSION` and the `prelude` module.
- **Features**: `std` (default), `alloc`, `serde`, with their documented
  implications.

What is **not** covered by the SemVer promise, and may change in a minor
release:

- The exact bit/register layout produced by a given constructor (the *sizing
  math*), and therefore the precise byte content of serialized state. Serialized
  data is portable across platforms of the same release, not guaranteed across
  major-version sizing changes.
- The exact hash values produced by `DefaultHasher` (it is not a stable hash for
  cross-version persistence; only its statistical quality is part of the
  contract).
- Estimation results to the last digit; only the documented error bounds are
  guaranteed.

The MSRV (Rust 1.75) is treated as a compatibility surface: raising it is a
minor-version change, announced in the changelog.

<br>

## Notes

- **Determinism.** The default hasher uses a fixed seed. This is what makes
  filters mergeable and serialized state portable, but it is not a defence
  against adversarial collisions; use a randomly-seeded `BuildHasher` for
  untrusted input.
- **No false negatives.** A Bloom filter never reports an inserted item as
  absent. False positives are the only error mode, and their rate is what you
  configure.
- **Saturation.** Inserting far beyond the configured capacity raises the
  false-positive rate; the estimation helpers also lose accuracy. Size for the
  expected load, or use `estimated_false_positive_rate` to monitor fill.

[`core::hash::BuildHasher`]: https://doc.rust-lang.org/core/hash/trait.BuildHasher.html
