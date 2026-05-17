//! Compile-time and round-trip checks for the rkyv schema in src/graph.rs.

use rustyroute::graph::{
    ArchivedGraph, DirectedEdge, Graph, GroupEntry, MAGIC, NodeCoord, SCHEMA_VERSION,
};

#[test]
fn magic_is_rrg1() {
    assert_eq!(MAGIC, b"RRG1");
}

#[test]
fn schema_version_is_one() {
    assert_eq!(SCHEMA_VERSION, 1u32);
}

#[test]
fn rkyv_roundtrip_minimal_graph() {
    let g = Graph {
        nodes: vec![
            NodeCoord { lng: 0.0, lat: 0.0 },
            NodeCoord { lng: 1.0, lat: 2.0 },
        ],
        node_offsets: vec![0, 1, 2],
        edges: vec![
            DirectedEdge {
                target: 1,
                weight_km: 12.34,
                edge_id: 0,
            },
            DirectedEdge {
                target: 0,
                weight_km: 12.34,
                edge_id: 0,
            },
        ],
        edge_endpoints: vec![(0, 1)],
        undirected_weights: vec![12.34],
        groups: vec![GroupEntry {
            name: "test".into(),
            edge_ids: vec![0],
        }],
    };

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&g).expect("serialise");
    let archived =
        rkyv::access::<ArchivedGraph, rkyv::rancor::Error>(&bytes).expect("access archived");
    assert_eq!(archived.nodes.len(), 2);
    assert_eq!(archived.edges.len(), 2);
    assert_eq!(archived.edge_endpoints.len(), 1);
    assert_eq!(archived.groups.len(), 1);
    assert_eq!(archived.groups[0].name.as_str(), "test");
}
