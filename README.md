# rustyroute

[![License: EUPL-1.2](https://img.shields.io/badge/License-EUPL--1.2-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.93.0-orange.svg)](Cargo.toml)

A Rust library for shortest-path maritime route computation.

> **Status: pre-release.** This repository contains the EUPL-1.2
> legal/governance scaffolding, the vendored Eurostat MARNET data, and
> a `build.rs` that compiles the MARNET GeoPackages into rkyv graph
> archives at build time. `Graph::load`/`Graph::from_bytes` and
> Dijkstra routing (`Graph::route`) have landed; distance matrices and
> further algorithms follow in later tickets — see
> [`CONTRIBUTING.md`](CONTRIBUTING.md) for the scope policy.

## Installation

Not yet published to crates.io. Once published:

```sh
cargo add rustyroute
```

## Usage

Load a graph and compute a route. Coordinates are `(lat, lng)` in
decimal degrees:

```rust
use rustyroute::Graph;
use std::collections::HashSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let graph = Graph::load(50)?; // 50 km resolution

    let route = graph.route(
        (43.30, 5.37),   // Marseille
        (31.23, 121.47), // Shanghai
        &HashSet::new(), // no blocked edges
    )?;
    println!("{:.1} km", route.distance_km);

    // Avoid a chokepoint: resolve one or more of the 13 baked-in edge
    // groups (see `rustyroute::EDGE_GROUPS`) and pass them to `route`.
    let blocked = graph.edges_for_groups(["suezCanal"])?;
    let around_africa = graph.route((34.0, 28.0), (20.0, 38.0), &blocked)?;
    println!("{:.1} km avoiding Suez", around_africa.distance_km);

    Ok(())
}
```

The `data-{N}km` features control which resolutions are baked into the
binary; `data-50km` is the default. `Graph::load` also honours
`$RUSTYROUTE_DATA_DIR` for loading archives from disk.

## CLI

An optional `rustyroute` binary is available behind the `cli` feature.
It is not built by default, so library-only consumers never compile
`clap`:

```sh
cargo install --path . --features cli
```

```sh
rustyroute route --from 43.30,5.37 --to 31.23,121.47
```

| Flag | Values | Default |
|------|--------|---------|
| `--from` / `--to` | `lat,lng` decimal degrees | required |
| `--resolution` | `5`, `10`, `20`, `50`, `100` (km) | `50` |
| `--block` | comma-separated edge groups, e.g. `suezCanal,menaiStrait` | none |
| `--format` | `json`, `geojson`, `line` | `geojson` |

The `cli` feature also enables every `data-{N}km` feature so an
installed binary can serve any `--resolution`.

Results go to stdout, diagnostics to stderr. Exit codes:

| Code | Meaning |
|------|---------|
| 0 | route found |
| 1 | bad arguments |
| 2 | no route between the endpoints |
| 3 | graph data unavailable for the requested resolution |
| 4 | failed to write output |

## Rollback

A bad release has two independent surfaces to roll back: the crates.io
publish (owned by release-plz) and the GitHub-release binaries + homebrew
formula (owned by cargo-dist, triggered separately by the release tag).
A crates.io yank does **not** touch the second surface at all.

### crates.io (yank)

Yank the bad version:

```sh
cargo yank --version X.Y.Z rustyroute
```

This blocks new dependency resolutions from selecting that version. It does
**not** remove the version from the registry and does **not** break
existing `Cargo.lock` files that already pin it — anyone already depending
on the yanked version keeps building.

To restore it:

```sh
cargo yank --undo --version X.Y.Z rustyroute
```

To confirm a yank took effect, resolve the crate fresh in a scratch project
(no existing lockfile to reuse) and check it selects the previous good
version, not the yanked one:

```sh
scratch=$(mktemp -d) && cargo init --lib --name rustyroute_yank_check "$scratch" && cd "$scratch"
cargo add rustyroute
cargo tree -p rustyroute
```

If the yanked version is the *only* version ever published, there is no
earlier version to fall back to — the expected result in that case is a
failed resolution, not a successful one against an older version.

### Binaries and the homebrew formula (cargo-dist)

A `cargo yank` has **no effect** on the GitHub release's attached binaries
or the homebrew formula — cargo-dist publishes those once, tied to the
release tag, independently of crates.io. To pull a bad binary release:

- Mark the affected GitHub release as a pre-release/draft (or delete it)
  so its assets stop being served as the latest release.
- Update or remove the affected formula in `spotship/homebrew-tap` so
  `brew install` stops serving the bad version.

### Yank vs. patch release

Yank when the published version must stop being resolved by new builds —
it's broken, insecure, or otherwise unfit to depend on. Publish a patch
release instead when the current version is still safe to use and the fix
is routine — a patch gives existing and new users a better version without
blocking anyone. When a release is both urgent and needs a fix, do both:
yank the bad version, then publish the patch.

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
