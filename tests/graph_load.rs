//! ENG-4679 integration tests: `Graph::from_bytes` happy path, the
//! full LoadError matrix (BadMagic / UnsupportedSchema /
//! InvalidArchive), `Graph::load(N)` unknown-resolution rejection,
//! count parity vs `build_csr`, and a compile-time Send+Sync
//! assertion required by Design 019's OnceLock pattern.

#[cfg(feature = "data-50km")]
use rustyroute::graph::{MAGIC, SCHEMA_VERSION};
use rustyroute::{Graph, LoadError};

#[cfg(feature = "data-50km")]
fn fifty_km_bytes() -> &'static [u8] {
    rustyroute::data::BYTES_50KM
}

// --- compile-time Send + Sync assertion ---
const _SEND_SYNC: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<Graph>();
    assert_sync::<Graph>();
};

// --- from_bytes happy path ---
#[cfg(feature = "data-50km")]
#[test]
fn from_bytes_50km_ok() {
    let g = Graph::from_bytes(fifty_km_bytes()).expect("from_bytes");
    assert!(g.node_count() > 0);
    assert!(g.edge_count() > 0);
    assert!(
        g.directed_edge_count() >= g.edge_count(),
        "directed_edge_count must be >= edge_count"
    );
    // from_bytes records 0 as the resolution sentinel.
    assert_eq!(g.resolution_km(), 0);
}

// --- Graph::load(7) -> UnknownResolution(7) ---
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn load_unknown_resolution_seven() {
    match Graph::load(7) {
        Err(LoadError::UnknownResolution(7)) => {}
        other => panic!("expected UnknownResolution(7), got {other:?}"),
    }
}

// --- Graph::load(50) happy path via OUT_DIR ---
#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[test]
fn load_50km_ok_via_out_dir() {
    let g = Graph::load(50).expect("load(50)");
    assert_eq!(g.resolution_km(), 50);
    assert!(g.node_count() > 0);
    assert!(g.edge_count() > 0);
}

// --- BadMagic ---
#[cfg(feature = "data-50km")]
#[test]
fn from_bytes_bad_magic() {
    let mut v = fifty_km_bytes().to_vec();
    v[0] ^= 0xFF; // corrupt first magic byte
    let leaked: &'static [u8] = Box::leak(v.into_boxed_slice());
    match Graph::from_bytes(leaked) {
        Err(LoadError::BadMagic(got)) => {
            assert_ne!(&got, MAGIC, "BadMagic should report the actual bytes seen");
        }
        other => panic!("expected BadMagic, got {other:?}"),
    }
}

// --- UnsupportedSchema ---
#[cfg(feature = "data-50km")]
#[test]
fn from_bytes_unsupported_schema() {
    let mut v = fifty_km_bytes().to_vec();
    // bump version from 1 to 3 (XOR byte 4 with 0x02).
    v[4] ^= 0x02;
    let leaked: &'static [u8] = Box::leak(v.into_boxed_slice());
    match Graph::from_bytes(leaked) {
        Err(LoadError::UnsupportedSchema(ver)) => {
            assert_ne!(ver, SCHEMA_VERSION, "should not equal current schema");
        }
        other => panic!("expected UnsupportedSchema, got {other:?}"),
    }
}

// --- InvalidArchive on payload tamper (no segfault) ---
#[cfg(feature = "data-50km")]
#[test]
fn from_bytes_invalid_archive_on_payload_tamper() {
    // rkyv writes the root struct at the END of the archive (leaf-first
    // layout), so the relative-pointer machinery — which bytecheck
    // validates — sits in the trailing bytes. Flipping a byte 16 bytes
    // from the end (inside the root struct's pointer/length fields)
    // reliably trips bytecheck with InvalidArchive on the current
    // schema. A byte deep inside the node/edge tables would land in
    // coordinate or weight data and would NOT trigger bytecheck —
    // those are scalar payload, not pointer machinery.
    let mut v = fifty_km_bytes().to_vec();
    assert!(v.len() > 64, "test fixture too small");
    let tamper_offset = v.len() - 16;
    v[tamper_offset] ^= 0xFF;
    let leaked: &'static [u8] = Box::leak(v.into_boxed_slice());
    match Graph::from_bytes(leaked) {
        Err(LoadError::InvalidArchive(_)) => {}
        Ok(_) => panic!("payload tamper at offset {tamper_offset} (len-16) was not detected"),
        Err(other) => panic!("expected InvalidArchive, got {other:?}"),
    }
}

