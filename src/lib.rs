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

#![deny(unsafe_code)]

pub mod data;
pub mod graph;
mod loader;

pub use crate::loader::{EdgeId, Graph, LoadError, NodeId, Route, RouteError};

include!(concat!(env!("OUT_DIR"), "/edge_groups.rs"));

/// Compiles the `README.md` code fences as doctests so the quickstarts
/// cannot drift from the API (ENG-4683). `#[cfg(doctest)]` keeps the
/// README out of rendered rustdoc output — it exists only during
/// `cargo test --doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
