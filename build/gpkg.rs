//! Raw edge data extracted from one row of a vendored `.gpkg`.
//!
//! The `RawEdge` struct is plain data and is reused by tests under
//! `tests/group_assignment.rs` that re-include this file via `#[path]`.
//! The actual GeoPackage SQLite reader (`iter_edges`) lives in
//! `build/gpkg_io.rs` — it depends on `rusqlite`, which is in
//! `[build-dependencies]` only, so integration tests cannot see it.
//! Tests construct `RawEdge` values directly.

pub struct RawEdge {
    pub fid: i64,
    pub pass: Option<String>,
    /// LineString vertices in (lng, lat) order. Length >= 2.
    pub points: Vec<(f64, f64)>,
}
