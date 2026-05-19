//! ENG-4679: downstream-consumer sub-package smoke library. Exists
//! solely so the sub-package compiles as a real crate; the actual
//! smoke assertions live in `tests/smoke.rs`.

/// Round-trip both public entrypoints. Returns `(node_count,
/// edge_count, directed_edge_count)` from the loaded graph for any
/// caller that wants to assert on shape.
pub fn exercise_public_api() -> (u32, u32, u32) {
    use rustyroute::Graph;
    // Path 1: Graph::load via the published-crate convenience.
    let loaded = Graph::load(50).expect("Graph::load(50)");
    assert_eq!(loaded.resolution_km(), 50);

    // Path 2: Graph::from_bytes via the static BYTES_50KM const.
    let static_g = Graph::from_bytes(rustyroute::data::BYTES_50KM)
        .expect("Graph::from_bytes(BYTES_50KM)");

    // Sanity: both observations agree on shape.
    assert_eq!(loaded.node_count(), static_g.node_count());
    assert_eq!(loaded.edge_count(), static_g.edge_count());
    assert_eq!(loaded.directed_edge_count(), static_g.directed_edge_count());

    (
        loaded.node_count(),
        loaded.edge_count(),
        loaded.directed_edge_count(),
    )
}
