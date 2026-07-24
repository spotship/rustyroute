# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Continuous fuzzing (ENG-4691): a self-contained `fuzz/` cargo-fuzz
  package (its own workspace root, excluded from the root build) with two
  libFuzzer targets — `load_archive` (arbitrary bytes → `Graph::from_bytes`)
  and `route_inputs` (fuzzed `from`/`to`/blocked composition against a fixed
  graph) — plus a committed valid seed. A `fuzz` CI workflow runs a 60s
  `load_archive` quick-pass per PR on nightly, and `oss-fuzz/` stages the
  Google OSS-Fuzz project files (Dockerfile, build.sh, project.yaml) with a
  submission runbook for CNCF-grade continuous fuzzing.
- Automated release pipeline (ENG-4692): `release-plz` opens a
  `chore: release vX.Y.Z` PR from conventional commits and, on merge,
  publishes to crates.io and creates the GitHub release/tag;
  `cargo-dist` config + `release.yaml` provide cross-platform binary
  distribution (dormant until a CLI binary lands — see
  `dist-workspace.toml`). Seeds this `CHANGELOG.md`, which `release-plz`
  maintains going forward.

## [0.1.0] - 2026-07-22

### Added

- Initial pre-API skeleton: EUPL-1.2 licensing and governance
  scaffolding; vendored Eurostat MARNET GeoPackage data at five
  resolutions (5/10/20/50/100 km); `build.rs` CSR graph compilation with
  a 13-group edge registry; zero-copy `Graph::from_bytes` and mmap-based
  `Graph::load` library APIs; CI, supply-chain audit, and pre-commit
  gates.

[Unreleased]: https://github.com/spotship/rustyroute/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/spotship/rustyroute/releases/tag/v0.1.0
