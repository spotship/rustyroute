//! Integration tests that lock in the structural invariants of
//! `.github/workflows/ci.yaml` introduced by ENG-4685.
//!
//! Why these tests exist
//! ---------------------
//! The CI workflow YAML is the only artefact of ENG-4685. Most of its
//! acceptance criteria can ONLY be verified on GitHub Actions itself
//! (a clippy violation reddening the clippy job, a Codecov bot
//! commenting on a PR, the cold-cache wall-clock <15 min). Those are
//! not in scope for local testing.
//!
//! What IS in scope here is the small set of invariants whose
//! regression is a silent drift hazard the spec explicitly calls out:
//!
//!   - **R9 — MSRV duplication.** `Cargo.toml [package].rust-version`
//!     and the `toolchain:` value in `ci.yaml`'s test-matrix MUST stay
//!     in lock-step. If someone bumps MSRV in Cargo.toml without
//!     touching the workflow, this test fails loudly.
//!   - **Acceptance-criteria enforcement mechanisms.** AC1/AC2 (clippy
//!     `-D warnings`, fmt `--check`) hinge on specific flag strings
//!     inside specific job definitions. If a future edit deletes those
//!     flags the gate becomes a no-op while CI stays green. Lock the
//!     flags in.
//!   - **Workflow structural shape.** Job names, trigger set, and the
//!     OS × toolchain matrix dimensions are part of the contract with
//!     the spec ("14 checks appear on the PR"). Lock them in.
//!
//! What is intentionally NOT tested:
//!
//!   - Codecov upload behaviour (requires GitHub).
//!   - Cold/warm-cache timing (requires GitHub runners).
//!   - features-matrix data-100km row — suspended pending ENG-4679 per
//!     the spec; the YAML carries the row as a placeholder today.
//!
//! These tests intentionally avoid pulling in a YAML parser. The file
//! is small, hand-authored, and validated by GitHub itself on push.
//! String-level assertions keep the test self-contained (no new dev-
//! dependencies — matches the convention in `tests/vendored_data.rs`).
//!
//! Acceptance-criteria mapping (see spec at
//! `.ship/tasks/eng-4685-.../plan/spec.md`):
//!   AC1 clippy gates `unused_variable`     -> `clippy_job_uses_deny_warnings`
//!   AC2 fmt gates whitespace drift         -> `fmt_job_uses_check_flag`
//!   AC3 features-matrix gates feature use  -> `features_matrix_has_no_default_features_row`
//!                                             (data-100km row suspended pending ENG-4679)
//!   AC4 Codecov upload on PRs              -> `coverage_job_uploads_to_codecov`
//!   AC5/AC6 cold/warm-cache budget         -> not testable locally
//!   R9   MSRV stays in sync                -> `msrv_in_workflow_matches_cargo_toml`
//!
//! Structural invariants (not tied to a numbered AC but part of the
//! design):
//!   triggers include PR + push:main + dispatch -> `workflow_triggers_pr_push_main_and_dispatch`
//!   concurrency cancel-in-progress is on       -> `workflow_has_cancel_in_progress_concurrency`
//!   3 OS × 2 toolchain test matrix             -> `test_matrix_covers_three_os_two_toolchains`
//!   docs job denies rustdoc warnings           -> `docs_job_denies_rustdoc_warnings`
//!   least-privilege permissions                -> `workflow_uses_least_privilege_permissions`

use std::fs;
use std::path::PathBuf;

/// Locate the workflow file relative to the crate root regardless of where
/// `cargo test` is invoked from. `CARGO_MANIFEST_DIR` is set by Cargo to
/// the crate root for integration tests.
fn workflow_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yaml")
}

fn cargo_toml_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

