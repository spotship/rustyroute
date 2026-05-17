//! End-to-end tests for the build.rs output (ENG-4678).
//!
//! These tests load each archive from OUT_DIR, validate its
//! magic+schema-version prefix, and assert:
//!   - all 5 archives exist and are non-empty
//!   - bytes-per-edge sanity (≤ 64 bytes/edge)
//!   - rkyv::access succeeds (i.e. archive is well-formed)
//!   - all 13 groups are non-empty
//!   - EDGE_GROUPS has the expected 13 names in expected order
//!   - menaiStrait counts match Python ground truth at every resolution
//!   - self-loops are preserved (≥10 per file)
//!
//! Tests intentionally use only the public `rustyroute::graph` API and
//! the safe `rkyv::access` API — no unsafe, no internal-build-detail
//! coupling.

use rustyroute::graph::{ArchivedGraph, MAGIC, SCHEMA_VERSION};

const RESOLUTIONS: &[u32] = &[5, 10, 20, 50, 100];

// The five `.rkyv` archives are embedded into this integration-test
// binary only — never into the published `rustyroute` library — so
// downstream consumers don't pay for ~MB of test fixtures in their
// dependency graph. `OUT_DIR` is set for every compile unit of this
// package, including integration tests, so `include_bytes!` resolves
// the same paths `build.rs` writes.
fn archive_bytes(res_km: u32) -> &'static [u8] {
    match res_km {
        5 => include_bytes!(concat!(env!("OUT_DIR"), "/data/5km.rkyv")),
        10 => include_bytes!(concat!(env!("OUT_DIR"), "/data/10km.rkyv")),
        20 => include_bytes!(concat!(env!("OUT_DIR"), "/data/20km.rkyv")),
        50 => include_bytes!(concat!(env!("OUT_DIR"), "/data/50km.rkyv")),
        100 => include_bytes!(concat!(env!("OUT_DIR"), "/data/100km.rkyv")),
        _ => panic!("unknown resolution: {res_km}"),
    }
}

// Validate the 8-byte prefix and return the rkyv payload slice. Every
// test that calls `rkyv::access` should go through this so a truncated
// or corrupt archive fails with a clear assertion instead of an index
// out-of-bounds panic.
fn rkyv_payload(res_km: u32) -> &'static [u8] {
    let bytes = archive_bytes(res_km);
    assert!(
        bytes.len() > 8,
        "archive for {res_km}km is too small: {} bytes (need >8 for prefix)",
        bytes.len()
    );
    assert_eq!(&bytes[0..4], MAGIC, "wrong magic for {res_km}km");
    let v = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(v, SCHEMA_VERSION, "wrong schema version for {res_km}km");
    &bytes[8..]
}

#[test]
fn all_five_archives_exist_with_magic() {
    // The header-validation assertions live in `rkyv_payload`; calling
    // it for every resolution is the test.
    for &n in RESOLUTIONS {
        let _ = rkyv_payload(n);
    }
}

#[test]
fn bytes_per_edge_within_bound() {
    for &n in RESOLUTIONS {
        let bytes = archive_bytes(n);
        let archived = rkyv::access::<ArchivedGraph, rkyv::rancor::Error>(rkyv_payload(n))
            .expect("access archived graph");
        let n_edges = archived.edge_endpoints.len() as u64;
        let bpe = bytes.len() as u64 / n_edges.max(1);
        assert!(
            bpe <= 64,
            "{n}km: {} bytes / {} edges = {} bpe (must be <= 64)",
            bytes.len(),
            n_edges,
            bpe
        );
    }
}

#[test]
fn edge_groups_has_thirteen_in_order() {
    assert_eq!(rustyroute::EDGE_GROUPS.len(), 13);
    let expected = [
        "suezCanal",
        "panamaCanal",
        "malaccaStrait",
        "gibraltarStrait",
        "doverStrait",
        "beringStrait",
        "magellanStrait",
        "babElMandebStrait",
        "kielCanal",
        "corinthCanal",
        "northwestPassage",
        "northeastPassage",
        "menaiStrait",
    ];
    for (i, name) in expected.iter().enumerate() {
        assert_eq!(rustyroute::EDGE_GROUPS[i], *name, "EDGE_GROUPS[{i}] drift");
    }
}

