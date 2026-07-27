//! Integration tests that lock in the structural invariants of
//! `.github/workflows/codspeed.yaml` introduced by ENG-4690.
//!
//! CodSpeed's actual measurement + PR-comment behaviour can only be
//! verified on GitHub Actions against the org's CodSpeed app. What IS in
//! scope here is the small set of string-level invariants whose silent
//! regression would neuter the perf gate: the triggers, the OIDC auth
//! wiring, the CodSpeed action pin, the cargo-codspeed build/run steps,
//! and the [skip-perf] escape hatch. String assertions match the
//! convention in tests/ci_workflow.rs and tests/audit_workflow.rs.

use std::fs;
use std::path::PathBuf;

fn workflow_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/codspeed.yaml")
}

fn read_workflow() -> String {
    let p = workflow_path();
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

#[test]
fn codspeed_workflow_exists() {
    assert!(
        workflow_path().exists(),
        "codspeed.yaml must exist at .github/workflows/codspeed.yaml."
    );
}

#[test]
fn codspeed_workflow_triggers_pr_and_push_main() {
    let wf = read_workflow();
    assert!(
        wf.contains("pull_request:"),
        "codspeed.yaml must trigger on `pull_request` — the PR perf-delta gate."
    );
    assert!(
        wf.contains("push:") && wf.contains("branches: [main]"),
        "codspeed.yaml must trigger on push to `main` — the baseline CodSpeed uses for deltas."
    );
}

#[test]
fn codspeed_workflow_uses_codspeed_action_v4() {
    let wf = read_workflow();
    assert!(
        wf.contains("CodSpeedHQ/action@v4"),
        "codspeed.yaml must pin CodSpeedHQ/action@v4 (floating major) — v4 \
         is the first major with OpenID Connect upload auth."
    );
}

#[test]
fn codspeed_workflow_authenticates_via_oidc() {
    let wf = read_workflow();
    assert!(
        wf.contains("id-token: write"),
        "codspeed.yaml must grant `id-token: write` — CodSpeed uploads \
         authenticate via OpenID Connect against the org's CodSpeed GitHub \
         App. Without it the upload fails 401 and no PR comment is posted."
    );
    // Matches the wiring (`token: ${{ secrets.CODSPEED_TOKEN }}`), not the
    // bare name — the workflow header cites CODSPEED_TOKEN when explaining
    // why the v2 revision 401'd, and that prose should stay allowed.
    assert!(
        !wf.contains("secrets.CODSPEED_TOKEN"),
        "codspeed.yaml must NOT wire in a CODSPEED_TOKEN secret — no such \
         secret is provisioned, and passing an empty one makes the upload \
         fail 401 'Repository not found'. OIDC replaces it."
    );
}

#[test]
fn codspeed_workflow_builds_and_runs_benches() {
    let wf = read_workflow();
    assert!(
        wf.contains("cargo codspeed build"),
        "codspeed.yaml must run `cargo codspeed build` to compile the \
         instrumented benches before running them."
    );
    assert!(
        wf.contains("cargo codspeed run"),
        "codspeed.yaml must run `cargo codspeed run` (via the CodSpeed \
         action) to execute the instrumented benches."
    );
    assert!(
        wf.contains("tool: cargo-codspeed"),
        "codspeed.yaml must install cargo-codspeed via \
         taiki-e/install-action (matches the repo's install convention)."
    );
}

#[test]
fn codspeed_workflow_honours_skip_perf_title() {
    let wf = read_workflow();
    assert!(
        wf.contains("[skip-perf]"),
        "codspeed.yaml must guard the perf job with a `[skip-perf]` \
         PR-title check so PRs that intentionally trade perf for clarity \
         can bypass the gate (ticket note; document the regression in \
         CHANGELOG per ENG-4692)."
    );
}

#[test]
fn codspeed_workflow_uses_least_privilege_permissions() {
    let wf = read_workflow();
    assert!(
        wf.contains("permissions:") && wf.contains("contents: read"),
        "codspeed.yaml must declare least-privilege `permissions: \
         contents: read` — mirrors ci.yaml/audit.yaml."
    );
}
