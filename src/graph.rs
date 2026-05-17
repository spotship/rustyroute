//! On-disk schema for rustyroute graph archives (the `.rkyv` files
//! produced by `build.rs` for each resolution).
//!
//! # File layout
//!
//! ```text
//! +-----------+-----------+---------------------------------------------+
//! | Offset    | Size      | Content                                     |
//! +-----------+-----------+---------------------------------------------+
//! | 0         | 4 bytes   | ASCII magic: b"RRG1"                        |
//! | 4         | 4 bytes   | u32 little-endian: SCHEMA_VERSION = 1       |
//! | 8         | N bytes   | rkyv-serialised Archived<Graph>             |
//! |           |           | (little-endian; rkyv 0.8 default)           |
//! +-----------+-----------+---------------------------------------------+
//! ```
//!
//! The 8-byte prefix is NOT part of the rkyv payload. Readers slice
//! `&bytes[8..]` before calling `rkyv::access::<ArchivedGraph>`. The
//! magic + version are written explicitly with `u32::to_le_bytes()` so
//! version checks work without parsing rkyv first.
//!
//! # Authoring
//!
//! The writer is `build/archive.rs` in this crate. The reader
//! (`Graph::load`) is out-of-scope for ENG-4678 and will be added in a
//! follow-up ticket — consumers must NOT parse the bytes by hand.
//!
//! # Endianness, alignment, stability
//!
//! - rkyv 0.8 uses little-endian for all primitive types.
//! - rkyv writes 16-byte aligned roots; the 8-byte prefix preserves
//!   relative-offset correctness inside the rkyv payload.
//! - `SCHEMA_VERSION` MUST be bumped on any change to a struct that
//!   derives `rkyv::Archive` (adding, removing, renaming, or reordering
//!   fields, or changing a field's type). rkyv stores fields at fixed
//!   relative offsets within the archived root, so any such change
//!   shifts the on-disk layout and breaks older archives. There is no
//!   "append-only safe" rule — treat every field change as breaking
//!   until a real versioning/migration story is in place.

/// Magic prefix written to every `.rkyv` file before the rkyv payload.
pub const MAGIC: &[u8; 4] = b"RRG1";

/// On-disk schema version. Bump on incompatible layout changes.
pub const SCHEMA_VERSION: u32 = 1;

/// Lon/lat coordinates of one graph node. f32 precision is ~3 m at 60°N
/// — well below the 5 km grid spacing.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug)]
#[rkyv(derive(Debug))]
pub struct NodeCoord {
    pub lng: f32,
    pub lat: f32,
}

/// One directed half-edge in the CSR adjacency. For an undirected edge
/// between distinct nodes A and B, two `DirectedEdge`s are emitted
/// (A→B, B→A) sharing the same `edge_id`. Self-loops (`A == B`) are the
/// one exception: they produce a single `DirectedEdge` with
/// `target == source`, because the "reverse" half would just duplicate
/// the forward one. Either way, the undirected edge has exactly one
/// entry in `Graph::edge_endpoints` and `Graph::undirected_weights`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Copy, Debug)]
#[rkyv(derive(Debug))]
pub struct DirectedEdge {
    /// Index into `Graph::nodes` of the destination.
    pub target: u32,
    /// Polyline-summed haversine distance, kilometres.
    pub weight_km: f32,
    /// Undirected edge id: index into `Graph::edge_endpoints` and
    /// `Graph::undirected_weights`. Same for the A→B and B→A halves.
    pub edge_id: u32,
}

/// One named edge group (chokepoint / passage). `edge_ids` is sorted.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug)]
#[rkyv(derive(Debug))]
pub struct GroupEntry {
    pub name: String,
    pub edge_ids: Vec<u32>,
}

/// The complete CSR graph for one resolution. See module docs for the
/// on-disk byte layout.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug)]
#[rkyv(derive(Debug))]
pub struct Graph {
    /// Node coordinate table. Index = node id.
    pub nodes: Vec<NodeCoord>,
    /// CSR row pointers. `len() == nodes.len() + 1`. The last element
    /// equals `edges.len()`.
    pub node_offsets: Vec<u32>,
    /// All directed half-edges, sorted by source node.
    pub edges: Vec<DirectedEdge>,
    /// Endpoints of each undirected edge: `(src_node_id, dst_node_id)`.
    /// Indexed by `DirectedEdge::edge_id`.
    pub edge_endpoints: Vec<(u32, u32)>,
    /// Weight of each undirected edge (km). Indexed by edge_id.
    pub undirected_weights: Vec<f32>,
    /// 13 named groups in the fixed `EDGE_GROUPS` order.
    pub groups: Vec<GroupEntry>,
}