fn read_workflow() -> String {
    let p = workflow_path();
    fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

fn read_cargo_toml() -> String {
    let p = cargo_toml_path();
    fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

/// Extract `rust-version = "X.Y.Z"` from the [package] table of Cargo.toml.
/// Returns the version string without quotes. Panics if missing — the
/// spec relies on this key existing as the single source of truth.
fn cargo_rust_version() -> String {
    let toml = read_cargo_toml();
    for line in toml.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("rust-version") {
            // Accept `rust-version = "1.93.0"` and minor whitespace variants.
            let rest = rest.trim_start_matches([' ', '\t', '=']);
            let rest = rest.trim();
            let trimmed = rest.trim_matches('"');
            return trimmed.to_string();
        }
    }
    panic!("Cargo.toml missing [package].rust-version — this is the MSRV source of truth");
}

// ---------------------------------------------------------------------------
// R9 — MSRV duplication
// ---------------------------------------------------------------------------

#[test]
fn msrv_in_workflow_matches_cargo_toml() {
    let msrv = cargo_rust_version();
    let wf = read_workflow();
    // The test-matrix MUST list the Cargo.toml MSRV as one of the toolchains.
    // We search for the quoted form to avoid matching unrelated occurrences
    // (e.g. comments mentioning the version).
    let needle = format!("\"{msrv}\"");
    assert!(
        wf.contains(&needle),
        "Cargo.toml rust-version = {msrv:?} but `.github/workflows/ci.yaml` test-matrix \
         does not list {needle} as a toolchain entry. Bump MSRV in both places."
    );
}

// ---------------------------------------------------------------------------
// AC1 — clippy gates `unused_variable` warnings
// ---------------------------------------------------------------------------

#[test]
fn clippy_job_uses_deny_warnings() {
    let wf = read_workflow();
    // Find the single `cargo clippy` invocation line and assert it
    // carries `-D warnings` on the SAME line. A loose
    // `contains("-D warnings")` would be satisfied by the unrelated
    // `RUSTDOCFLAGS: "-D warnings"` in the docs job, which would let
    // the gate quietly become a no-op.
    let clippy_lines: Vec<&str> = wf
        .lines()
        .filter(|l| l.contains("cargo clippy"))
        .collect();
    assert_eq!(
        clippy_lines.len(),
        1,
        "expected exactly one `cargo clippy` invocation in ci.yaml, found {}",
        clippy_lines.len()
    );
    let clippy = clippy_lines[0];
    assert!(
        clippy.contains("-D warnings"),
        "clippy invocation must include `-- -D warnings` on the same line — \
         without it, an `unused_variable` warning would NOT fail the job (AC1). \
         Offending line: {clippy:?}"
    );
    assert!(
        clippy.contains("--all-targets"),
        "clippy invocation must use `--all-targets` so tests + examples are linted too. \
         Offending line: {clippy:?}"
    );
    assert!(
        clippy.contains("--all-features"),
        "clippy invocation must use `--all-features` so feature-gated code is linted too. \
         Offending line: {clippy:?}"
    );
}

// ---------------------------------------------------------------------------
// AC2 — fmt gates whitespace drift
// ---------------------------------------------------------------------------

#[test]
fn fmt_job_uses_check_flag() {
    let wf = read_workflow();
    assert!(
        wf.contains("cargo fmt --all --check"),
        "fmt job must run `cargo fmt --all --check` — without `--check`, the job \
         would silently reformat files instead of failing on drift (AC2)."
    );
}

// ---------------------------------------------------------------------------
// AC3 — features-matrix exercises feature-flag combinations
// ---------------------------------------------------------------------------

#[test]
fn features_matrix_has_no_default_features_row() {
    let wf = read_workflow();
    // The `--no-default-features` build is the row that catches
    // "I forgot to #[cfg]-gate this" — the AC3 sentinel. The
    // data-100km row is suspended pending ENG-4679 per the spec.
    assert!(
        wf.contains("--no-default-features"),
        "features-matrix must include a `--no-default-features` row to enforce \
         AC3 (un-gated use of feature-gated items fails CI)."
    );
    assert!(
        wf.contains("--all-features"),
        "features-matrix must include an `--all-features` row to catch gated \
         items that fail to compile when ALL features are on at once."
    );
}

// ---------------------------------------------------------------------------
// AC4 — coverage uploads to Codecov on PRs
// ---------------------------------------------------------------------------

#[test]
fn coverage_job_uploads_to_codecov() {
    let wf = read_workflow();
    assert!(
        wf.contains("cargo-llvm-cov") || wf.contains("cargo llvm-cov"),
        "coverage job must use cargo-llvm-cov to generate LCOV (AC4)."
    );
    assert!(
        wf.contains("codecov/codecov-action"),
        "coverage job must use the official codecov action to upload (AC4)."
    );
    assert!(
        wf.contains("CODECOV_TOKEN"),
        "coverage job must pass `CODECOV_TOKEN` — public-repo uploads have been \
         rate-limited since 2024 (AC4, R6)."
    );
    assert!(
        wf.contains("lcov.info") || wf.contains("--lcov"),
        "coverage job must emit LCOV format for Codecov ingestion."
    );
}

// ---------------------------------------------------------------------------
// Structural invariants — triggers, concurrency, permissions
// ---------------------------------------------------------------------------

#[test]
fn workflow_triggers_pr_push_main_and_dispatch() {
    let wf = read_workflow();
    assert!(
        wf.contains("pull_request:"),
        "workflow must trigger on `pull_request` — this is the primary PR gate."
    );
    assert!(
        wf.contains("push:") && wf.contains("branches: [main]"),
        "workflow must trigger on `push` to `main` — release-line gate."
    );
    assert!(
        wf.contains("workflow_dispatch"),
        "workflow must allow `workflow_dispatch` for manual reruns."
    );
}

#[test]
fn workflow_has_cancel_in_progress_concurrency() {
    let wf = read_workflow();
    assert!(
        wf.contains("concurrency:"),
        "workflow must declare a concurrency group to dedupe rapid pushes."
    );
    assert!(
        wf.contains("cancel-in-progress: true"),
        "concurrency must set `cancel-in-progress: true` so fixup pushes \
         don't stack matrix runs."
    );
    assert!(
        wf.contains("github.ref"),
        "concurrency group must be keyed on `github.ref` so PR and main \
         runs don't cancel each other."
    );
}

#[test]
fn workflow_uses_least_privilege_permissions() {
    let wf = read_workflow();
    assert!(
        wf.contains("permissions:") && wf.contains("contents: read"),
        "workflow must declare least-privilege `permissions: contents: read` \
         — Codecov action does not need write."
    );
}

// ---------------------------------------------------------------------------
// Structural invariants — test matrix shape
// ---------------------------------------------------------------------------

#[test]
fn test_matrix_covers_three_os_two_toolchains() {
    let wf = read_workflow();
    for os in ["ubuntu-latest", "macos-latest", "windows-latest"] {
        assert!(
            wf.contains(os),
            "test-matrix must include `{os}` to catch OS-specific regressions \
             in the bundled SQLite build (rusqlite via `cc`)."
        );
    }
    assert!(
        wf.contains("stable"),
        "test-matrix must include the `stable` toolchain."
    );
    let msrv = cargo_rust_version();
    assert!(
        wf.contains(&format!("\"{msrv}\"")),
        "test-matrix must include the MSRV toolchain `{msrv}` from Cargo.toml."
    );
    assert!(
        wf.contains("fail-fast: false"),
        "test-matrix must set `fail-fast: false` so OS-specific failures \
         don't mask each other."
    );
}

// ---------------------------------------------------------------------------
// Structural invariants — docs job
// ---------------------------------------------------------------------------

#[test]
fn docs_job_denies_rustdoc_warnings() {
    let wf = read_workflow();
    // Find the RUSTDOCFLAGS assignment line and assert it carries
    // `-D warnings` on the SAME line. Avoids cross-contamination with
    // the clippy job's `-- -D warnings` argument.
    let rustdocflags_lines: Vec<&str> = wf
        .lines()
        .filter(|l| l.contains("RUSTDOCFLAGS"))
        .collect();
    assert_eq!(
        rustdocflags_lines.len(),
        1,
        "expected exactly one RUSTDOCFLAGS line in ci.yaml, found {}",
        rustdocflags_lines.len()
    );
    assert!(
        rustdocflags_lines[0].contains("-D warnings"),
        "docs job must set `RUSTDOCFLAGS: \"-D warnings\"` so broken \
         intra-doc links and missing-summary lints fail CI before they \
         reach docs.rs. Offending line: {:?}",
        rustdocflags_lines[0]
    );
    assert!(
        wf.contains("cargo doc"),
        "docs job must run `cargo doc`."
    );
}

// ---------------------------------------------------------------------------
// Sanity — every named job listed in the spec is present
// ---------------------------------------------------------------------------

#[test]
fn all_spec_jobs_are_declared() {
    let wf = read_workflow();
    // The spec promises six job IDs (fmt, clippy, test-matrix,
    // features-matrix, coverage, docs). Lock them in.
    for job in [
        "fmt:",
        "clippy:",
        "test-matrix:",
        "features-matrix:",
        "coverage:",
        "docs:",
    ] {
        assert!(
            wf.contains(job),
            "workflow must declare job `{}` per the ENG-4685 spec.",
            job.trim_end_matches(':')
        );
    }
}
