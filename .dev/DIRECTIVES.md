# bloom-lib - Directives

## Priority

1. `REPS.md` - SUPREME AUTHORITY
2. This file
3. `.dev/PROMPT.md`
4. `.dev/ROADMAP.md`

REPS overrides everything else.

## Cross-platform discipline

Runs on Linux, macOS, Windows. Every public function works on all three or is feature-gated with clear docs.

## REPS compliance

- Zero-allocation hot path where feasible
- Lock-free where contention matters
- `unsafe` only when measured and documented
- No `unwrap()` / `expect()` / `todo!()` / `unimplemented!()` in shipping code
- No `print_stdout` / `print_stderr` / `dbg!()` in shipping code
- All public items documented with at least one code example

## Versioning

Fast-track to 1.0. Every release tagged. Every release has `.dev/release/v<version>.md`.

## Documentation

- Every public item: rustdoc with one example minimum
- `README.md`: current with public API
- `CHANGELOG.md`: every change under `[Unreleased]` before commit
- Release notes: `.dev/release/v<version>.md` before tagging

## Pre-1.0 audit

Mandatory. Checklist in `.dev/ROADMAP.md`. Produces written report in `.dev/release/v1.0.0.md`.

## Out of scope

- Specific async runtime hard-dependency
- Heavy framework dependency
- REPS violations