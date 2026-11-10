# rustyroute

[![License: EUPL-1.2](https://img.shields.io/badge/License-EUPL--1.2-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.93.0-orange.svg)](Cargo.toml)

rustyroute computes the shortest maritime route between two points on
Earth — the path a ship actually sails around coastlines and through
named straits and canals, not the great-circle line a naive distance
calculation would draw straight through land. A route can be forced
around a named chokepoint (e.g. the Suez Canal) by blocking its edge
group, so "what if this canal were closed" is one function call.

It ships as a Rust library, pre-baked at five resolutions (5–100 km),
with an optional command-line tool for one-off queries from the shell.

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

A release has two surfaces: the crates.io publish (owned by release-plz)
and the cargo-dist binaries and installers (owned by `release.yaml`,
triggered by the `vX.Y.Z` tag). A crates.io yank touches only the first.

**State as of this writing.** `rustyroute` has never been published to
crates.io, so this procedure has not been rehearsed: there is no
published version to yank and un-yank. Rehearse it against the first
published version and record the result here.

### crates.io (yank)

Yank the bad version:

```sh
cargo yank rustyroute --version X.Y.Z
```

This stops new dependency resolutions selecting that version. It does
**not** remove the version from the registry and does **not** break
existing `Cargo.lock` files that already pin it: anyone already
depending on it keeps building.

To restore it:

```sh
cargo yank rustyroute --version X.Y.Z --undo
```

To confirm a yank took effect, resolve the crate fresh in a scratch
project (no lockfile to reuse) and check it selects the previous good
version, not the yanked one:

```sh
scratch=$(mktemp -d) && cargo init --lib --name rustyroute_yank_check "$scratch" && cd "$scratch"
cargo add rustyroute
cargo tree -p rustyroute
```

If the yanked version is the only version ever published, the expected
result is a failed resolution, not one against an older version.

### What a yank leaves behind

A yank reverts nothing else. Each remaining piece has its own undo:

| Left behind | What undoes it |
|---|---|
| The `vX.Y.Z` git tag | Leave it. Tags are history; deleting one breaks anyone who pinned it. The fix ships as a new tag. |
| The release-plz version-bump and CHANGELOG commit on `main` | Do not revert it. The next release (the patch) bumps past it; add a CHANGELOG line noting the yank. |
| The GitHub release for the tag, and any binaries attached to it | Delete the release's assets, or delete the release, so the installers stop serving them (see below). |
| Binaries already installed on users' machines | Nothing on our side removes them. Ship a patch release and tell users to reinstall. |

### Binaries and installers (cargo-dist)

**Does a tag trigger `release.yaml`?** It fires on a pushed `v*.*.*` tag,
but release-plz pushes that tag with `RELEASE_PLZ_TOKEN`, falling back to
`GITHUB_TOKEN`. A tag pushed with `GITHUB_TOKEN` does not trigger other
workflows. So binaries exist for a release only if the `RELEASE_PLZ_TOKEN`
repo secret is set. As of this writing it is not set (neither as a repo
nor an org secret), so a release tag does not trigger `release.yaml` and
no cargo-dist binaries exist. Check the release's page: if it has no attached
assets, there is nothing on this surface to roll back. In addition,
`release.yaml` and `dist-workspace.toml` have not yet been regenerated
with the real dist CLI (see the comments in both files), so treat the
binary surface as not yet live.

`dist-workspace.toml` enables three installers:

- **shell** (`curl ... | sh`) and **powershell** (`irm ... | iex`). Both
  download the binaries attached to the GitHub release. Deleting the bad
  release's assets, or the release itself, is what stops them installing
  the bad version; mark the previous good release as "latest" so they
  fall back to it.
- **homebrew**. **Not set up yet.** The tap repo `spotship/homebrew-tap`
  and its `HOMEBREW_TAP_TOKEN` secret do not exist, as
  `dist-workspace.toml` records, so no formula has ever been published
  and there is nothing to roll back. Once the tap exists, remove or
  revert the bad formula in it as well.

### Yank vs. patch release

Yank when the version must stop being resolved by new builds: it is
broken, insecure, or otherwise unfit to depend on. Publish a patch
release instead when the version is still safe and the fix is routine.
When a release is unfit **and** a fix is ready, do both: yank, then
publish the patch.

## Attribution

rustyroute is based on and inspired by Eurostat's
[SeaRoute](https://github.com/eurostat/searoute) project. SeaRoute is
published under EUPL-1.2 by the European Union (Eurostat). See
[`NOTICE`](NOTICE) for full attribution.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). All commits must be signed off
under the [Developer Certificate of Origin](https://developercertificate.org/).

`Taskfile.yml` defines one task, `task init`, which runs `cargo fetch`
and then `pre-commit install`. There is no `task lint` or `task test`
here, unlike other Spot Ship repositories: `cargo fmt` and `cargo clippy`
replace `task lint`, and `cargo test` replaces `task test`. The exact
invocations CI enforces are listed once, in the "Local validation"
section of [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Security

See [`SECURITY.md`](SECURITY.md). Do **not** report vulnerabilities via
public issues.

## License

Licensed under the [European Union Public Licence v. 1.2](LICENSE)
(`EUPL-1.2`).
