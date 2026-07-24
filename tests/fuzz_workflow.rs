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
    // Runs the load_archive quick-pass with a time bound. (A `--target`
    // flag may sit between the target name and `--`, so assert the pieces
    // rather than one contiguous substring.)
    assert!(
        w.contains("cargo fuzz run load_archive") && w.contains("-max_total_time="),
        "must run load_archive with a max_total_time bound"
    );
    // ASan requires the dynamically-linked gnu triple (musl's static libc
    // breaks the sanitizer on GitHub runners).
    assert!(
        w.contains("--target x86_64-unknown-linux-gnu"),
        "fuzz build/run must pin the gnu target for ASan compatibility"
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

// ---- OSS-Fuzz staging files (deliverables for the external submission) ----
//
// The PR to google/oss-fuzz is a human follow-up (public repo + approval
// cycle), so these lock the *repo-side* deliverables: the project files must
// stay structurally valid and in sync with the fuzz crate, so the eventual
// submission does not fail an OSS-Fuzz `check_build`.

#[test]
fn oss_fuzz_project_yaml_has_required_keys() {
    let y = read("oss-fuzz/projects/rustyroute/project.yaml");
    for key in [
        "language: rust",
        "primary_contact:",
        "main_repo:",
        "fuzzing_engines:",
        "sanitizers:",
    ] {
        assert!(y.contains(key), "project.yaml must contain `{key}`");
    }
    assert!(y.contains("libfuzzer"), "libfuzzer engine required");
    assert!(y.contains("address"), "address sanitizer required");
}

#[test]
fn oss_fuzz_dockerfile_uses_rust_base_builder() {
    let d = read("oss-fuzz/projects/rustyroute/Dockerfile");
    assert!(
        d.contains("FROM gcr.io/oss-fuzz-base/base-builder-rust"),
        "Dockerfile must build on the OSS-Fuzz Rust base image"
    );
    assert!(
        d.contains("COPY build.sh"),
        "Dockerfile must stage build.sh"
    );
}

#[test]
fn oss_fuzz_build_sh_builds_all_targets_and_seeds() {
    let b = read("oss-fuzz/projects/rustyroute/build.sh");
    assert!(
        b.starts_with("#!/bin/bash"),
        "build.sh needs a bash shebang"
    );
    assert!(
        b.contains("cargo fuzz build"),
        "must build the fuzz targets"
    );
    // Every declared fuzz target must be handled by build.sh, and vice versa —
    // guards against renaming a target in Cargo.toml but not the build script.
    let cargo = read("fuzz/Cargo.toml");
    for target in ["load_archive", "route_inputs"] {
        assert!(
            cargo.contains(&format!("name = \"{target}\"")),
            "target {target} should be declared in fuzz/Cargo.toml"
        );
        assert!(
            b.contains(target),
            "build.sh must handle the {target} target"
        );
    }
    // The seed must be delivered via OSS-Fuzz's <target>_seed_corpus.zip
    // convention; loose files copied into $OUT are ignored by OSS-Fuzz.
    assert!(
        b.contains("load_archive_seed_corpus.zip"),
        "seed must be packaged as load_archive_seed_corpus.zip"
    );
}

#[test]
fn oss_fuzz_build_sh_is_committed_executable() {
    // OSS-Fuzz invokes build.sh directly, so it must carry the executable bit.
    // Check git's tracked mode (100755) — platform-independent, unlike the
    // local filesystem bit which PermissionsExt cannot read on Windows.
    let out = Command::new("git")
        .args(["ls-files", "-s", "oss-fuzz/projects/rustyroute/build.sh"])
        .current_dir(root())
        .output();
    let out = match out {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => o,
        // Not a git checkout (e.g. a packaged crate tarball) — nothing to
        // assert against; skip rather than fail.
        _ => return,
    };
    let line = String::from_utf8_lossy(&out.stdout);
    assert!(
        line.starts_with("100755"),
        "build.sh must be committed executable (git mode 100755), got: {line}"
    );
}
