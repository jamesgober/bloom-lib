# Changelog

All notable changes to `bloom-lib` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [1.1.0] - 2026-11-12

Fixes from an independent review: hashing is now the same on every target,
deserialization validates its input, oversized constructor parameters return
errors, and a keyed hasher is available for untrusted input. No breaking API
change and no MSRV change (still Rust 1.75).

### Compatibility

- **Hash values and serialized state on x86_64 and aarch64 are unchanged.**
  `tests/golden.rs` pins hash values for byte slices, strings and every integer
  type, and the full serialized form of every structure, to what 1.0.0
  produced on x86_64. Filters written there by 1.0.0 decode and answer queries
  exactly as before.
- On 32-bit and big-endian targets, hash values change to match x86_64 (see
  Fixed). State that 1.0.0 serialized *on those targets* gives false negatives
  under 1.1.0; rebuild it. State serialized on x86_64 or aarch64 now works on
  them, which it did not before.
- From 1.1.0 the hash values and the serialized form are held fixed for the
  rest of 1.x (see `docs/API.md#compatibility`).

### Added

- `hash::KeyedHasher` and `hash::KeyedHashBuilder`: SipHash-2-4 with a 128-bit
  secret key, for inputs an adversary can choose. `KeyedHashBuilder::new`,
  `random` (with `std`) and `key`. Neither type prints its key in `Debug`.
  `KeyedHashBuilder` has no `Default`, so a keyed structure cannot be
  deserialized with a made-up key. Both are in the prelude.
- `replace_hasher` on every structure, to attach the original key after
  deserializing a keyed structure (serialized state never includes the
  hasher).
- Golden compatibility tests, tests for hostile serialized state, constructor
  limits and the keyed hasher, and a CI job that runs the test suite on
  `i686-unknown-linux-gnu` and big-endian `s390x-unknown-linux-gnu`.

### Changed

- `BloomFilter::with_dimensions` and `with_dimensions_and_hasher` reject
  `num_hashes` above 2,048 with `Error::InvalidParameter`. The sizing formulas
  never produce more than about 1,075, so `new` and `with_hasher` are not
  affected.
- Docs: the `RandomState` advice is replaced by `KeyedHashBuilder`; the docs
  now say that a `RandomState` structure must not be persisted (deserializing
  draws a new key and the restored structure reports almost every item
  absent); `DefaultHasher` is no longer described as resistant to anything; the
  Cuckoo `capacity()` docs say it counts slots and can be up to twice the
  requested capacity, and the false-positive rate is stated as an upper bound
  at full load.

### Fixed

- **Hashing depended on the target's byte order and pointer width.** The
  hashers used the standard library's default integer writes, which feed
  native-endian bytes and a native-width `usize`. The `[u8]` and `str` length
  prefix was 4 bytes on 32-bit targets, and integer keys were byte-swapped on
  big-endian targets, so a filter serialized on x86_64 reported stored keys as
  absent on i686, armv7, wasm32 or s390x. Both hashers now feed integers in
  little-endian order and widen `usize`/`isize` to 64 bits on every target.
- **Constructors aborted or panicked on oversized parameters** although they
  return `Result`. Sizes are now computed with checked arithmetic and
  allocated with `try_reserve_exact`, and failures return
  `Error::InvalidParameter`: `BloomFilter::with_dimensions(u64::MAX, 1)`
  aborted the process, `CountMinSketch::with_dimensions(usize::MAX / 2 + 2, 2)`
  overflowed, `CuckooFilter::new(usize::MAX / 4)` panicked, and on 32-bit
  targets a large bit count was truncated.

### Security

- **`Deserialize` accepted any field values.** Hostile bytes could make a
  structure panic on its first query, loop for as long as the input said
  (Count-Min with width 0 and a depth of 2^64, or a Bloom hash count of
  2^32), or answer every query wrongly (a Bloom hash count of 0 made every
  item present). Every structure now has a validating `Deserialize` that
  checks each field against the others and against the size of the data, and
  returns an error. The serialized format itself is unchanged. The checks are
  listed in `docs/API.md#untrusted-state`.
- **Collisions in the default hasher can be computed directly**, and the only
  alternative the docs offered (`RandomState`) broke on a serde round trip.
  `KeyedHashBuilder` closes both gaps.

### Third-party dependencies

Unchanged: `libm` (always) and `serde` (optional, `serde` feature).

---

## [1.0.0] - 2026-05-29

