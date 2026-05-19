//! ENG-4679 E2E: exercise the documented operator workflow for the
//! `$RUSTYROUTE_DATA_DIR` env-var override end-to-end.
//!
//! Resolution-order step 1 (env-var override) is the deployment story
//! the spec calls out ("routefinder-style deployments override via
//! `$RUSTYROUTE_DATA_DIR` so the binary stays small"). The dev phase
//! covers the negative case (`DataFileMissing` when the configured
//! directory has no `{N}km.rkyv`) inside the loader's `#[cfg(test)]`
//! module, but the happy path — pointing the env var at a directory
//! that actually contains the archive and seeing `Graph::load` succeed
//! against it — was not exercised at the integration-test surface.
//!
//! This test:
//! 1. Copies the in-tree `$OUT_DIR/data/50km.rkyv` archive into a
//!    fresh tempdir.
//! 2. Sets `$RUSTYROUTE_DATA_DIR` to that tempdir.
//! 3. Calls `Graph::load(50)` and asserts the resulting handle reports
//!    the requested resolution and non-zero counts that match
//!    `Graph::from_bytes(BYTES_50KM)` for byte-identical archives.
//! 4. Removes the env var so it does not contaminate other tests.
//!
//! Both tests in this binary mutate the same `RUSTYROUTE_DATA_DIR`
//! env var. libtest defaults to running tests in parallel, so they
//! are serialized via an `ENV_LOCK` mutex below. The guard provides
//! the single-threaded mutation that Rust 2024's `unsafe`
//! `set_var`/`remove_var` require for soundness.

#![cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use rustyroute::{Graph, LoadError};

/// Serializes env-mutating tests in this binary. See the file-level
/// comment for why this is required.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Locate the in-tree `50km.rkyv` archive produced by `build.rs`. The
/// `data` module's `BYTES_50KM` const is `include_bytes!`-baked from
/// `$OUT_DIR/data/50km.rkyv` at compile time; we mirror that lookup at
/// runtime by reading the same path via `env!("OUT_DIR")`. `OUT_DIR`
/// is always set by cargo when compiling an integration test, so the
/// `env!` macro form (compile-time panic if absent) is safe here.
fn in_tree_50km_path() -> PathBuf {
    PathBuf::from(env!("OUT_DIR")).join("data/50km.rkyv")
}

#[test]
fn load_50km_from_env_data_dir_happy_path() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Stage a fresh tempdir with a copy of the 50km archive.
    let tmp = tempfile::tempdir().expect("create tempdir");
    let src = in_tree_50km_path();
    let dst = tmp.path().join("50km.rkyv");
    fs::copy(&src, &dst)
        .unwrap_or_else(|e| panic!("copy {} -> {}: {e}", src.display(), dst.display()));

    // Point the env var at the staged directory and invoke load.
    // SAFETY: the ENV_LOCK guard above ensures this test is the only
    // thread mutating env state for its duration, satisfying Rust
    // 2024's single-threaded-mutation requirement for `set_var`/
    // `remove_var`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("RUSTYROUTE_DATA_DIR", tmp.path());
    }

    let result = Graph::load(50);

    // Clean up env state BEFORE any panicking assertion so a failing
    // assertion doesn't leak state into adjacent test binaries that
    // cargo may schedule next.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("RUSTYROUTE_DATA_DIR");
    }

    let g = result.expect("Graph::load(50) via RUSTYROUTE_DATA_DIR");
    assert_eq!(g.resolution_km(), 50, "resolution_km should be 50");
    assert!(g.node_count() > 0, "node_count must be > 0");
    assert!(g.edge_count() > 0, "edge_count must be > 0");
    assert!(
        g.directed_edge_count() >= g.edge_count(),
        "directed_edge_count must be >= edge_count"
    );

    // Cross-check against BYTES_50KM: same archive, same counts.
    let static_g =
        Graph::from_bytes(rustyroute::data::BYTES_50KM).expect("Graph::from_bytes(BYTES_50KM)");
    assert_eq!(
        g.node_count(),
        static_g.node_count(),
        "mmap-via-env and static-bytes node counts must agree"
    );
    assert_eq!(
        g.edge_count(),
        static_g.edge_count(),
        "mmap-via-env and static-bytes edge counts must agree"
    );
    assert_eq!(
        g.directed_edge_count(),
        static_g.directed_edge_count(),
        "mmap-via-env and static-bytes directed_edge counts must agree"
    );
}

#[test]
fn load_50km_from_env_data_dir_missing_file_reports_path() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Empty tempdir — env var points at it but the file is absent.
    let tmp = tempfile::tempdir().expect("create tempdir");

    // SAFETY: the ENV_LOCK guard above ensures this test is the only
    // thread mutating env state for its duration, satisfying Rust
    // 2024's single-threaded-mutation requirement for `set_var`/
    // `remove_var`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("RUSTYROUTE_DATA_DIR", tmp.path());
    }

    let result = Graph::load(50);

    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("RUSTYROUTE_DATA_DIR");
    }

    match result {
        Err(LoadError::DataFileMissing(p)) => {
            assert!(
                p.ends_with("50km.rkyv"),
                "missing-file path should end with 50km.rkyv, got {}",
                p.display()
            );
            assert!(
                p.starts_with(tmp.path()),
                "missing-file path should be inside the env-var directory: {}",
                p.display()
            );
        }
        other => panic!("expected DataFileMissing, got {other:?}"),
    }
}
