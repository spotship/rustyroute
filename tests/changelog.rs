//! ENG-4692 — structural invariants of CHANGELOG.md.
//!
//! CHANGELOG.md is hand-seeded now and maintained by release-plz going
//! forward (conventional-commit → Keep-a-Changelog entries). These tests
//! lock the preamble, the seeded v0.1.0 entry, and — most importantly —
//! that the newest released heading matches Cargo.toml's `version`, the
//! single-source-of-truth guard modelled on
//! tests/ci_workflow.rs:86-118's MSRV-sync test.
//!
//! Not tested locally: whether release-plz correctly appends future
//! entries (requires GitHub + real commit history).
//!
//! No parser dev-dependency (convention: tests/ci_workflow.rs:36-39).

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_changelog() -> String {
    let p = repo_root().join("CHANGELOG.md");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

/// Extract `version = "X.Y.Z"` from the [package] table of Cargo.toml,
/// the same way tests/ci_workflow.rs:86-99 extracts rust-version.
/// `strip_prefix("version")` never matches the "rust-version" line
/// (which starts with `r`), so line order is irrelevant.
fn cargo_version() -> String {
    let p = repo_root().join("Cargo.toml");
    let toml = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    for line in toml.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("version") {
            let rest = rest.trim_start_matches([' ', '\t', '=']).trim();
            return rest.trim_matches('"').to_string();
        }
    }
    panic!("Cargo.toml missing [package].version");
}

#[test]
fn changelog_exists() {
    assert!(
        repo_root().join("CHANGELOG.md").exists(),
        "CHANGELOG.md must exist at the repo root (seeded stub, then release-plz-maintained)."
    );
}

#[test]
fn changelog_has_keep_a_changelog_preamble() {
    let cl = read_changelog();
    assert!(
        cl.contains("Keep a Changelog"),
        "CHANGELOG.md must reference Keep a Changelog (the format release-plz appends into)."
    );
    assert!(
        cl.contains("Semantic Versioning"),
        "CHANGELOG.md must reference Semantic Versioning."
    );
}

#[test]
fn changelog_has_v0_1_0_entry() {
    let cl = read_changelog();
    assert!(
        cl.contains("## [0.1.0]"),
        "CHANGELOG.md must contain the seeded `## [0.1.0]` release heading."
    );
}

#[test]
fn changelog_newest_version_matches_cargo_toml() {
    let cl = read_changelog();
    let version = cargo_version();
    let needle = format!("## [{version}]");
    assert!(
        cl.contains(&needle),
        "CHANGELOG.md must contain a `{needle}` heading matching Cargo.toml \
         version = \"{version}\" — single source of truth. Bump both together."
    );
}
