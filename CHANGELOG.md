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

## [1.0.0] - 2026-05-28

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

[Unreleased]: https://github.com/jamesgober/bloom-lib/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/jamesgober/bloom-lib/compare/v0.9.0...v1.0.0
[0.9.0]: https://github.com/jamesgober/bloom-lib/compare/v0.5.0...v0.9.0
[0.5.0]: https://github.com/jamesgober/bloom-lib/compare/v0.2.0...v0.5.0
[0.2.0]: https://github.com/jamesgober/bloom-lib/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/bloom-lib/releases/tag/v0.1.0
