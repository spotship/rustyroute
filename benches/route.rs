//! ENG-4690: criterion benchmarks for the routing hot path.
//!
//! Authored against `codspeed-criterion-compat` (a drop-in criterion
//! replacement): under `cargo bench` it behaves as plain criterion
//! (with html_reports); under `cargo codspeed` it registers these
//! benches with the CodSpeed instrumentation harness for regression
//! tracking on PRs.
//!
//! Every graph is loaded via `Graph::load(N)` — the production entry
//! point. `build.rs` bakes all five resolutions into `$OUT_DIR/data`
//! regardless of `data-*` features, so `Graph::load(5)` works in-tree
//! under the default feature set; no extra feature flag is needed to
//! run the whole suite.

use std::collections::HashSet;

use codspeed_criterion_compat::{Criterion, black_box, criterion_group, criterion_main};
use rustyroute::{EdgeId, Graph};

// (lat, lng) decimal degrees — open-water points near each port.
// Verified during implementation to snap to routable sea nodes (see
// Step 4). Marseille<->Shanghai is the flagship Suez-dependent route;
// Singapore<->Yokohama exercises the heavy 5km graph.
const MARSEILLE: (f64, f64) = (43.30, 5.37);
const SHANGHAI: (f64, f64) = (31.23, 121.47);
const SINGAPORE: (f64, f64) = (1.26, 103.83);
const YOKOHAMA: (f64, f64) = (35.44, 139.64);

fn graph_load_50km(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_load_50km");
    // Cold: full mmap + magic/version validation + rkyv checked access.
    group.bench_function("cold", |b| {
        b.iter(|| black_box(Graph::load(black_box(50)).expect("load 50km graph")));
    });
    // Warm: graph already loaded; measure warm archived() re-access via
    // node_count, which re-runs rkyv checked access on each call.
    let g = Graph::load(50).expect("load 50km graph");
    group.bench_function("warm", |b| {
        b.iter(|| black_box(g.node_count()));
    });
    group.finish();
}

fn route_marseille_shanghai_50km(c: &mut Criterion) {
    let g = Graph::load(50).expect("load 50km graph");
    let empty: HashSet<EdgeId> = HashSet::new();
    let suez = g
        .edges_for_groups(["suezCanal"])
        .expect("suezCanal group resolves");

    let mut group = c.benchmark_group("route_marseille_shanghai_50km");
    group.bench_function("open", |b| {
        b.iter(|| {
            black_box(
                g.route(black_box(MARSEILLE), black_box(SHANGHAI), &empty)
                    .expect("open Marseille->Shanghai route exists"),
            )
        });
    });
    group.bench_function("suez_blocked", |b| {
        b.iter(|| {
            black_box(
                g.route(black_box(MARSEILLE), black_box(SHANGHAI), &suez)
                    .expect("Suez-blocked Marseille->Shanghai detour exists"),
            )
        });
    });
    group.finish();
}

fn route_singapore_yokohama_5km(c: &mut Criterion) {
    // Heavy graph; warm only (5km cold-load is too noisy — ticket note).
    let g = Graph::load(5).expect("load 5km graph");
    let empty: HashSet<EdgeId> = HashSet::new();
    c.bench_function("route_singapore_yokohama_5km", |b| {
        b.iter(|| {
            black_box(
                g.route(black_box(SINGAPORE), black_box(YOKOHAMA), &empty)
                    .expect("Singapore->Yokohama route exists"),
            )
        });
    });
}

fn edges_for_groups_all_13(c: &mut Criterion) {
    let g = Graph::load(50).expect("load 50km graph");
    c.bench_function("edges_for_groups_all_13", |b| {
        b.iter(|| {
            black_box(
                g.edges_for_groups(black_box(rustyroute::EDGE_GROUPS).iter().copied())
                    .expect("all 13 groups resolve"),
            )
        });
    });
}

criterion_group!(
    benches,
    graph_load_50km,
    route_marseille_shanghai_50km,
    route_singapore_yokohama_5km,
    edges_for_groups_all_13
);
criterion_main!(benches);
