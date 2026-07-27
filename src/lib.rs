//! Maritime sea-routing primitives for Rust.
//!
//! This crate exposes a pre-baked graph for each of five resolutions
//! (5/10/20/50/100 km) via three layered APIs:
//!
//! - [`data`] holds feature-gated static byte slices
//!   (`BYTES_5KM`...`BYTES_100KM`), `include_bytes!`-baked at compile
//!   time from `build.rs` output.
//! - [`Graph::from_bytes`] validates such a slice and returns a
//!   handle whose `archived()` method exposes the rkyv-zero-copy
//!   graph data. Works on every target including `wasm32`.
//! - [`Graph::load`] mmaps the graph from disk on native targets,
//!   falling back to the static slice when no path source resolves.
//!
//! Once a [`Graph`] is loaded, [`Graph::route`] runs a Dijkstra
//! shortest path with a caller-supplied set of blocked undirected
//! edges (see [`Graph::edges_for_groups`] to resolve the 13 named
//! chokepoint/passage groups into an [`EdgeId`] set). Distance matrices
//! and further algorithms follow in later tickets. See `README.md` and
//! `NOTICE` for project status and upstream attribution.

#![deny(
    unsafe_code,
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    rust_2024_compatibility,
    rustdoc::broken_intra_doc_links
)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    // Every module here is named for the concept it owns (`data`,
    // `graph`, `loader`), so `graph::GraphData` and `data::DATA_LEN_*KM`
    // repeat the module name by design — renaming them to satisfy the
    // lint would make the public API read worse, not better.
    clippy::module_name_repetitions,
    // `RouteError` / `LoadError` are `thiserror` enums whose `#[error]`
    // messages document each failure mode at the variant, and the
    // fallible public methods (`from_bytes`, `load`, `route`,
    // `edges_for_groups`) each carry a hand-written `# Errors` section.
    clippy::missing_errors_doc,
    // NOT "no public panics" — `Graph::archived` and `Graph::route` both
    // contain a deliberate `.expect(...)` on an invariant established at
    // construction. Both document it in a `# Panics` section; this allow
    // exists so the lint does not also demand one on the infallible
    // getters that merely call `archived()` internally.
    clippy::missing_panics_doc
)]

pub mod data;
pub mod graph;
mod loader;

pub use crate::loader::{EdgeId, Graph, LoadError, NodeId, Route, RouteError};

include!(concat!(env!("OUT_DIR"), "/edge_groups.rs"));
