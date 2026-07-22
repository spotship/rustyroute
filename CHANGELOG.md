# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
