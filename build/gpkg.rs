//! Raw edge data extracted from one row of a vendored `.gpkg`.
//!
//! `RawEdge` is plain data and is re-included via `#[path]` by tests
//! that exercise the build pipeline (`tests/group_assignment.rs`,
//! `tests/tampered_gpkg_panic.rs`). The GeoPackage SQLite reader
//! (`iter_edges`) lives in `build/gpkg_io.rs`; `rusqlite` is declared
//! under both `[build-dependencies]` and `[dev-dependencies]` so the
//! end-to-end tampered-gpkg test can re-include `gpkg_io.rs` and drive
//! the real reader. Tests that don't need the SQLite reader (e.g.
//! `group_assignment.rs`) construct `RawEdge` values directly instead.

pub struct RawEdge {
    pub fid: i64,
    pub pass: Option<String>,
    /// LineString vertices in (lng, lat) order. Length >= 2.
    pub points: Vec<(f64, f64)>,
}
