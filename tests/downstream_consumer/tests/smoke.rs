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
