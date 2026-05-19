<h1 align="center">
    <strong>bloom-lib</strong>
    <br>
    <sup><sub>PROBABILISTIC DATA STRUCTURES FOR RUST</sub></sup>
</h1>

<p align="center">
    <a href="https://crates.io/crates/bloom-lib"><img alt="crates.io" src="https://img.shields.io/crates/v/bloom-lib.svg"></a>
    <a href="https://docs.rs/bloom-lib"><img alt="docs.rs" src="https://docs.rs/bloom-lib/badge.svg"></a>
    <a href="https://github.com/jamesgober/bloom-lib/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/jamesgober/bloom-lib/actions/workflows/ci.yml/badge.svg"></a>
    <a href="#license"><img alt="license" src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg"></a>
</p>

<p align="center">Bloom filters, Cuckoo filters, Count-Min Sketch, HyperLogLog, MinHash, Top-K. Tunable false-positive rates, serializable state.</p>

---

## Status

**Active development.** Scaffolded and on the path to 1.0. See [.dev/ROADMAP.md](.dev/ROADMAP.md) for milestone tracking.

The public API is not yet stable. Pin specific versions; expect changes pre-1.0.

---

## What it does

Probabilistic data structure library: Bloom filters, Cuckoo filters, Count-Min Sketch, HyperLogLog, MinHash, and Top-K. Tunable false-positive rates, serializable state, merge support, and streaming-safe updates.

---

## Quick start

```toml
[dependencies]
bloom-lib = "0.1"
```

---

## Standards

- **REPS** governs every decision. See [REPS.md](REPS.md).
- **MSRV:** Rust 1.75.
- **Edition:** 2024.
- **Cross-platform:** Linux, macOS, Windows.

---

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

<sub>Copyright &copy; 2026 <strong>James Gober</strong>. All rights reserved.</sub>