//! Dedupe LineString endpoints into a node table and build the CSR
//! adjacency for one resolution.

use crate::build::geometry::polyline_length_km;
use crate::build::gpkg::RawEdge;
use crate::graph::{DirectedEdge, GraphData, GroupEntry, NodeCoord};
use std::collections::HashMap;

pub struct CsrBuilt {
    pub nodes: Vec<NodeCoord>,
    pub node_offsets: Vec<u32>,
    pub edges: Vec<DirectedEdge>,
    pub edge_endpoints: Vec<(u32, u32)>,
    pub undirected_weights: Vec<f32>,
    /// Parallel to `edge_endpoints` / `undirected_weights`: which RawEdge
    /// each undirected edge id came from. Used by group assignment.
    pub raw_edge_index: Vec<usize>,
}

impl CsrBuilt {
    pub fn into_graph_with_groups(self, groups: Vec<GroupEntry>) -> GraphData {
        GraphData {
            nodes: self.nodes,
            node_offsets: self.node_offsets,
            edges: self.edges,
            edge_endpoints: self.edge_endpoints,
            undirected_weights: self.undirected_weights,
            groups,
        }
    }
}

pub fn build_csr(raw: &[RawEdge]) -> CsrBuilt {
    // 1. Dedup endpoints into node ids (key = bit-pattern of (lng, lat)).
    let mut node_by_key: HashMap<(u64, u64), u32> = HashMap::new();
    let mut nodes: Vec<NodeCoord> = Vec::new();

    let mut intern = |lng: f64, lat: f64| -> u32 {
        let key = (lng.to_bits(), lat.to_bits());
        if let Some(&id) = node_by_key.get(&key) {
            return id;
        }
        let id = nodes.len() as u32;
        nodes.push(NodeCoord {
            lng: lng as f32,
            lat: lat as f32,
        });
        node_by_key.insert(key, id);
        id
    };

    // 2. Walk raw edges; assign undirected edge_id; compute weight.
    let mut edge_endpoints: Vec<(u32, u32)> = Vec::with_capacity(raw.len());
    let mut undirected_weights: Vec<f32> = Vec::with_capacity(raw.len());
    let mut raw_edge_index: Vec<usize> = Vec::with_capacity(raw.len());

    for (idx, e) in raw.iter().enumerate() {
        let src = intern(e.points[0].0, e.points[0].1);
        let dst = intern(
            e.points[e.points.len() - 1].0,
            e.points[e.points.len() - 1].1,
        );
        let w = polyline_length_km(&e.points) as f32;
        edge_endpoints.push((src, dst));
        undirected_weights.push(w);
        raw_edge_index.push(idx);
    }

    // 3. Build adjacency: out-edges per source node.
    let n_nodes = nodes.len();
    let mut adj: Vec<Vec<DirectedEdge>> = vec![Vec::new(); n_nodes];
    for (edge_id, &(src, dst)) in edge_endpoints.iter().enumerate() {
        let w = undirected_weights[edge_id];
        adj[src as usize].push(DirectedEdge {
            target: dst,
            weight_km: w,
            edge_id: edge_id as u32,
        });
        // Self-loop exception: when src == dst, the "reverse" half-edge
        // would be identical to the forward one (same target, same
        // edge_id) and would just inflate the adjacency list with a
        // duplicate. Emit only the forward half. The self-loop is still
        // represented by exactly one `edge_endpoints` entry with
        // `.0 == .1`, so downstream consumers can still detect it.
        if src != dst {
            adj[dst as usize].push(DirectedEdge {
                target: src,
                weight_km: w,
                edge_id: edge_id as u32,
            });
        }
    }

    // 4. Sort each adjacency list by (target, edge_id) for determinism.
    for list in &mut adj {
        list.sort_by_key(|e| (e.target, e.edge_id));
    }

    // 5. Flatten into CSR.
    let mut node_offsets: Vec<u32> = Vec::with_capacity(n_nodes + 1);
    let mut edges: Vec<DirectedEdge> = Vec::new();
    node_offsets.push(0);
    for list in adj {
        edges.extend(list);
        node_offsets.push(edges.len() as u32);
    }

    // 6. Structural assertions.
    assert_eq!(node_offsets.len(), nodes.len() + 1);
    assert_eq!(*node_offsets.last().unwrap() as usize, edges.len());
    for e in &edges {
        assert!(
            (e.target as usize) < nodes.len(),
            "edge target out of range"
        );
    }

    CsrBuilt {
        nodes,
        node_offsets,
        edges,
        edge_endpoints,
        undirected_weights,
        raw_edge_index,
    }
}
