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

/// True when `wf` pins `action` to `version`, accepting both the floating
/// tag (`owner/action@v0.5`) and the digest form Renovate rewrites it into
/// (`owner/action@<40-hex> # v0.5`) — `renovate.json` extends
/// `helpers:pinGitHubActionDigests`. Branch refs, bare digests, and
/// digests annotated with a different version are still rejected.
///
/// Deliberately duplicated from `tests/audit_workflow.rs` (which carries
/// the long rationale): each workflow test file in this repo is
/// self-contained and keeps its own copy of its small string helpers.
fn pins_action_at(wf: &str, action: &str, version: &str) -> bool {
    let needle = format!("{action}@");
    wf.lines().any(|line| {
        let Some((_, rest)) = line.split_once(&needle) else {
            return false;
        };
        let (git_ref, comment) = match rest.split_once('#') {
            Some((r, c)) => (r.trim(), Some(c.trim())),
            None => (rest.trim(), None),
        };
        if git_ref == version {
            return true;
        }
        git_ref.len() == 40
            && git_ref.chars().all(|c| c.is_ascii_hexdigit())
            && comment == Some(version)
    })
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
        pins_action_at(&wf, "MarcoIeni/release-plz-action", "v0.5"),
        "release-plz.yaml must pin MarcoIeni/release-plz-action to v0.5 — \
         as `@v0.5` or as `@<sha> # v0.5`."
    );
}

#[test]
fn release_plz_workflow_references_both_tokens() {
    let wf = read_workflow();
    // Assert the actual `env:` mappings, not any occurrence — the header
    // comments also mention these token names, so a bare `contains` would
    // pass even if the env wiring were deleted (same windowing discipline
    // as audit_workflow.rs:326-345 scoping to a block).
    assert!(
        wf.lines().any(|l| {
            let t = l.trim();
            t.starts_with("GITHUB_TOKEN:") && t.contains("secrets.")
        }),
        "release-plz.yaml must wire GITHUB_TOKEN from a secret in the action's \
         env: mapping (not merely mention it in a comment)."
    );
    assert!(
        wf.lines().any(|l| {
            let t = l.trim();
            t.starts_with("CARGO_REGISTRY_TOKEN:") && t.contains("secrets.CARGO_REGISTRY_TOKEN")
        }),
        "release-plz.yaml must wire CARGO_REGISTRY_TOKEN from secrets in the env: mapping \
         (crates.io publish)."
    );
}

#[test]
fn release_plz_workflow_prefers_pat_for_downstream_triggers() {
    let wf = read_workflow();
    // A ref created with the default GITHUB_TOKEN does not trigger other
    // workflows (GitHub recursion guard), so the vX.Y.Z tag must be
    // pushed with a PAT / GitHub App token (RELEASE_PLZ_TOKEN) for
    // release.yaml to fire. Lock in that the workflow reaches for that
    // secret (with a documented GITHUB_TOKEN fallback) rather than the
    // default token alone.
    // Assert the real env mapping sources from secrets.RELEASE_PLZ_TOKEN,
    // not just that the name appears somewhere (it also appears in comments).
    assert!(
        wf.lines().any(|l| {
            let t = l.trim();
            t.starts_with("GITHUB_TOKEN:") && t.contains("secrets.RELEASE_PLZ_TOKEN")
        }),
        "release-plz.yaml must source the action token from secrets.RELEASE_PLZ_TOKEN \
         in the env: mapping (not merely a comment) so the pushed vX.Y.Z tag triggers \
         release.yaml — the default GITHUB_TOKEN alone cannot."
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
    // Match the actual permission entries as standalone (trimmed) lines,
    // not any occurrence — the header comment also prints "(contents: write)"
    // and "(pull-requests: write)" in prose, so a bare `contains` would pass
    // even if the permissions: block were removed.
    assert!(
        wf.contains("permissions:"),
        "release-plz.yaml must declare a permissions: block."
    );
    assert!(
        wf.lines().any(|l| l.trim() == "contents: write"),
        "release-plz.yaml permissions: must grant `contents: write` as an actual entry \
         (not just a comment) — the deliberate exception to the contents: read norm \
         (it pushes tags / creates the GH release)."
    );
    assert!(
        wf.lines().any(|l| l.trim() == "pull-requests: write"),
        "release-plz.yaml permissions: must grant `pull-requests: write` as an actual entry \
         — release-plz needs it to open/update the release PR (without it, PR creation 403s)."
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
