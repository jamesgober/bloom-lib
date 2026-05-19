# bloom-lib - Roadmap to 1.0

Fast-track. No slow-stepping.

---

## Phase 0.1.0 - Scaffold (done)

- [x] Repository created
- [x] Cargo.toml, README, LICENSE x2, CHANGELOG, gitignore, editorconfig, rustfmt, clippy
- [x] REPS.md
- [x] CI workflow (Linux/macOS/Windows on stable + MSRV)
- [x] Initial commit pushed

---

## Phase 0.2.0 - Foundation

Define the public API surface. This is the contract that 1.0 will preserve.

Skill areas in scope:

  - probabilistic data structures
  - hash function selection
  - streaming-safe state management

- [ ] Public types defined
- [ ] Public traits defined (where applicable)
- [ ] Module structure laid out
- [ ] Error type defined
- [ ] First end-to-end smoke test passing
- [ ] CHANGELOG updated
- [ ] `.dev/release/v0.2.0.md` written

---

## Phase 0.5.0 - Implementation

Most of the feature set landed.

- [ ] All public API methods implemented (no `todo!()`)
- [ ] Property tests for state machines / invariants
- [ ] Integration tests covering primary use cases
- [ ] Basic benchmarks in place
- [ ] Documentation drafted for every public item
- [ ] No `unwrap` / `expect` outside of tests
- [ ] CHANGELOG updated
- [ ] `.dev/release/v0.5.0.md` written

---

## Phase 0.9.0 - Hardening + Audit

Feature freeze. Quality focus.

### Audit checklist (mandatory)

#### Feature completeness
- [ ] Every roadmap item delivered
- [ ] Every README claim verified against code

#### Code cleanliness
- [ ] No dead code
- [ ] No commented-out code
- [ ] No TODO/FIXME without tracking issue
- [ ] No `#[allow(...)]` without justification

#### Error hardening
- [ ] Every public function: all error paths documented
- [ ] Every error variant: documented + tested
- [ ] No panics in shipping code paths
- [ ] Error messages actionable

#### API stability
- [ ] Every public item reviewed for 1.0 stability
- [ ] Sealed traits where appropriate
- [ ] `#[non_exhaustive]` on enums likely to grow

#### Documentation
- [ ] Every public item: rustdoc with at least one example
- [ ] README accurate
- [ ] CHANGELOG complete
- [ ] `cargo doc --no-deps --all-features` zero warnings
- [ ] Examples in `examples/` (where applicable)

#### Tests
- [ ] Unit test coverage on all public functions
- [ ] Integration tests covering documented use cases
- [ ] Property tests for invariant-bearing types
- [ ] Cross-platform CI green
- [ ] Both stable and MSRV (1.75) green

#### Performance
- [ ] Hot paths identified and benchmarked
- [ ] Allocation profile checked
- [ ] No regressions vs prior phase
- [ ] Benchmark baselines saved

#### Final gates
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` clean
- [ ] `cargo test --all-features` clean
- [ ] `cargo doc` clean with `RUSTDOCFLAGS=-D warnings`

### Output
- [ ] `.dev/release/v0.9.0.md` written
- [ ] Audit findings logged
- [ ] All audit findings resolved or explicitly deferred to 1.x

---

## Phase 0.9.x - Audit fixes

Iterate as findings are resolved.

- [ ] All 0.9.0 audit blockers resolved
- [ ] No new features (feature-frozen)
- [ ] Final benchmarks recorded
- [ ] Final API freeze

---

## Phase 1.0.0 - Stable release

- [ ] All 0.9.x audit findings resolved
- [ ] Final API freeze
- [ ] Final benchmark numbers captured
- [ ] `.dev/release/v1.0.0.md` written
- [ ] Tag `v1.0.0` on main
- [ ] Publish to crates.io