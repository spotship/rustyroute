# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Continuous performance signal (ENG-4690): a `criterion` benchmark suite
  (`benches/route.rs`) covering the routing hot path — `Graph::load`,
  `Graph::route` (open + Suez-blocked), the heavy 5km route, and the
  13-group registry lookup — plus a `codspeed.yaml` workflow that reports
  per-PR performance deltas vs `main` via `CodSpeedHQ/action@v2`. Benches
  are authored against `codspeed-criterion-compat` (a drop-in criterion
  replacement: plain criterion under `cargo bench`, instrumented under
  `cargo codspeed`). `[skip-perf]` in a PR title bypasses the perf job.
  Uploads authenticate to CodSpeed over OpenID Connect (`id-token: write`)
  via the org's CodSpeed GitHub App — no `CODSPEED_TOKEN` secret required.
- Automated release pipeline (ENG-4692): `release-plz` opens a
  `chore: release vX.Y.Z` PR from conventional commits and, on merge,
  publishes to crates.io and creates the GitHub release/tag;
  `cargo-dist` config + `release.yaml` provide cross-platform binary
  distribution (active as of ENG-4682, which lands the `rustyroute` CLI
  binary — see `dist-workspace.toml`). Seeds this `CHANGELOG.md`, which
  `release-plz` maintains going forward.

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
