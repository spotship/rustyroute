# rustyroute

[![crates.io](https://img.shields.io/crates/v/rustyroute.svg)](https://crates.io/crates/rustyroute)
[![docs.rs](https://img.shields.io/docsrs/rustyroute)](https://docs.rs/rustyroute)
[![CI](https://github.com/spotship/rustyroute/actions/workflows/ci.yaml/badge.svg)](https://github.com/spotship/rustyroute/actions/workflows/ci.yaml)
[![License: EUPL-1.2](https://img.shields.io/badge/License-EUPL--1.2-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.93.0-orange.svg)](Cargo.toml)

Maritime sea-routing primitives on Eurostat MARNET data, in safe Rust.
5 resolutions, 13 named chokepoint groups, zero-copy mmap load. Give it
two `(lat, lng)` points and it returns the shortest sea path between
them, optionally routing around named chokepoints like the Suez Canal or
the Strait of Malacca. The graph data ships inside the crate, so there
is nothing to download and no service to call.

> **Status: pre-1.0.** The API can break between minor versions, and
> `rustyroute` is not yet published to crates.io — the crates.io and
> docs.rs badges above stay grey until the first release. This crate is
> the routing core behind Spot Ship's `marine-router` production
> service. Distance matrices and further algorithms follow in later
> releases; see [`CONTRIBUTING.md`](CONTRIBUTING.md) for the scope
> policy.

## Quickstart (library)

```sh
cargo add rustyroute
```

(Not on crates.io yet — until the first release, depend on it by git or
path.)

That is the whole setup on your side. The default features bake the
50 km graph into your binary, so the snippet below runs as-is — no
environment variable, no data directory, nothing to configure. Note that
rustyroute's *own* first build is slow: its `build.rs` compiles a
bundled SQLite and parses ~17 MiB of GeoPackages into the five graph
archives. That cost is paid once, at build time; every run afterwards
just memory-maps the result.

```rust
use rustyroute::Graph;
use std::collections::HashSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Works on default features: the 50 km graph is baked into the crate.
    let graph = Graph::load(50)?;

    // Coordinates are (lat, lng) — Marseille to Shanghai.
    let route = graph.route((43.30, 5.37), (31.23, 121.47), &HashSet::new())?;

    println!("{:.1} km over {} points", route.distance_km, route.coordinates.len());
    println!("first: {:?}", route.coordinates[0]); // (lat, lng)
    Ok(())
}
```

This prints `16354.1 km over 106 points`.

## Quickstart (HTTP server with axum)

A complete routing service in about fifty lines. Add these dependencies:

```toml
[dependencies]
rustyroute = "0.1"
axum = "0.8"
tokio = { version = "1", features = ["macros", "net", "rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust,no_run
//! ENG-4683: minimal axum HTTP server over rustyroute.
//!
//! Run it:
//!
//!     cargo run --example axum_server
//!     curl "localhost:3000/route?fromLatLng=43.30,5.37&toLatLng=31.23,121.47"
//!
//! This file is the single source of truth for the README's HTTP
//! quickstart — `tests/readme_contract.rs` asserts the README fence and
//! this file are byte-identical, so edit here and re-sync the README.

use std::collections::HashSet;
use std::sync::OnceLock;

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rustyroute::{EdgeId, Graph, RouteError};
use serde_json::json;

/// Load the graph once and leak it for the process lifetime — the
/// long-lived-handle pattern documented on `rustyroute::Graph`. `Graph`
/// is `Send + Sync` but not `Clone`, so a handler shared across tokio
/// worker threads needs `&'static Graph` (or an `Arc`).
///
/// `Graph::load(50)` needs no setup on default features: it falls back
/// to the `data-50km` slice baked into the binary. Without a filesystem
/// (wasm, scratch containers) use that slice directly instead —
/// `Graph::from_bytes(rustyroute::data::BYTES_50KM)` — which requires
/// the `data-50km` feature to be enabled.
fn graph() -> &'static Graph {
    static G: OnceLock<&'static Graph> = OnceLock::new();
    G.get_or_init(|| Box::leak(Box::new(Graph::load(50).expect("load 50km graph"))))
}

/// `?fromLatLng=43.30,5.37&toLatLng=31.23,121.47&block=suezCanal`
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RouteQuery {
    from_lat_lng: String,
    to_lat_lng: String,
    /// Comma-separated edge-group names; see `rustyroute::EDGE_GROUPS`.
    block: Option<String>,
}

/// Parse a `"lat,lng"` pair. The library's coordinate order is
/// (lat, lng) — the same order this endpoint accepts.
fn parse_lat_lng(s: &str) -> Option<(f64, f64)> {
    let (lat, lng) = s.split_once(',')?;
    Some((lat.trim().parse().ok()?, lng.trim().parse().ok()?))
}

async fn route(Query(q): Query<RouteQuery>) -> Response {
    let Some(from) = parse_lat_lng(&q.from_lat_lng) else {
        return bad_request("fromLatLng must be `lat,lng`");
    };
    let Some(to) = parse_lat_lng(&q.to_lat_lng) else {
        return bad_request("toLatLng must be `lat,lng`");
    };

    let graph = graph();
    let blocked: HashSet<EdgeId> = match q.block.as_deref().filter(|s| !s.is_empty()) {
        None => HashSet::new(),
        Some(names) => match graph.edges_for_groups(names.split(',').map(str::trim)) {
            Ok(ids) => ids,
            Err(e) => return bad_request(&e.to_string()),
        },
    };

    match graph.route(from, to, &blocked) {
        Ok(r) => {
            // THE coordinate swap. The library speaks (lat, lng);
            // GeoJSON positions are [lng, lat]. Getting this backwards
            // is the single most common mistake — do it once, here, at
            // the response boundary.
            let mut coordinates: Vec<[f64; 2]> =
                r.coordinates.iter().map(|&(lat, lng)| [lng, lat]).collect();
            // A self-route returns one coordinate, but RFC 7946 §3.1.4
            // requires a LineString to have two or more positions.
            // Repeat the point for a valid degenerate line — the same
            // thing the `rustyroute` CLI does.
            if coordinates.len() == 1 {
                coordinates.push(coordinates[0]);
            }
            Json(json!({
                "type": "FeatureCollection",
                "features": [{
                    "type": "Feature",
                    "geometry": { "type": "LineString", "coordinates": coordinates },
                    "properties": {
                        "distance_km": r.distance_km,
                        "resolution": graph.resolution_km(),
                    },
                }],
            }))
            .into_response()
        }
        Err(RouteError::NoRoute) => (StatusCode::NOT_FOUND, "no route").into_response(),
        Err(e) => bad_request(&e.to_string()),
    }
}

fn bad_request(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response()
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/route", get(route));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
```

Run it and ask for a route:

```sh
cargo run --example axum_server
curl "localhost:3000/route?fromLatLng=43.30,5.37&toLatLng=31.23,121.47"
```

```json
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "geometry": {
        "type": "LineString",
        "coordinates": [[5.1927490234375, 43.230220794677734], "..."]
      },
      "properties": { "distance_km": 16354.073, "resolution": 50 }
    }
  ]
}
```

**Mind the coordinate order.** `rustyroute` speaks `(lat, lng)`, because
that is the order people write coordinates in. GeoJSON positions are
`[lng, lat]`. The example does that swap once, at the response boundary,
and it is the single thing new users most often get backwards — note
that the first position above begins `5.19…` (a longitude near
Marseille), not `43.23…`.

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

## How the data is built

Everything happens at compile time. `build.rs` reads the vendored
GeoPackages, builds a CSR adjacency, classifies the chokepoint groups,
and writes one rkyv archive per resolution. At run time the library
either mmaps that archive or reads the copy baked into your binary —
either way the graph is used in place, with no parsing step.

```text
  build time (build.rs)                          run time
  ─────────────────────                          ────────

  vendor/eurostat-marnet/
    marnet_plus_{5,10,20,50,100}km.gpkg
              │
              │  build/gpkg_io.rs   read GeoPackage LineStrings
              ▼
        RawEdge stream
              │
              │  build/csr.rs       dedupe nodes, build CSR adjacency,
              │                     haversine edge weights (km)
              ▼
          CsrBuilt
              │
              │  build/groups.rs    12 `pass`-tag groups + menaiStrait
              │                     (bbox); empty group => build error
              ▼
         GraphData  ──  build/archive.rs  ──▶  $OUT_DIR/data/{N}km.rkyv
              │                                  b"RRG1" + u32 version
              │                                  + rkyv payload
              │
              │  build/registry.rs
              ▼
    $OUT_DIR/edge_groups.rs  ──▶  pub const EDGE_GROUPS: &[&str; 13]
                                             (included by src/lib.rs)

                                   $OUT_DIR/data/{N}km.rkyv
                                             │
                        include_bytes! ──────┤ (src/data.rs, gated by
                        4-byte aligned       │  the data-{N}km feature)
                                             ▼
                                   rustyroute::data::BYTES_{N}KM
                                             │
     Graph::load(N) ── mmap from disk ───────┤── Graph::from_bytes(..)
       $RUSTYROUTE_DATA_DIR                  │      (any target, incl. wasm)
       → $OUT_DIR → static slice             ▼
                                          Graph
                                             │
                                             ▼
                              Graph::route((lat,lng), (lat,lng), &blocked)
                                             │
                                             ▼
                        Route { coordinates: Vec<(lat, lng)>,
                                distance_km, edge_ids }
```

## Data and features

The maritime network is Eurostat's SeaRoute / MARNET dataset, vendored
byte-for-byte under `vendor/eurostat-marnet/` from upstream commit
`88a2e568a8e0144d1f5a81c3931a7bc2bcce6901` and published by Eurostat
under EUPL-1.2 — the same licence as this crate. See [`NOTICE`](NOTICE)
and [`vendor/eurostat-marnet/README.md`](vendor/eurostat-marnet/README.md)
for the full provenance chain, checksums, and download date.

Each resolution is a separate feature, so you pay only for the grids you
use. Sizes are the rkyv archive baked into your binary:

| Feature | Resolution | Nodes | Edges | Baked size |
|---|---|---:|---:|---:|
| `data-5km` | 5 km | 36,121 | 72,478 | 2.90 MiB |
| `data-10km` | 10 km | 23,288 | 48,301 | 1.93 MiB |
| `data-20km` | 20 km | 14,046 | 29,581 | 1.18 MiB |
| **`data-50km`** (default) | 50 km | 7,390 | 15,498 | **632 KiB** |
| `data-100km` | 100 km | 4,688 | 9,847 | 402 KiB |

To swap the default resolution, turn the defaults off and pick another:

```toml
rustyroute = { version = "0.1", default-features = false, features = ["data-20km"] }
```

Two other ways to get graph data in:

- **From disk.** Set `$RUSTYROUTE_DATA_DIR` and `Graph::load(N)` mmaps
  `{N}km.rkyv` from there instead of using a baked copy — useful when you
  want one archive shared by several processes, or a binary that stays
  small.
- **From a byte slice.** `Graph::from_bytes(rustyroute::data::BYTES_50KM)`
  works on every target including `wasm32`, where there is no filesystem
  to mmap. This still needs a data feature: `default-features = false`
  on its own compiles no `BYTES_*KM` constant at all, so write
  `default-features = false, features = ["data-50km"]`.

## Edge groups

Thirteen named chokepoints and passages are baked into every archive.
Resolve any of them to a set of edge ids with
`graph.edges_for_groups(["suezCanal"])` and pass that set to `route()` to
find the path that avoids them. The names are also available at compile
time as `rustyroute::EDGE_GROUPS`.

| Group | Source |
|---|---|
| `suezCanal` | upstream `pass` tag `suez` |
| `panamaCanal` | `pass` tag `panama` |
| `malaccaStrait` | `pass` tag `malacca` |
| `gibraltarStrait` | `pass` tag `gibraltar` |
| `doverStrait` | `pass` tag `dover` |
| `beringStrait` | `pass` tag `bering` |
| `magellanStrait` | `pass` tag `magellan` |
| `babElMandebStrait` | `pass` tag `babelmandeb` |
| `kielCanal` | `pass` tag `kiel` |
| `corinthCanal` | `pass` tag `corinth` |
| `northwestPassage` | `pass` tag `northwest` |
| `northeastPassage` | `pass` tag `northeast` |
| `menaiStrait` | bbox `lng ∈ [-4.20, -4.00]`, `lat ∈ [53.13, 53.30]` |

Twelve of the groups come straight from the upstream `pass` attribute.
`menaiStrait` has no upstream tag, so it is derived geometrically: any
edge whose LineString intersects that closed bounding box joins the
group. A group that ends up empty at any resolution is a hard build
error, so all thirteen are guaranteed present in shipped data.

Blocking the Suez Canal on a Marseille → Shanghai route lengthens it
from 16,354 km to 25,047 km — the trip around the Cape of Good Hope.

## Performance

Two numbers are asserted by the test suite as budgets: a cold
`Graph::load(50)` under 50 ms and a warm one under 1 ms. That test is
`#[ignore]`d, because CI runners vary too much for a hard timing gate.

Measured on a Linux host with rustc 1.97, release profile:

| Operation | Time |
|---|---|
| `Graph::load(50)` — cold | 27 µs |
| `Graph::load(50)` — warm | 26 µs |
| `Graph::from_bytes(BYTES_50KM)` | 2 µs |
| `route()` — 50 km, Marseille → Shanghai (106 points) | 5.4 ms |
| `route()` — 5 km, same pair (204 points) | 29 ms |

The `from_bytes` and 50 km rows need `data-50km` (on by default); the
5 km row needs `data-5km`, which is not.

Loading is effectively free: the archive is mmapped or already resident
in the binary, and rkyv reads it in place with no deserialisation. The
cost that matters is `route()` itself, currently dominated by a
linear-scan nearest-node snap over the node table — a k-d tree is
tracked for a later release.

For size, the default features add about 632 KiB to your binary and all
five resolutions add about 7.0 MiB. That is the whole footprint — there
is no sidecar data file to ship — so a container image carrying this
crate grows by whichever resolutions you enable and nothing else. Treat
these numbers as observations on one machine, not guarantees.

## License and attribution

Licensed under the
[European Union Public Licence v. 1.2](LICENSE) (`EUPL-1.2`).

rustyroute is based on and inspired by Eurostat's
[SeaRoute](https://github.com/eurostat/searoute) project, published
under EUPL-1.2 by the European Union (Eurostat). The vendored MARNET
GeoPackages are redistributed unmodified under the same licence. See
[`NOTICE`](NOTICE) for full attribution.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). All commits must be signed off
under the [Developer Certificate of Origin](https://developercertificate.org/)
— `git commit -s` does this for you.

## Security

See [`SECURITY.md`](SECURITY.md). Do **not** report vulnerabilities via
public issues.
