//! ENG-4692 — structural invariants of the release-plz automation:
//! `release-plz.toml` + `.github/workflows/release-plz.yaml`.
//!
//! Why these tests exist
//! ---------------------
//! Like ci.yaml/audit.yaml (tests/ci_workflow.rs:1-57,
//! tests/audit_workflow.rs:1-43), the real behaviour of this automation
//! can ONLY be verified on GitHub Actions with real secrets and a real
//! conventional-commit history. What IS in scope here is the small set
//! of string-level invariants whose silent regression would break the
//! release loop while leaving CI green:
//!   - PR title uses Tera `{{ version }}` (a `{version}` typo silently
//!     no-ops the template and ships a literal-string PR title);
//!   - the workflow runs only on push-to-main (never on PRs), pins the
//!     action, carries both required tokens, fetches full history,
//!     grants the `contents: write` exception, and uses its own
//!     concurrency namespace (not ci-/audit-).
//!
//! What is intentionally NOT tested (requires GitHub Actions):
//!   - Whether release-plz actually opens a correct release PR.
//!   - Whether the crate actually publishes to crates.io.
//!   - cargo-dist binary attachment — a separate, currently-dormant gap
//!     (no [[bin]] target yet; see Cargo.toml:22-25 and
//!     tests/release_dist_workflow.rs).
//!
//! No YAML/TOML parser dev-dependency — matches the convention in
//! tests/ci_workflow.rs:36-39 and tests/audit_workflow.rs:22-27.
//!
//! AC mapping (spec at .ship/tasks/eng-4692-.../plan/spec.md):
//!   AC1 -> release_plz_toml_* tests
//!   AC2 -> release_plz_workflow_* tests

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

fn read_toml() -> String {
    read("release-plz.toml")
}

fn read_workflow() -> String {
    read(".github/workflows/release-plz.yaml")
}

// --- release-plz.toml ---

#[test]
fn release_plz_toml_exists_at_repo_root() {
    assert!(
        repo_root().join("release-plz.toml").exists(),
        "release-plz.toml must exist at the repo root — the release-plz config source of truth."
    );
}

#[test]
fn release_plz_toml_declares_rustyroute_package() {
    let toml = read_toml();
    // Scope to the [[package]] block so an unrelated `name` key can't
    // satisfy this (same windowing pattern as audit_workflow.rs:387-409).
    let pkg = toml
        .find("[[package]]")
        .map(|i| &toml[i..])
        .expect("release-plz.toml must contain a [[package]] table");
    assert!(
        pkg.contains("name = \"rustyroute\""),
        "release-plz.toml [[package]] must set name = \"rustyroute\"."
    );
}

#[test]
fn release_plz_toml_enables_git_release() {
    let toml = read_toml();
    assert!(
        toml.contains("git_release_enable = true"),
        "release-plz.toml must set git_release_enable = true — creating the GH \
         release is what pushes the vX.Y.Z tag that release.yaml keys on."
    );
}

#[test]
fn release_plz_toml_pr_name_uses_tera_syntax() {
    let toml = read_toml();
    // Exact match: release-plz uses Tera templating, so the title must be
    // "v{{ version }}" (double brace). A single-brace "v{version}" would
    // ship literally in the PR title. Lock the exact string in.
    assert!(
        toml.contains("pr_name = \"chore: release v{{ version }}\""),
        "release-plz.toml [workspace].pr_name must be exactly \
         `chore: release v{{{{ version }}}}` (Tera double-brace syntax)."
    );
}

// --- release-plz.yaml ---

#[test]
fn release_plz_workflow_exists() {
    assert!(
        repo_root()
            .join(".github/workflows/release-plz.yaml")
            .exists(),
        "the release-plz workflow must exist at .github/workflows/release-plz.yaml."
    );
}

#[test]
fn release_plz_workflow_name_is_release_plz() {
    let wf = read_workflow();
    assert!(
        wf.contains("name: \"release-plz\""),
        "release-plz.yaml must set lowercase quoted `name: \"release-plz\"`."
    );
}

#[test]
fn release_plz_workflow_triggers_only_on_push_to_main() {
    let wf = read_workflow();
    assert!(
        wf.contains("push:") && wf.contains("branches: [main]"),
        "release-plz.yaml must trigger on push to main."
    );
    assert!(
        !wf.contains("pull_request"),
        "release-plz.yaml must NOT trigger on pull_request — release automation \
         must not run speculatively on PRs."
    );
}

#[test]
fn release_plz_workflow_uses_action_v0_5() {
    let wf = read_workflow();
    assert!(
        wf.contains("MarcoIeni/release-plz-action@v0.5"),
        "release-plz.yaml must pin MarcoIeni/release-plz-action@v0.5."
    );
}

#[test]
fn release_plz_workflow_references_both_tokens() {
    let wf = read_workflow();
    assert!(
        wf.contains("GITHUB_TOKEN"),
        "release-plz.yaml must pass GITHUB_TOKEN to the action."
    );
    assert!(
        wf.contains("CARGO_REGISTRY_TOKEN"),
        "release-plz.yaml must pass CARGO_REGISTRY_TOKEN (crates.io publish) to the action."
    );
}

#[test]
fn release_plz_workflow_checkout_uses_full_history() {
    let wf = read_workflow();
    assert!(
        wf.contains("fetch-depth: 0"),
        "release-plz.yaml checkout must use fetch-depth: 0 — release-plz walks \
         conventional-commit history back to the last release tag."
    );
}

#[test]
fn release_plz_workflow_grants_contents_write() {
    let wf = read_workflow();
    assert!(
        wf.contains("permissions:") && wf.contains("contents: write"),
        "release-plz.yaml must declare `contents: write` — the deliberate \
         exception to the repo-wide contents: read norm (it pushes tags / \
         creates the GH release)."
    );
}

#[test]
fn release_plz_workflow_uses_own_concurrency_namespace() {
    let wf = read_workflow();
    assert!(
        wf.contains("group: release-plz-"),
        "release-plz.yaml concurrency group must be release-plz-… ."
    );
    assert!(
        !wf.contains("group: ci-") && !wf.contains("group: audit-"),
        "release-plz.yaml must not reuse the ci-/audit- concurrency groups."
    );
}
