//! CodSpeed benchmarks for the rustyroute public API.
//!
//! Covers the three runtime hot paths exposed by [`rustyroute::Graph`]:
//!
//! - [`Graph::from_bytes`]: rkyv checked-access validation of a graph
//!   archive (the cost paid once per process on load).
//! - [`Graph::route`]: the Dijkstra shortest-path search, measured both
//!   with an empty blocked set and with the Suez chokepoint blocked
//!   (forcing the long detour around Africa).
//! - [`Graph::edges_for_groups`]: resolving named chokepoint groups into
//!   an [`EdgeId`] set.
//!
//! All benchmarks use the feature-baked 50km static archive so they run
//! with the crate's `default` feature set and require no on-disk data.
#![cfg(feature = "data-50km")]

use divan::Bencher;
use rustyroute::{EdgeId, Graph};
use std::collections::HashSet;
use std::sync::OnceLock;

fn main() {
    divan::main();
}

// Load and validate the 50km archive once for the whole bench binary.
// Every routing method takes `&self`, so a shared handle keeps the
// per-iteration work focused on the routing algorithm rather than on
// repeated archive validation.
fn graph() -> &'static Graph {
    static G: OnceLock<Graph> = OnceLock::new();
    G.get_or_init(|| Graph::from_bytes(rustyroute::data::BYTES_50KM).expect("load 50km graph"))
}

// Eastern Mediterranean and central Red Sea. The only short maritime
// link between them is the Suez Canal; blocking `suezCanal` forces the
// detour around Africa.
const MED: (f64, f64) = (34.0, 28.0);
const RED_SEA: (f64, f64) = (20.0, 38.0);

/// Validate and access a fresh graph handle from the static archive.
/// Exercises the rkyv checked-access path paid at load time.
#[divan::bench]
fn from_bytes() -> Graph {
    Graph::from_bytes(divan::black_box(rustyroute::data::BYTES_50KM)).expect("load 50km graph")
}

/// Dijkstra shortest path Med -> Red Sea with no blocked edges (the
/// short route through the Suez Canal).
#[divan::bench]
fn route_open(bencher: Bencher) {
    let g = graph();
    let blocked = HashSet::new();
    bencher.bench_local(|| {
        g.route(divan::black_box(MED), divan::black_box(RED_SEA), &blocked)
            .expect("open Med->Red Sea route exists")
    });
}

/// Dijkstra shortest path Med -> Red Sea with the Suez Canal blocked,
/// forcing the long detour around Africa — a much larger explored
/// frontier than the open route.
#[divan::bench]
fn route_suez_blocked(bencher: Bencher) {
    let g = graph();
    let suez = g.edges_for_groups(["suezCanal"]).expect("suezCanal group");
    bencher.bench_local(|| {
        g.route(divan::black_box(MED), divan::black_box(RED_SEA), &suez)
            .expect("a detour around Africa still exists")
    });
}

/// Resolve the union of two named chokepoint groups into an edge set.
#[divan::bench]
fn edges_for_groups(bencher: Bencher) {
    let g = graph();
    bencher.bench_local(|| {
        let out: HashSet<EdgeId> = g
            .edges_for_groups(divan::black_box(["suezCanal", "panamaCanal"]))
            .expect("known groups");
        out
    });
}
