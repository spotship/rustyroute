//! ENG-4680 smoke tests for the routing API: self-route, blocked-edge
//! detour (Suez), edge-group lookup, coordinate validation, and the
//! all-blocked `NoRoute` path. The heavier golden-route distance table
//! lives in ENG-4681.
// ENG-4684: `float_cmp` is allowed because
// `assert_eq!(route.distance_km, 0.0)` for a self-route is an exact-zero
// assertion, not an approximate one. `Graph::route` returns a literal
// `0.0` on the `start == goal` short circuit without summing any edge
// weights, so there is no accumulated error for a tolerance to absorb --
// and a tolerance would hide a regression that started returning a small
// non-zero distance.
#![allow(clippy::float_cmp)]
#![cfg(feature = "data-50km")]

use rustyroute::{EdgeId, Graph, RouteError};
use std::collections::HashSet;
use std::sync::OnceLock;

// Load and validate the 50km archive once for the whole test binary.
// `Graph::from_bytes` re-runs rkyv's checked access over the (large)
// archive on every call, so caching a single handle in a `OnceLock`
// avoids repeating that work per test. Every routing method takes
// `&self`, so a shared `&'static Graph` keeps test semantics identical.
fn graph() -> &'static Graph {
    static G: OnceLock<Graph> = OnceLock::new();
    G.get_or_init(|| Graph::from_bytes(rustyroute::data::BYTES_50KM).expect("load 50km graph"))
}

// Eastern Mediterranean and central Red Sea. The only short maritime
// link between them is the Suez Canal; blocking `suezCanal` forces the
// detour around Africa.
const MED: (f64, f64) = (34.0, 28.0);
const RED_SEA: (f64, f64) = (20.0, 38.0);

/// AC: self-route (`from == to`, snapping to one node) yields a
/// single-coordinate path, zero distance, and no edges.
#[test]
fn self_route_is_single_point_zero_distance() {
    let g = graph();
    let route = g
        .route(MED, MED, &HashSet::new())
        .expect("self-route should succeed");
    assert_eq!(route.coordinates.len(), 1, "self-route has one coordinate");
    assert_eq!(route.distance_km, 0.0, "self-route distance is zero");
    assert!(route.edge_ids.is_empty(), "self-route traverses no edges");
}

/// AC: routing across Suez with `suezCanal` blocked is strictly longer
/// than without (inequality only — exact numbers live in ENG-4681).
#[test]
fn suez_block_forces_strictly_longer_route() {
    let g = graph();
    let open = g
        .route(MED, RED_SEA, &HashSet::new())
        .expect("open Med->Red Sea route exists");
    assert!(open.distance_km > 0.0, "open route has positive distance");

    let suez = g.edges_for_groups(["suezCanal"]).expect("suezCanal group");
    assert!(!suez.is_empty(), "suezCanal group is non-empty");

    let blocked = g
        .route(MED, RED_SEA, &suez)
        .expect("a detour around Africa still exists");
    assert!(
        blocked.distance_km > open.distance_km,
        "blocking Suez ({blocked_km:.1}km) must exceed the open route ({open_km:.1}km)",
        blocked_km = blocked.distance_km,
        open_km = open.distance_km,
    );
}

/// AC: `edges_for_groups` returns the union of the named groups; an
/// unknown name yields `UnknownGroup` naming the offender.
#[test]
fn edges_for_groups_union_and_unknown() {
    let g = graph();
    let suez = g.edges_for_groups(["suezCanal"]).expect("suezCanal");
    let panama = g.edges_for_groups(["panamaCanal"]).expect("panamaCanal");
    let union = g
        .edges_for_groups(["suezCanal", "panamaCanal"])
        .expect("union");

    let expected: HashSet<_> = suez.union(&panama).copied().collect();
    assert_eq!(union, expected, "union must equal suez ∪ panama");
    assert!(union.is_superset(&suez) && union.is_superset(&panama));

    match g.edges_for_groups(["foo"]) {
        Err(RouteError::UnknownGroup(name)) => assert_eq!(name, "foo"),
        other => panic!("expected UnknownGroup(\"foo\"), got {other:?}"),
    }
    // A valid name followed by an unknown one still errors on the
    // unknown, and reports it by name.
    match g.edges_for_groups(["suezCanal", "nope"]) {
        Err(RouteError::UnknownGroup(name)) => assert_eq!(name, "nope"),
        other => panic!("expected UnknownGroup(\"nope\"), got {other:?}"),
    }
}

/// AC: with every path to the target severed, `route` returns
/// `NoRoute`. Blocking the whole edge set isolates every node — the
/// degenerate "island" case; realistic island scenarios are covered by
/// ENG-4681's golden routes.
#[test]
fn all_edges_blocked_yields_no_route() {
    let g = graph();
    let all: HashSet<EdgeId> = (0..g.edge_count()).collect();
    match g.route(MED, RED_SEA, &all) {
        Err(RouteError::NoRoute) => {}
        other => panic!("expected NoRoute, got {other:?}"),
    }
}

/// Out-of-range or non-finite endpoints are rejected before any search,
/// attributing the failure to the offending endpoint.
#[test]
fn out_of_bounds_coordinates_are_rejected() {
    let g = graph();
    let empty = HashSet::new();

    match g.route((91.0, 0.0), RED_SEA, &empty) {
        Err(RouteError::BadFromCoord(c)) => assert_eq!(c, (91.0, 0.0)),
        other => panic!("expected BadFromCoord, got {other:?}"),
    }
    match g.route(MED, (0.0, 181.0), &empty) {
        Err(RouteError::BadToCoord(c)) => assert_eq!(c, (0.0, 181.0)),
        other => panic!("expected BadToCoord, got {other:?}"),
    }
    match g.route((f64::NAN, 0.0), RED_SEA, &empty) {
        Err(RouteError::BadFromCoord(_)) => {}
        other => panic!("expected BadFromCoord for NaN, got {other:?}"),
    }
}