// --- truncated header → BadMagic ---
#[test]
fn from_bytes_truncated_header() {
    let v: &[u8] = b"RRG"; // 3 bytes — less than the 8-byte header
    let leaked: &'static [u8] = Box::leak(v.to_vec().into_boxed_slice());
    match Graph::from_bytes(leaked) {
        Err(LoadError::BadMagic(_)) => {}
        other => panic!("expected BadMagic on truncated header, got {other:?}"),
    }
}

// --- AC4 happy half: load(100) ok when data-100km is on ---
#[cfg(all(not(target_arch = "wasm32"), feature = "data-100km"))]
#[test]
fn load_100km_ok() {
    let g = Graph::load(100).expect("load(100)");
    assert_eq!(g.resolution_km(), 100);
    assert!(g.node_count() > 0);
}

// AC8: node_count/edge_count/directed_edge_count parity vs build_csr.
//
// We rebuild the CSR from the vendored .gpkg using the same pipeline
// build.rs uses, then load the corresponding .rkyv via Graph::load and
// compare counts.
//
// `tests/build_helpers_csr.rs` is included here (not as a separate
// integration-test binary) so that the helper module can resolve
// `crate::build::*` and `crate::graph` — the same alias pattern used by
// `tests/group_assignment.rs:4-26`.

#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[path = "../build/csr.rs"]
pub mod csr;
#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[path = "../build/geometry.rs"]
pub mod geometry;
#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[path = "../build/gpkg.rs"]
pub mod gpkg;
#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[path = "../build/gpkg_io.rs"]
pub mod gpkg_io;
// `build/csr.rs` references `crate::graph::{DirectedEdge, GraphData,
// GroupEntry, NodeCoord}`. Re-include `src/graph.rs` here so the
// `crate::graph` path resolves inside this integration-test crate.
// (The schema struct is the SAME source — but Rust's type system sees
// this re-include as a separate type from `rustyroute::graph::*`. We
// only use it via `csr::build_csr` for counting purposes, so the
// type-identity drift is invisible.)
#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[path = "../src/graph.rs"]
pub mod graph;

// Alias module so that `crate::build::{geometry, gpkg}` resolves to
// the local `#[path]`-included modules above (csr and gpkg_io both
// reference `crate::build::geometry` and `crate::build::gpkg`). Same
// alias pattern as `tests/group_assignment.rs:24-26`.
#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
mod build {
    pub use super::{geometry, gpkg};
}

#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
struct GroundTruth {
    nodes: usize,
    undirected_edges: usize,
    directed_half_edges: usize,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
fn ground_truth_for(resolution_km: u32) -> GroundTruth {
    use std::path::PathBuf;
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join(format!(
        "vendor/eurostat-marnet/marnet_plus_{resolution_km}km.gpkg"
    ));
    let raw = gpkg_io::iter_edges(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let built = csr::build_csr(&raw);
    GroundTruth {
        nodes: built.nodes.len(),
        undirected_edges: built.edge_endpoints.len(),
        directed_half_edges: built.edges.len(),
    }
}

// AC9: cold load(50) < 50ms, warm load(50) < 1ms on a stock dev
// machine. `#[ignore]` because CI runners vary too much for a hard
// gate; run manually via `cargo test -- --ignored`.
#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[test]
#[ignore = "timing; run with --ignored"]
fn load_50km_cold_under_50ms_warm_under_1ms() {
    use std::time::{Duration, Instant};
    let t0 = Instant::now();
    let g0 = Graph::load(50).expect("cold load");
    let cold = t0.elapsed();
    drop(g0);

    let t1 = Instant::now();
    let _g1 = Graph::load(50).expect("warm load");
    let warm = t1.elapsed();

    assert!(cold < Duration::from_millis(50), "cold load took {cold:?}");
    assert!(warm < Duration::from_millis(1), "warm load took {warm:?}");
}

#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[test]
fn counts_match_build_csr_for_every_resolution() {
    for &n in &[5u32, 10, 20, 50, 100] {
        let truth = ground_truth_for(n);
        let g = match Graph::load(n) {
            Ok(g) => g,
            Err(LoadError::DataNotAvailable(_)) => {
                // Allowed when the feature is off; skip this resolution.
                continue;
            }
            Err(e) => panic!("load({n}) unexpected error: {e:?}"),
        };
        assert_eq!(
            g.node_count() as usize,
            truth.nodes,
            "{n}km node_count mismatch"
        );
        assert_eq!(
            g.edge_count() as usize,
            truth.undirected_edges,
            "{n}km edge_count mismatch"
        );
        assert_eq!(
            g.directed_edge_count() as usize,
            truth.directed_half_edges,
            "{n}km directed_edge_count mismatch"
        );
    }
}
