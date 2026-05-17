# rustyroute

[![License: EUPL-1.2](https://img.shields.io/badge/License-EUPL--1.2-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.93.0-orange.svg)](Cargo.toml)

A Rust library for shortest-path maritime route computation.

> **Status: pre-API skeleton.** This repository contains the EUPL-1.2
> legal/governance scaffolding, the vendored Eurostat MARNET data, and
> a `build.rs` that compiles the MARNET GeoPackages into rkyv graph
> archives at build time. Public routing APIs (`Graph::load`,
> Dijkstra, distance matrices) will be introduced in follow-up tickets
> — see [`CONTRIBUTING.md`](CONTRIBUTING.md) for the scope policy.

## Installation

Not yet published to crates.io. Once published:

```sh
cargo add rustyroute
```

## Usage

Public routing APIs will be documented here when they are introduced.

## Attribution

rustyroute is based on and inspired by Eurostat's
[SeaRoute](https://github.com/eurostat/searoute) project. SeaRoute is
published under EUPL-1.2 by the European Union (Eurostat). See
[`NOTICE`](NOTICE) for full attribution.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). All commits must be signed off
under the [Developer Certificate of Origin](https://developercertificate.org/).

## Security

See [`SECURITY.md`](SECURITY.md). Do **not** report vulnerabilities via
public issues.

## License

Licensed under the [European Union Public Licence v. 1.2](LICENSE)
(`EUPL-1.2`).
