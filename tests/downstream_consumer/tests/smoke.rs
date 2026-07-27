//! Smoke test for the downstream-consumer sub-package. Runs both
//! published-crate entrypoints (Graph::load + Graph::from_bytes via
//! BYTES_50KM) and asserts non-trivial counts.

#[test]
fn round_trip_load_and_from_bytes_50km() {
    let (nodes, undirected, directed) = downstream_consumer::exercise_public_api();
    assert!(nodes > 0, "expected non-zero node count");
    assert!(undirected > 0, "expected non-zero edge_count");
    assert!(
        directed >= undirected,
        "directed_edge_count must be >= edge_count"
    );
}

/// ENG-4683 AC3: a clean external project on default features can run
/// the README's library quickstart verbatim and get a valid `Route` —
/// no env var, no data directory, no build script of its own.
#[test]
fn readme_quickstart_route_works_for_an_external_consumer() {
    let (points, distance_km) = downstream_consumer::exercise_route();
    assert!(
        points > 1,
        "a Marseille->Shanghai route needs many points, got {points}"
    );
    assert!(
        distance_km > 1000.0,
        "expected a plausible ocean crossing, got {distance_km} km"
    );
}
