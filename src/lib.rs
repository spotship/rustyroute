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
//! Routing algorithms (Dijkstra, distance matrices) follow in later
//! tickets. See `README.md` and `NOTICE` for project status and
//! upstream attribution.

#![deny(unsafe_code)]

pub mod data;
pub mod graph;
pub mod loader;

pub use crate::loader::{Graph, LoadError};

include!(concat!(env!("OUT_DIR"), "/edge_groups.rs"));
