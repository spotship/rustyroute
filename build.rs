//! ENG-4678: compile vendored Eurostat MARNET GeoPackages into rkyv
//! graph archives + emit `pub const EDGE_GROUPS` for the crate root.
//!
//! Reads each `vendor/eurostat-marnet/marnet_plus_{N}km.gpkg`, builds
//! a CSR adjacency, classifies edges into 13 named groups, and writes
//! `$OUT_DIR/data/{N}km.rkyv`. Also emits `$OUT_DIR/edge_groups.rs`
//! consumed by `src/lib.rs` via `include!`.

#[path = "src/graph.rs"]
mod graph;

#[path = "build/mod.rs"]
mod build;

fn main() {
    build::run();
}
