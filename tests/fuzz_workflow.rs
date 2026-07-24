//! ENG-4691: lock in the structural invariants of the fuzz setup.
//!
//! Like `tests/ci_workflow.rs`, these are string-level assertions (no YAML
//! parser) plus one spawned `cargo metadata` check. What can only be verified
//! on GitHub Actions itself (the actual libFuzzer run finding no crash) is out
//! of scope here — these guard against silent local drift:
//!
//!   - the workflow exists with the nightly + cargo-fuzz shape it needs,
//!   - the fuzz crate is its own workspace root with both targets declared,
//!   - the committed seed is a real, valid archive,
//!   - and — the load-bearing acceptance invariant — the root build never
//!     picks up the fuzz package as a member.
//!
//! Convention notes: string assertions mirror `tests/ci_workflow.rs`; the
//! spawned-cargo pattern (using the `CARGO`/`CARGO_MANIFEST_DIR` env vars
//! Cargo sets for integration tests) mirrors
//! `tests/downstream_consumer_smoke.rs`. No new dev-dependencies.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Crate root, regardless of where `cargo test` is invoked from.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let p = root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

// ---- fuzz.yaml workflow invariants ----

#[test]
fn fuzz_workflow_exists_and_is_named_fuzz() {
    let w = read(".github/workflows/fuzz.yaml");
    assert!(w.contains("name: \"fuzz\""), "workflow must be named fuzz");
}

#[test]
fn fuzz_workflow_uses_nightly_and_installs_cargo_fuzz() {
    let w = read(".github/workflows/fuzz.yaml");
    assert!(
        w.contains("dtolnay/rust-toolchain@nightly"),
        "cargo-fuzz needs a nightly sanitizer toolchain"
    );
    assert!(w.contains("tool: cargo-fuzz"), "must install cargo-fuzz");
}

#[test]
fn fuzz_workflow_builds_both_and_runs_load_archive() {
    let w = read(".github/workflows/fuzz.yaml");
    // Builds all targets (catches a route_inputs compile break).
    assert!(w.contains("cargo fuzz build"), "must build all targets");
    // Runs the load_archive quick-pass with a time bound.
    assert!(
        w.contains("cargo fuzz run load_archive -- -max_total_time="),
        "must run load_archive with a max_total_time bound"
    );
}

#[test]
fn fuzz_workflow_triggers_and_least_privilege() {
    let w = read(".github/workflows/fuzz.yaml");
    assert!(w.contains("pull_request:"), "runs on pull_request");
    assert!(w.contains("workflow_dispatch:"), "supports manual dispatch");
    assert!(
        w.contains("cancel-in-progress: true"),
        "concurrency cancels superseded runs"
    );
    assert!(
        w.contains("permissions:") && w.contains("contents: read"),
        "least-privilege permissions"
    );
}

// ---- fuzz crate structural invariants ----

#[test]
fn fuzz_crate_is_its_own_workspace_root() {
    let c = read("fuzz/Cargo.toml");
    assert!(
        c.contains("[workspace]"),
        "fuzz must declare its own [workspace] so it stays excluded from any \
         future root workspace"
    );
    assert!(
        c.contains("cargo-fuzz = true"),
        "cargo-fuzz metadata marker"
    );
}

#[test]
fn fuzz_targets_declared() {
    let c = read("fuzz/Cargo.toml");
    assert!(c.contains("name = \"load_archive\""), "load_archive target");
    assert!(c.contains("name = \"route_inputs\""), "route_inputs target");
}

#[test]
fn seed_corpus_present_and_valid_header() {
    let p = root().join("fuzz/corpus/load_archive/seed_100km.rkyv");
    let bytes = fs::read(&p).unwrap_or_else(|e| panic!("failed to read seed {}: {e}", p.display()));
    assert!(bytes.len() >= 8, "seed too short to carry a header");
    assert_eq!(&bytes[0..4], b"RRG1", "seed magic must be RRG1");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().expect("4-byte slice")),
        1,
        "seed schema version must be 1"
    );
}

// ---- ACCEPTANCE: root build must NOT pick up the fuzz package ----

#[test]
fn root_metadata_excludes_fuzz_package() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(&cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root())
        .output()
        .expect("spawn cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = String::from_utf8_lossy(&out.stdout);
    assert!(
        !json.contains("rustyroute-fuzz"),
        "root `cargo metadata` must NOT list the fuzz package as a workspace member"
    );
}
