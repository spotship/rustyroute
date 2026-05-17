//! Maritime sea-routing primitives for Rust.
//!
//! This crate is intentionally pre-API. ENG-4678 introduces the graph
//! archive format (see [`graph`]) and the `EDGE_GROUPS` registry
//! (added by Task 6). Routing APIs (`Graph::load`, Dijkstra, distance
//! matrices) follow in later tickets. See `README.md` and `NOTICE`
//! for project status and upstream attribution.

#![forbid(unsafe_code)]

pub mod graph;
