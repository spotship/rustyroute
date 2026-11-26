//! ENG-4692 — structural invariants of the cargo-dist ("dist")
//! binary-distribution layer: `dist-workspace.toml` +
//! `.github/workflows/release.yaml`.
//!
//! BINARY-LANDED NOTE: the `rustyroute` [[bin]] target now exists
//! (src/bin/rustyroute.rs, gated on the `cli` feature — ENG-4682).
//! These tests lock the *config shape* only; they do NOT (and cannot)
//! verify that dist actually builds/attaches binaries — that requires
//! GitHub Actions runners and a real tag push, and is a known, accepted
//! gap (spec AC9).
//!
//! No YAML/TOML parser dev-dependency — same convention as
//! tests/ci_workflow.rs:36-39 / tests/audit_workflow.rs:22-27.
//!
//! AC mapping (spec at .ship/tasks/eng-4692-.../plan/spec.md):
//!   AC3 -> `release_workflow`_* tests
//!   AC4 -> `dist_workspace_toml`_* tests

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
    // Presence-only on the VALUE (not the version number, which legitimately
    // drifts — same spirit as ci_workflow.rs reading MSRV from Cargo.toml).
    // Match the actual key assignment line, not any occurrence: the header
    // comment also says "re-pin `cargo-dist-version`", so a bare `contains`
    // would pass even if the real key were deleted.
    assert!(
        toml.lines().any(|l| {
            let t = l.trim();
            t.starts_with("cargo-dist-version") && t.contains('=')
        }),
        "dist-workspace.toml must set `cargo-dist-version = \"…\"` as an actual key \
         (not just a comment mention) so the workflow installs a reproducible dist version."
    );
}

#[test]
fn release_workflow_dist_version_matches_config() {
    // Single-source-of-truth cross-file guard (same pattern as
    // ci_workflow.rs:86-118's MSRV-sync test): the `cargo install
    // cargo-dist --version X` step in release.yaml MUST install the exact
    // version dist-workspace.toml was authored against. A bump to one file
    // without the other silently desyncs the CI-installed dist from its
    // config, with nothing else catching it.
    let toml = read_dist_toml();
    let line = toml
        .lines()
        .find_map(|l| l.trim().strip_prefix("cargo-dist-version"))
        .expect("dist-workspace.toml must set cargo-dist-version");
    let version = line
        .trim_start_matches([' ', '\t', '='])
        .trim()
        .trim_matches('"');
    let wf = read_workflow();
    assert!(
        wf.contains(&format!("--version {version}")),
        "release.yaml's `cargo install cargo-dist --version` must match \
         dist-workspace.toml cargo-dist-version = \"{version}\" — keep the \
         installed dist version in lock-step with the config it targets."
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
    // Match the actual permission entry as a standalone (trimmed) line, not
    // any occurrence — the header comment references `contents: read`/`write`
    // in prose, so a bare `contains` would pass even if the permissions:
    // block were removed (same pattern as release_plz_config.rs:202-223).
    assert!(
        wf.contains("permissions:"),
        "release.yaml must declare a permissions: block."
    );
    assert!(
        wf.lines().any(|l| l.trim() == "contents: write"),
        "release.yaml permissions: must grant `contents: write` as an actual entry \
         (not just a comment) — dist creates/uploads to the GitHub release for the \
         pushed tag."
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
