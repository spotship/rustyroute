//! End-to-end test for the empty-group panic AC (ENG-4678).
//!
//! Spec acceptance criterion:
//!   "Build fails (non-zero exit, clear error) if you hand-edit a .gpkg
//!    to remove a Suez edge."
//!
//! Strategy: rather than spawning `cargo build` as a subprocess (slow;
//! rebuilds the SQLite amalgamation in a fresh target dir), this test
//! drives the SAME pipeline that `build.rs` runs — the real
//! `gpkg_io::iter_edges` reader, the real `csr::build_csr`, the real
//! `groups::assign_groups` — against a tampered copy of a vendored
//! .gpkg with the Suez row removed.
//!
//! What this proves:
//!   - The actual SQLite reader (rusqlite) correctly returns rows from
//!     a tampered .gpkg copy.
//!   - When the Suez row is absent, the build pipeline panics with
//!     the documented message naming the empty group (`suezCanal`)
//!     and the resolution.
//!   - A clean copy of the same .gpkg goes through the pipeline
//!     without panicking — proving the test is exercising the right
//!     failure surface, not a coincidence.
//!
//! Notes:
//!   - Uses the 100km resolution because it is the smallest (~1 MB)
//!     and has exactly 1 Suez row, so a single DELETE empties the
//!     group.
//!   - The vendored .gpkg is never mutated; only a tempdir copy is
//!     edited.

#[path = "../build/csr.rs"]
pub mod csr;
#[path = "../build/geometry.rs"]
pub mod geometry;
#[path = "../build/gpkg.rs"]
pub mod gpkg;
#[path = "../build/gpkg_io.rs"]
pub mod gpkg_io;
#[path = "../src/graph.rs"]
pub mod graph;
#[path = "../build/groups.rs"]
pub mod groups;

// `gpkg_io.rs` and `groups.rs` use `crate::build::{geometry,gpkg,csr}`
// internally. The alias module below makes those paths resolve in
// this test crate without touching the source files (matching the
// pattern used by tests/group_assignment.rs).
mod build {
    pub use super::{csr, geometry, gpkg};
}

use std::path::PathBuf;

/// Copy the 100km vendored .gpkg into a fresh tempdir and return the path.
fn fresh_gpkg_copy(tmp: &tempfile::TempDir) -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest.join("vendor/eurostat-marnet/marnet_plus_100km.gpkg");
    let dst = tmp.path().join("marnet_plus_100km.gpkg");
    std::fs::copy(&src, &dst).expect("copy vendored .gpkg into tempdir");
    dst
}

/// Confirm that the unmodified .gpkg copy goes through the full
/// pipeline without panicking. This is the "control" half of the
/// empty-group test — if this ever starts panicking, the test below
/// would be triggering on the wrong condition.
#[test]
fn untampered_gpkg_pipeline_succeeds() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = fresh_gpkg_copy(&tmp);

    let raw = gpkg_io::iter_edges(&path).expect("read untampered .gpkg");
    assert!(!raw.is_empty(), "vendored 100km .gpkg returned 0 rows");
    let suez_count = raw
        .iter()
        .filter(|r| r.pass.as_deref() == Some("suez"))
        .count();
    assert_eq!(
        suez_count, 1,
        "100km .gpkg expected exactly 1 suez row; got {suez_count}"
    );

    let built = csr::build_csr(&raw);
    // Should NOT panic: every group has at least one edge in clean data.
    let groups_out = groups::assign_groups(&raw, &built, 100);
    assert_eq!(groups_out.len(), 13);
}

/// Tamper a copy of the 100km .gpkg by deleting the Suez row, then
/// drive the build pipeline. Must panic with the documented message
/// naming the empty group and the resolution.
///
/// This is the acceptance-criterion test: `cargo build` against a
/// tampered .gpkg would fire this exact panic (build.rs calls the
/// same `groups::assign_groups` we call here), producing a non-zero
/// build exit code and the message printed in the build output.
#[test]
fn tampered_gpkg_removing_suez_panics_at_build_pipeline() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = fresh_gpkg_copy(&tmp);

    // Remove the Suez row using the real SQLite engine — same engine
    // build.rs reads with, so the post-edit byte layout is what
    // build.rs would see.
    {
        let conn = rusqlite::Connection::open(&path).expect("open sqlite for tamper");
        let deleted = conn
            .execute("DELETE FROM type WHERE pass = 'suez'", [])
            .expect("delete suez row");
        assert_eq!(
            deleted, 1,
            "expected 1 suez row to delete; deleted {deleted}"
        );
    }

    let raw = gpkg_io::iter_edges(&path).expect("read tampered .gpkg");
    assert_eq!(
        raw.iter()
            .filter(|r| r.pass.as_deref() == Some("suez"))
            .count(),
        0,
        "tampered .gpkg still contains a suez row"
    );

    let built = csr::build_csr(&raw);

    // Capture the panic via catch_unwind so we can pattern-match the
    // message body — proving the AC's "clear error" wording.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        groups::assign_groups(&raw, &built, 100);
    }));

    let payload = result.expect_err("expected panic from empty Suez group");
    let msg: String = if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        panic!("panic payload was neither &str nor String");
    };

    assert!(
        msg.contains("edge group `suezCanal` is empty"),
        "panic missing group name; got: {msg}"
    );
    assert!(
        msg.contains("100km"),
        "panic missing resolution; got: {msg}"
    );
    assert!(
        msg.contains("upstream data drift"),
        "panic missing remediation hint; got: {msg}"
    );
    assert!(
        msg.contains("vendor/eurostat-marnet/marnet_plus_100km.gpkg"),
        "panic missing path to inspect; got: {msg}"
    );
}
