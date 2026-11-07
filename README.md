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

## HTTP quickstart

rustyroute is a library: it ships no HTTP framework and takes no
opinion on how you serve it. Wrapping it is small enough that you do
not need anyone else's service to do it — the whole thing is one
handler.

```toml
[dependencies]
rustyroute = "0.1"
axum = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use rustyroute::{Graph, RouteError};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Params {
    from: String,
    to: String,
    #[serde(default)]
    block: String,
}

fn parse_latlng(s: &str) -> Option<(f64, f64)> {
    let (lat, lng) = s.split_once(',')?;
    Some((lat.trim().parse().ok()?, lng.trim().parse().ok()?))
}

async fn route(
    State(graph): State<Arc<Graph>>,
    Query(p): Query<Params>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let bad = |m: String| (StatusCode::BAD_REQUEST, m);
    let from = parse_latlng(&p.from).ok_or_else(|| bad("from must be \"lat,lng\"".into()))?;
    let to = parse_latlng(&p.to).ok_or_else(|| bad("to must be \"lat,lng\"".into()))?;

    let names = p.block.split(',').map(str::trim).filter(|s| !s.is_empty());
    let blocked = graph.edges_for_groups(names).map_err(|e| bad(e.to_string()))?;

    match graph.route(from, to, &blocked) {
        Ok(r) => Ok(Json(json!({
            "type": "Feature",
            "properties": { "distance_km": r.distance_km },
            "geometry": {
                "type": "LineString",
                // GeoJSON is [lng, lat]; rustyroute returns (lat, lng).
                "coordinates": r.coordinates.iter().map(|(lat, lng)| [*lng, *lat]).collect::<Vec<_>>(),
            },
        }))),
        Err(RouteError::NoRoute) => Err((StatusCode::UNPROCESSABLE_ENTITY, "no route".into())),
        Err(e) => Err(bad(e.to_string())),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let graph = Arc::new(Graph::load(50)?);
    let app = Router::new().route("/route", get(route)).with_state(graph);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

`Graph::load` runs once at startup and is shared through an `Arc`. The
graph is read-only and `route` neither mutates nor copies it, so every
request routes against the same mmap. Blocking is resolved per
request: all 13 groups together cost microseconds, so there is nothing
to cache.

```sh
curl 'localhost:3000/route?from=43.30,5.37&to=31.23,121.47'
# {"type":"Feature","properties":{"distance_km":16354.1}, ...}

curl 'localhost:3000/route?from=43.30,5.37&to=31.23,121.47&block=suezCanal'
# 25047.5 km — the same voyage routed around Africa
```

Unparseable coordinates and unknown group names return `400`; a
destination made unreachable by blocking returns `422`.

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
