//! Integration tests that lock in the structural invariants of
//! `.github/workflows/codspeed.yaml` introduced by ENG-4690.
//!
//! CodSpeed's actual measurement + PR-comment behaviour can only be
//! verified on GitHub Actions with a live CODSPEED_TOKEN. What IS in
//! scope here is the small set of string-level invariants whose silent
//! regression would neuter the perf gate: the triggers, the token
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
fn codspeed_workflow_uses_codspeed_action_v2() {
    let wf = read_workflow();
    assert!(
        wf.contains("CodSpeedHQ/action@v2"),
        "codspeed.yaml must pin CodSpeedHQ/action@v2 (floating major)."
    );
}

#[test]
fn codspeed_workflow_passes_codspeed_token() {
    let wf = read_workflow();
    assert!(
        wf.contains("secrets.CODSPEED_TOKEN"),
        "codspeed.yaml must pass `token: ${{ secrets.CODSPEED_TOKEN }}` — \
         without it the CodSpeed upload is unauthenticated and no PR \
         comment is posted."
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