First stable release. The public API is frozen and follows SemVer from here; see
[`docs/API.md#compatibility`](docs/API.md#compatibility). No functional changes
since v0.9.0.

### Changed

- Documented the API freeze and SemVer promise in `README.md` (Stability) and
  `docs/API.md` (Compatibility).

---

## [0.9.0] - 2026-05-28

Feature freeze and pre-1.0 audit. No new functionality; hardening and
verification only. The full audit report is in `docs/release/v0.9.0.md`.

### Added

- Unit tests for every `Error` variant's `Display` output and the
  `std::error::Error` implementation.
- `deny.toml` supply-chain policy for `cargo deny check` (advisories, bans,
  licenses, sources).

### Changed

- Marked `BloomFilter::contains` and `CuckooFilter::contains` `#[must_use]`;
  ignoring a membership check is almost always a bug.
- Declared `required-features` on every example so `--no-default-features`
  builds skip them cleanly.

---

## [0.5.0] - 2026-05-28

### Added

- `CuckooFilter<T, S>` — approximate set membership with deletion support
  (`insert`, `contains`, `remove`, `len`, `capacity`, `load_factor`, `clear`),
  using 16-bit fingerprints, 4-slot buckets, and a victim cache for correctness
  at the load limit.
- `CountMinSketch<T, S>` — saturating frequency estimation (`add`, `increment`,
  `estimate`, `total_count`, `width`, `depth`, `merge`, `clear`).
- `HyperLogLog<T, S>` — distinct-count estimation with bias correction and
  linear counting (`insert`, `count`, `merge`, `clear`, `precision`,
  `with_error_rate`).
- `MinHash<T, S>` — Jaccard similarity estimation (`insert`, `similarity`,
  `merge`, `num_hashes`, `clear`).
- `TopK<T, S>` — heavy-hitter tracking over a Count-Min Sketch (`insert`,
  `estimate`, `top`, `k`, `len`, `clear`).
- Property-test suite (`tests/proptests.rs`) covering the no-false-negative,
  never-undercount, cardinality-envelope, and similarity-bound invariants.
- Criterion benchmark harness (`benches/probabilistic.rs`) for every hot path.
- Five new examples: `membership_with_deletion`, `frequency`, `cardinality`,
  `similarity`, `top_words`.
- Internal `hash::mix64` avalanche helper for low-entropy inputs.

### Changed

- `docs/API.md` and `README.md` document the full structure set with parameters,
  examples, and measured benchmark figures.

---

## [0.2.0] - 2026-05-28

### Added

- `BloomFilter<T, S>` — a classic Bloom filter with a tunable false-positive
  rate, generic over the item type and `BuildHasher`. Supports `new`,
  `with_dimensions`, `with_hasher`, `with_dimensions_and_hasher`, `insert`
  (with a novelty flag), `contains`, `clear`, `merge`, `count_ones`, `is_empty`,
  `num_bits`, `num_hashes`, `estimated_len`, and `estimated_false_positive_rate`.
- `Error` — the crate-wide, `#[non_exhaustive]`, allocation-free error type with
  `InvalidParameter`, `IncompatibleParameters`, and `CapacityExceeded` variants;
  implements `Display` and (under `std`) `std::error::Error`.
- `hash` module — `DefaultHasher`, a fast deterministic 64-bit hasher, and
  `DefaultHashBuilder`, the zero-sized default `BuildHasher`.
- `prelude` module re-exporting the common types.
- `alloc` and `serde` Cargo features. `serde` derives `Serialize`/`Deserialize`
  for every structure; `alloc` enables the structures on heap-capable `no_std`
  targets.
- `docs/API.md` reference covering the complete public surface, and a
  `bloom_dedup` example.

### Changed

- Replaced the placeholder `REPS.md` with the canonical standards (REPS v0.2.0).
- `std` now implies `alloc`.

### Fixed

- Added `.gitattributes` to normalize line endings to LF, preventing the
  Windows CI runner from failing `cargo fmt --check` under `autocrlf`.

---

## [0.1.0] - 2026-05-18

### Added

- Initial scaffold and repository bootstrap.
- REPS compliance baseline.
- CI for Linux/macOS/Windows on stable and MSRV (1.75).

[Unreleased]: https://github.com/jamesgober/bloom-lib/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/jamesgober/bloom-lib/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/jamesgober/bloom-lib/releases/tag/v1.0.0
