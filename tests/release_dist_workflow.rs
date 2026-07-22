//! ENG-4692 — structural invariants of the cargo-dist ("dist")
//! binary-distribution layer: `dist-workspace.toml` +
//! `.github/workflows/release.yaml`.
//!
//! DORMANT-UNTIL-BINARY NOTE: rustyroute has no [[bin]] target yet — the
//! `cli` feature is a forward-declared placeholder (Cargo.toml:22-25,
//! ENG-4xxx). These tests lock the *config shape* so the layer is ready
//! to activate when the binary lands; they do NOT (and cannot) verify
//! that dist actually builds/attaches binaries — that requires GitHub
//! Actions runners and a real tag push, and is a known, accepted gap
//! (spec AC9).
//!
//! No YAML/TOML parser dev-dependency — same convention as
//! tests/ci_workflow.rs:36-39 / tests/audit_workflow.rs:22-27.
//!
//! AC mapping (spec at .ship/tasks/eng-4692-.../plan/spec.md):
//!   AC3 -> release_workflow_* tests
//!   AC4 -> dist_workspace_toml_* tests

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

fn read_dist_toml() -> String {
    read("dist-workspace.toml")
}

fn read_workflow() -> String {
    read(".github/workflows/release.yaml")
}

// --- dist-workspace.toml ---

#[test]
fn dist_workspace_toml_exists() {
    assert!(
        repo_root().join("dist-workspace.toml").exists(),
        "dist-workspace.toml must exist at the repo root (the modern dist config \
         location; [workspace.metadata.dist] is deprecated)."
    );
}

#[test]
fn dist_workspace_toml_lists_all_five_targets() {
    let toml = read_dist_toml();
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        assert!(
            toml.contains(target),
            "dist-workspace.toml [dist].targets must include `{target}`."
        );
    }
}

#[test]
fn dist_workspace_toml_lists_three_installers() {
    let toml = read_dist_toml();
    // Quoted form so `"shell"` does not match inside `"powershell"`.
    for installer in ["shell", "powershell", "homebrew"] {
        assert!(
            toml.contains(&format!("\"{installer}\"")),
            "dist-workspace.toml [dist].installers must include \"{installer}\"."
        );
    }
}

#[test]
fn dist_workspace_toml_enables_cli_feature() {
    let toml = read_dist_toml();
    assert!(
        toml.contains("\"cli\""),
        "dist-workspace.toml must build with features = [\"cli\"] so the future \
         CLI binary (Cargo.toml:22-25) compiles with all data resolutions."
    );
}

#[test]
fn dist_workspace_toml_pins_cargo_dist_version() {
    let toml = read_dist_toml();
    // Presence-only: the pinned version legitimately drifts on dist
    // upgrades, so we don't hardcode the value (same spirit as
    // ci_workflow.rs reading MSRV from Cargo.toml rather than duplicating).
    assert!(
        toml.contains("cargo-dist-version"),
        "dist-workspace.toml must pin `cargo-dist-version` so the workflow \
         installs a reproducible dist version."
    );
}

// --- release.yaml ---

#[test]
fn release_workflow_exists() {
    assert!(
        repo_root().join(".github/workflows/release.yaml").exists(),
        "the dist release workflow must exist at .github/workflows/release.yaml."
    );
}

#[test]
fn release_workflow_name_is_release() {
    let wf = read_workflow();
    assert!(
        wf.contains("name: \"release\""),
        "release.yaml must set lowercase quoted `name: \"release\"`."
    );
}

#[test]
fn release_workflow_triggers_on_version_tag_push() {
    let wf = read_workflow();
    assert!(
        wf.contains("tags:"),
        "release.yaml must trigger on a tag push (`on: push: tags:`)."
    );
    assert!(
        wf.contains("v*.*.*"),
        "release.yaml tag trigger must match the `v*.*.*` release tag pattern \
         that release-plz pushes."
    );
}

#[test]
fn release_workflow_grants_contents_write() {
    let wf = read_workflow();
    assert!(
        wf.contains("permissions:") && wf.contains("contents: write"),
        "release.yaml must declare `contents: write` — dist creates/uploads to \
         the GitHub release for the pushed tag."
    );
}

#[test]
fn release_workflow_uses_own_concurrency_namespace() {
    let wf = read_workflow();
    assert!(
        wf.contains("group: release-"),
        "release.yaml concurrency group must be release-… ."
    );
    assert!(
        !wf.contains("group: ci-")
            && !wf.contains("group: audit-")
            && !wf.contains("group: release-plz-"),
        "release.yaml must not reuse the ci-/audit-/release-plz- concurrency groups."
    );
}
