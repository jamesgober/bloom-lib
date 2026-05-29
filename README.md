<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <strong>bloom-lib</strong>
    <br>
    <sup><sub>PROBABILISTIC DATA STRUCTURES FOR RUST</sub></sup>
</h1>

<p align="center">
    <a href="https://crates.io/crates/bloom-lib"><img alt="crates.io" src="https://img.shields.io/crates/v/bloom-lib.svg"></a>
    <a href="https://crates.io/crates/bloom-lib" alt="Download bloom-lib"><img alt="Crates.io Downloads" src="https://img.shields.io/crates/d/bloom-lib?color=%230099ff"></a>
    <a href="https://docs.rs/bloom-lib"><img alt="docs.rs" src="https://docs.rs/bloom-lib/badge.svg"></a>
    <a href="https://github.com/jamesgober/bloom-lib/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/jamesgober/bloom-lib/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md" title="MSRV"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.75%2B-blue"></a>
</p>

<p align="center">Bloom filters, Cuckoo filters, Count-Min Sketch, HyperLogLog, MinHash, Top-K. Tunable false-positive rates, serializable state.</p>


## What it does

`bloom-lib` is a collection of probabilistic data structures: compact summaries
that answer questions about a data set — membership, cardinality, frequency,
similarity — with bounded, tunable error in a fraction of the memory an exact
structure would need. They are designed for streaming workloads: insertions are
allocation-free, state is serializable, and compatible structures can be merged.

The crate is `no_std`-friendly (it needs only `alloc`), carries no required
runtime dependency, and is generic over the hash function so you can trade the
fast deterministic default for an adversary-resistant one when inputs are
untrusted.

---

## Available structures