#[test]
fn all_groups_non_empty_every_resolution() {
    for &n in RESOLUTIONS {
        let archived = rkyv::access::<ArchivedGraph, rkyv::rancor::Error>(rkyv_payload(n))
            .expect("access archived graph");
        assert_eq!(archived.groups.len(), 13, "{n}km: group count != 13");
        for (i, g) in archived.groups.iter().enumerate() {
            assert!(
                !g.edge_ids.is_empty(),
                "{n}km: group {} ({}) is empty",
                i,
                g.name.as_str()
            );
        }
    }
}

#[test]
fn group_names_match_edge_groups_constant() {
    for &n in RESOLUTIONS {
        let archived =
            rkyv::access::<ArchivedGraph, rkyv::rancor::Error>(rkyv_payload(n)).expect("access");
        for (i, g) in archived.groups.iter().enumerate() {
            assert_eq!(
                g.name.as_str(),
                rustyroute::EDGE_GROUPS[i],
                "{n}km: name mismatch at {i}"
            );
        }
    }
}

#[test]
fn menai_strait_counts_match_ground_truth() {
    // Python-verified counts (see spec § "Menai Strait derivation"). If
    // upstream MARNET changes, these will fail loudly — update both the
    // ground truth and the rerun-if-changed list in build/mod.rs.
    let expected: &[(u32, usize)] = &[(5, 4), (10, 2), (20, 2), (50, 3), (100, 1)];
    for &(n, want) in expected {
        let archived =
            rkyv::access::<ArchivedGraph, rkyv::rancor::Error>(rkyv_payload(n)).expect("access");
        let menai = archived
            .groups
            .iter()
            .find(|g| g.name.as_str() == "menaiStrait")
            .expect("menaiStrait present");
        let got = menai.edge_ids.len();
        assert_eq!(
            got, want,
            "{n}km: menaiStrait expected {want} edges, got {got}"
        );
    }
}

#[test]
fn self_loops_preserved() {
    for &n in RESOLUTIONS {
        let archived =
            rkyv::access::<ArchivedGraph, rkyv::rancor::Error>(rkyv_payload(n)).expect("access");
        // Archived<(u32, u32)> exposes `.0` and `.1` as ArchivedU32; use
        // .to_native() for the comparison so the test is portable across
        // rkyv's archived primitive wrappers.
        let n_self = archived
            .edge_endpoints
            .iter()
            .filter(|ep| ep.0.to_native() == ep.1.to_native())
            .count();
        // Verified counts: 14/15/19/20/14 across 5/10/20/50/100km.
        assert!(
            n_self >= 10,
            "{n}km: only {n_self} self-loops; expected >=10"
        );
    }
}

#[test]
fn csr_structural_invariants() {
    for &n in RESOLUTIONS {
        let archived =
            rkyv::access::<ArchivedGraph, rkyv::rancor::Error>(rkyv_payload(n)).expect("access");
        assert_eq!(
            archived.node_offsets.len(),
            archived.nodes.len() + 1,
            "{n}km: node_offsets length"
        );
        let last_native = archived.node_offsets[archived.node_offsets.len() - 1].to_native();
        assert_eq!(
            last_native as usize,
            archived.edges.len(),
            "{n}km: node_offsets last != edges.len()"
        );
        let n_nodes = archived.nodes.len() as u32;
        for e in archived.edges.iter() {
            let t = e.target.to_native();
            assert!(t < n_nodes, "{n}km: edge target {t} out of range {n_nodes}");
        }
    }
}

#[test]
fn rkyv_format_docstring_present() {
    // AC: "rkyv file format documented in module-level doc comment."
    // We check the literal file rather than rustdoc output to avoid a
    // rustdoc build step inside the test. Build the path from
    // CARGO_MANIFEST_DIR so the test doesn't depend on the test binary's
    // current working directory.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/graph.rs");
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        src.contains("//! On-disk schema"),
        "missing module-level doc"
    );
    assert!(src.contains("MAGIC"), "module doc missing MAGIC reference");
    assert!(
        src.contains("SCHEMA_VERSION"),
        "module doc missing SCHEMA_VERSION reference"
    );
    assert!(
        src.contains("rkyv-serialised"),
        "module doc missing rkyv reference"
    );
}
