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

/// Which pre-baked grid to serve. Used for both the load and the
/// reported `resolution`, so the two cannot disagree. (Don't reach for
/// `Graph::resolution_km()` here: it returns 0 for a handle built with
/// `Graph::from_bytes`, which is exactly the substitution suggested
/// below.)
const RESOLUTION_KM: u32 = 50;

/// Load the graph once and leak it for the process lifetime — the
/// long-lived-handle pattern documented on `rustyroute::Graph`. `Graph`
/// is `Send + Sync` but not `Clone`, so a handler shared across tokio
/// worker threads needs `&'static Graph` (or an `Arc`).
///
/// `Graph::load` needs no setup on default features: it falls back
/// to the `data-50km` slice baked into the binary. Without a filesystem
/// (wasm, scratch containers) use that slice directly instead —
/// `Graph::from_bytes(rustyroute::data::BYTES_50KM)` — which requires
/// the `data-50km` feature to be enabled.
fn graph() -> &'static Graph {
    static G: OnceLock<&'static Graph> = OnceLock::new();
    G.get_or_init(|| {
        let g = Graph::load(RESOLUTION_KM).expect("load the graph");
        Box::leak(Box::new(g))
    })
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
                        "resolution": RESOLUTION_KM,
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
    // `PORT=0` asks the OS for a free port — that is how
    // tests/axum_example_e2e.rs boots this example without colliding
    // with anything already on 3000. The line below prints whichever
    // port was actually bound.
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