| Structure | Answers | Deletion | Mergeable |
|-----------|---------|----------|-----------|
| [`BloomFilter`](https://docs.rs/bloom-lib/latest/bloom_lib/struct.BloomFilter.html) | Is this item in the set? | — | ✅ |
| [`CuckooFilter`](https://docs.rs/bloom-lib/latest/bloom_lib/struct.CuckooFilter.html) | Is this item in the set? | ✅ | — |
| [`CountMinSketch`](https://docs.rs/bloom-lib/latest/bloom_lib/struct.CountMinSketch.html) | How often have I seen this item? | — | ✅ |
| [`HyperLogLog`](https://docs.rs/bloom-lib/latest/bloom_lib/struct.HyperLogLog.html) | How many distinct items are there? | — | ✅ |
| [`MinHash`](https://docs.rs/bloom-lib/latest/bloom_lib/struct.MinHash.html) | How similar are two sets? | — | ✅ |
| [`TopK`](https://docs.rs/bloom-lib/latest/bloom_lib/struct.TopK.html) | What are the most frequent items? | — | — |

Every structure is generic over the item type and the hash function, supports
`clear`, and (with the `serde` feature) serializes its state.

---

## Installation

```toml
[dependencies]
bloom-lib = "1.0"

# With serialization support:
bloom-lib = { version = "1.0", features = ["serde"] }

# On a heap-capable no_std target:
bloom-lib = { version = "1.0", default-features = false, features = ["alloc"] }
```

---

## Quick start

```rust
use bloom_lib::BloomFilter;

// Size for 100,000 distinct items at a 0.1% false-positive rate.
let mut filter = BloomFilter::new(100_000, 0.001).unwrap();

filter.insert("session-token");

assert!(filter.contains("session-token")); // never a false negative
assert!(!filter.contains("never-seen"));    // very likely absent
```

`insert` doubles as a deduplicating primitive: it returns `true` the first time
it sees an item and `false` when the item was probably already present.

```rust
use bloom_lib::BloomFilter;

let mut seen = BloomFilter::new(10_000, 0.01).unwrap();
assert!(seen.insert("first-visit"));   // newly added
assert!(!seen.insert("first-visit"));  // already present
```

---

## API overview

For the complete reference with parameters and worked examples, see
[`docs/API.md`](docs/API.md).

- [`BloomFilter`](docs/API.md#bloomfilter) — set membership with a tunable
  false-positive rate, merge support, and cardinality estimation.
- [`CuckooFilter`](docs/API.md#cuckoofilter) — set membership that also supports
  deletion.
- [`CountMinSketch`](docs/API.md#countminsketch) — per-item frequency estimation.
- [`HyperLogLog`](docs/API.md#hyperloglog) — distinct-count estimation in tiny,
  fixed memory.
- [`MinHash`](docs/API.md#minhash) — Jaccard similarity estimation between sets.
- [`TopK`](docs/API.md#topk) — the most frequent items (heavy hitters) in a stream.
- [`Error`](docs/API.md#error) — the crate-wide error type returned by fallible
  operations.
- [Hashing](docs/API.md#hashing) — `DefaultHasher`, `DefaultHashBuilder`, and the
  rationale for deterministic hashing.
- [Prelude](docs/API.md#prelude) — convenient glob re-exports.

---

## Hashing

Every structure is generic over [`core::hash::BuildHasher`] and defaults to the
deterministic `DefaultHashBuilder`. A fixed seed makes hashing reproducible:
filters built with the default hasher are always mergeable, and serialized state
round-trips byte-for-byte across runs and platforms. The trade-off is that a
fixed seed offers no defence against an adversary crafting colliding inputs. When
inputs are untrusted, supply a randomly-seeded hasher such as
`std::collections::hash_map::RandomState`:

```rust
use std::collections::hash_map::RandomState;
use bloom_lib::BloomFilter;

let filter: BloomFilter<&str, RandomState> =
    BloomFilter::with_hasher(1_000, 0.01, RandomState::new()).unwrap();
```

The default hasher is a fast, non-cryptographic 64-bit hash with strong
avalanche behaviour. From a single hashing pass the structures synthesise the
many index positions they need using the Kirsch–Mitzenmacher double-hashing
scheme, and map hashes into range with a division-free multiply-shift reduction.

[`core::hash::BuildHasher`]: https://doc.rust-lang.org/core/hash/trait.BuildHasher.html

---

## Performance

The hot paths are allocation-free and use only integer arithmetic: one hashing
pass per item, then a handful of multiply-shift index computations and array
probes. Floating-point math runs only at construction, via
[`libm`](https://crates.io/crates/libm), so a structure's derived geometry is
bit-identical on every platform.

Latest local Criterion means (`cargo bench --bench probabilistic`, Windows
x86_64, Rust stable, steady-state on a pre-populated structure):

| Operation | ns/op |
|-----------|------:|
| `BloomFilter::insert` | ~6.9 |
| `BloomFilter::contains` (hit) | ~5.5 |
| `CuckooFilter::contains` (hit) | ~6.0 |
| `CountMinSketch::increment` | ~5.8 |
| `CountMinSketch::estimate` | ~6.3 |
| `HyperLogLog::insert` | ~0.9 |
| `MinHash::insert` (128 hashes) | ~24 |
| `TopK::insert` (k = 100) | ~72 |

`MinHash` and `TopK` scale with their configured size (the signature length and
`k`, respectively); the others are effectively constant-time per operation. Run
`cargo bench` to reproduce these on your hardware.

---

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std`   | ✅ | Enables every structure and the `std::error::Error` impl for `Error`. Implies `alloc`. |
| `alloc` | — | Enables every structure without `std`, for heap-capable `no_std` targets. |
| `serde` | — | Derives `Serialize`/`Deserialize` for every structure. Implies `alloc`. |

With no features enabled the crate still compiles and exposes `VERSION` and
`Error`, so it can sit in a `no_std` dependency tree without pulling in a heap.

---

## Examples

Runnable examples live in [`examples/`](examples), one per structure:

| Example | Shows |
|---------|-------|
| [`bloom_dedup`](examples/bloom_dedup.rs) | Deduplicating a stream with a `BloomFilter`. |
| [`membership_with_deletion`](examples/membership_with_deletion.rs) | Granting and revoking membership with a `CuckooFilter`. |
| [`frequency`](examples/frequency.rs) | Per-item frequency estimation with a `CountMinSketch`. |
| [`cardinality`](examples/cardinality.rs) | Counting unique items with `HyperLogLog`. |
| [`similarity`](examples/similarity.rs) | Document similarity with `MinHash`. |
| [`top_words`](examples/top_words.rs) | Most frequent words with `TopK`. |

```bash
cargo run --example bloom_dedup --release
```

---

## Testing

```bash
# Default features
cargo test

# Every feature, including serde round-trips and property tests
cargo test --all-features

# Heap-only no_std build
cargo test --no-default-features --features alloc

# Microbenchmarks
cargo bench --bench probabilistic
```

---

## Cross-platform support

Tier-1 targets — Linux, macOS, and Windows (x86_64 and aarch64) — compile and
pass the full suite on both stable and the MSRV (Rust 1.75). CI runs formatting,
clippy with `-D warnings`, the test suite, and a documentation build with
`RUSTDOCFLAGS=-D warnings` across all three operating systems.

---


## Standards

- **REPS** governs every decision. See [REPS.md](REPS.md).
- **MSRV:** Rust 1.75.
- **Edition:** 2021.
- **Cross-platform:** Linux, macOS, Windows.

---

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

<!-- FOOT COPYRIGHT
################################################# -->
<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2025 <strong>JAMES GOBER.</strong></sup>
</div>
