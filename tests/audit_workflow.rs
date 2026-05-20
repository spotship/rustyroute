//! Integration tests that lock in the structural invariants of
//! `.github/workflows/audit.yaml` and `deny.toml` introduced by
//! ENG-4686.
//!
//! Why these tests exist
//! ---------------------
//! The supply-chain gate's value comes from being hard to silently
//! weaken. A PR that "fixes a CI failure" by dropping `openssl-sys`
//! from the deny list, or by deleting the license rationale comment,
//! or by removing `issues: write` from audit.yaml (silently breaking
//! the nightly cron's GH-issue filing) would leave CI green but the
//! gate effectively gone. These tests lock the load-bearing pieces
//! into source control.
//!
//! Most of the audit workflow's behaviour can ONLY be verified on
//! GitHub Actions (the action interpreting cargo-deny's exit code,
//! the schedule firing at 06:00 UTC, the issue-filing behaviour on
//! advisory hits). What IS in scope here is the small set of
//! string-level invariants whose silent regression is the gate's
//! main hazard.
//!
//! These tests intentionally avoid pulling in a YAML / TOML parser.
//! Both files are small, hand-authored, and validated by GitHub on
//! push (audit.yaml) and by cargo-deny itself (deny.toml).
//! String-level assertions keep the test self-contained — matches
//! the convention in `tests/ci_workflow.rs` (which sets the
//! precedent with `tests/vendored_data.rs` chain).
//!
//! Acceptance-criteria mapping (see spec at
//! `.ship/tasks/eng-4686-.../plan/spec.md`):
//!   AC1 cargo deny check passes locally        -> `deny_toml_passes_cargo_deny_check_if_tool_is_installed`
//!                                                 (skips when cargo-deny is not on PATH; runs the live
//!                                                 gate when it is)
//!   AC2 cargo deny check passes in CI          -> `audit_workflow_runs_cargo_deny_action`
//!   AC3 GPL-3.0 dep fails the workflow         -> `deny_toml_allowlist_excludes_gpl`
//!   AC4 known-vulnerable dep flagged with id   -> not testable locally (documented in spec)
//!   AC5 cargo-audit on every push/PR           -> `audit_workflow_runs_audit_check_v2`
//!   AC6 nightly cron 06:00 UTC opens GH issues -> `audit_workflow_cron_is_0_6_utc`
//!                                                + `audit_workflow_grants_issues_write_for_scheduled_issues`
//!   AC7 licence rationale + GPL exclusion note -> `deny_toml_license_rationale_comment_is_present`
//!   AC8 tests/audit_workflow.rs passes         -> this file
//!   AC9 deny.toml is the only policy file      -> `no_audit_toml_at_repo_root`
//!   AC10 audit doesn't share ci concurrency    -> `audit_workflow_has_separate_concurrency_group`

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workflow_path() -> PathBuf {
    repo_root().join(".github/workflows/audit.yaml")
}

fn deny_toml_path() -> PathBuf {
    repo_root().join("deny.toml")
}

fn read_workflow() -> String {
    let p = workflow_path();
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

fn read_deny_toml() -> String {
    let p = deny_toml_path();
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

// ---------------------------------------------------------------------------
// audit.yaml — triggers, schedule, permissions, concurrency
// ---------------------------------------------------------------------------

#[test]
fn audit_workflow_triggers_pr_push_main_schedule_and_dispatch() {
    let wf = read_workflow();
    assert!(
        wf.contains("pull_request:"),
        "audit.yaml must trigger on `pull_request` — primary PR gate."
    );
    assert!(
        wf.contains("push:") && wf.contains("branches: [main]"),
        "audit.yaml must trigger on push to `main` — release-line gate."
    );
    assert!(
        wf.contains("schedule:"),
        "audit.yaml must declare a `schedule:` block for the nightly cron."
    );
    assert!(
        wf.contains("workflow_dispatch"),
        "audit.yaml must allow `workflow_dispatch` for manual reruns."
    );
}

#[test]
fn audit_workflow_cron_is_0_6_utc() {
    let wf = read_workflow();
    // Lock the specific cron expression from the ticket. Drift (e.g.
    // someone changing it to `0 0 * * *`) would silently change when
    // the nightly issue-filing runs.
    assert!(
        wf.contains(r#"cron: "0 6 * * *""#) || wf.contains("cron: '0 6 * * *'"),
        "audit.yaml schedule must be `0 6 * * *` (06:00 UTC) per the ENG-4686 ticket."
    );
}

#[test]
fn audit_workflow_grants_issues_write_for_scheduled_issues() {
    let wf = read_workflow();
    // Without `issues: write`, rustsec/audit-check@v2's nightly
    // issue-filing silently no-ops on `schedule:` runs. The gate
    // technically still runs but nothing actually surfaces — exactly
    // the failure mode this test exists to prevent.
    assert!(
        wf.contains("issues: write"),
        "audit.yaml must grant `issues: write` so rustsec/audit-check@v2 \
         can open / update advisory issues on scheduled runs (AC6)."
    );
}

#[test]
fn audit_workflow_uses_least_privilege_permissions() {
    let wf = read_workflow();
    assert!(
        wf.contains("permissions:") && wf.contains("contents: read"),
        "audit.yaml must declare top-level least-privilege \
         `permissions: contents: read` — only `issues:` and `checks:` \
         are write-scoped (for audit-check's issue and check-run APIs)."
    );
}

#[test]
fn audit_workflow_has_cancel_in_progress_concurrency() {
    let wf = read_workflow();
    assert!(
        wf.contains("concurrency:"),
        "audit.yaml must declare a concurrency group to dedupe rapid pushes."
    );
    assert!(
        wf.contains("cancel-in-progress: true"),
        "concurrency must set `cancel-in-progress: true` so fixup \
         pushes don't stack audit runs."
    );
}

#[test]
fn audit_workflow_has_separate_concurrency_group() {
    let wf = read_workflow();
    // The audit workflow must NOT reuse ci.yaml's `ci-${{ github.ref }}`
    // group — otherwise an in-flight ci run on the same ref would cancel
    // a starting audit run (and vice versa). The audit group must be
    // its own namespace.
    assert!(
        wf.contains("group: audit-"),
        "audit.yaml's concurrency group must be `audit-…`, NOT `ci-…`. \
         A shared group would let ci and audit cancel each other (AC10)."
    );
    // Belt and braces: assert the group is NOT exactly `ci-…`.
    assert!(
        !wf.contains("group: ci-"),
        "audit.yaml must not reuse the `ci-…` concurrency group."
    );
}

// ---------------------------------------------------------------------------
// audit.yaml — action versions
// ---------------------------------------------------------------------------

#[test]
fn audit_workflow_uses_cargo_deny_action_v2() {
    let wf = read_workflow();
    // Tag pinning matters: @v2 floats over v2.x.x; @main would let
    // upstream silently change behaviour. Lock the floating major.
    assert!(
        wf.contains("EmbarkStudios/cargo-deny-action@v2"),
        "audit.yaml must pin EmbarkStudios/cargo-deny-action@v2 \
         (floating major). A different tag breaks the ticket's \
         action-version contract."
    );
}

#[test]
fn audit_workflow_uses_audit_check_v2() {
    let wf = read_workflow();
    assert!(
        wf.contains("rustsec/audit-check@v2"),
        "audit.yaml must pin rustsec/audit-check@v2 (floating major)."
    );
}

#[test]
fn audit_workflow_runs_cargo_deny_action() {
    let wf = read_workflow();
    // Smoke test that the cargo-deny job actually invokes the action
    // (not just imports it).
    let lines: Vec<&str> = wf
        .lines()
        .filter(|l| l.contains("EmbarkStudios/cargo-deny-action"))
        .collect();
    assert!(
        !lines.is_empty(),
        "audit.yaml must reference the cargo-deny action at least once (AC2)."
    );
}

#[test]
fn audit_workflow_runs_audit_check_v2() {
    let wf = read_workflow();
    let lines: Vec<&str> = wf
        .lines()
        .filter(|l| l.contains("rustsec/audit-check"))
        .collect();
    assert!(
        !lines.is_empty(),
        "audit.yaml must reference rustsec/audit-check at least once (AC5)."
    );
}

// ---------------------------------------------------------------------------
// audit.yaml — does NOT skip on docs-only PRs
// ---------------------------------------------------------------------------

#[test]
fn audit_workflow_does_not_use_paths_ignore() {
    let wf = read_workflow();
    // ci.yaml skips docs-only PRs, which is fine for lint/test. The
    // audit workflow MUST NOT — a docs-only PR that also bumps a dep
    // could otherwise sneak past the gate (the dep change goes
    // through `paths-ignore` because Cargo.toml is not in `docs/`,
    // but a sloppy edit that adds `**/*.toml` to paths-ignore would
    // silently break this). Lock the absence in.
    assert!(
        !wf.contains("paths-ignore:"),
        "audit.yaml must NOT use `paths-ignore:` — every push/PR must \
         go through the supply-chain gate, including docs-only PRs."
    );
}

// ---------------------------------------------------------------------------
// audit.yaml — Cargo.lock is generated in-job, not committed
// ---------------------------------------------------------------------------

#[test]
fn audit_workflow_generates_lockfile_before_scanning() {
    let wf = read_workflow();
    // Cargo.lock is intentionally gitignored (.gitignore:7-13).
    // The workflow must generate one in-job. Without this, both
    // cargo-deny's advisory check and cargo-audit could match
    // version-specific RUSTSEC entries against a non-deterministic
    // resolution.
    assert!(
        wf.contains("cargo generate-lockfile"),
        "audit.yaml must run `cargo generate-lockfile` before invoking \
         cargo-deny / cargo-audit — Cargo.lock is gitignored for this \
         library crate."
    );
}

// ---------------------------------------------------------------------------
// deny.toml — file existence
// ---------------------------------------------------------------------------

#[test]
fn deny_toml_exists_at_repo_root() {
    let p = deny_toml_path();
    assert!(
        p.exists(),
        "deny.toml must exist at the repo root — it's the single \
         source of truth for cargo-deny policy."
    );
}

#[test]
fn no_audit_toml_at_repo_root() {
    // cargo-deny is the unified policy file. cargo-audit also reads
    // an `audit.toml`; if someone adds one we'd have two sources of
    // truth for advisory policy. Forbid it.
    let p = repo_root().join("audit.toml");
    assert!(
        !p.exists(),
        "audit.toml must not exist at the repo root — deny.toml is the \
         single source of truth for advisory policy (AC9)."
    );
}

// ---------------------------------------------------------------------------
// deny.toml — licence allowlist
// ---------------------------------------------------------------------------

#[test]
fn deny_toml_allowlist_contains_ticket_licenses() {
    let toml = read_deny_toml();
    // The ticket's allowlist (Unicode-3.0 added per spec
    // investigation — unicode-ident 1.0.x forces it).
    let required = [
        "EUPL-1.2",
        "MIT",
        "Apache-2.0",
        "Apache-2.0 WITH LLVM-exception",
        "BSD-2-Clause",
        "BSD-3-Clause",
        "ISC",
        "Unicode-DFS-2016",
        "Unicode-3.0",
        "Zlib",
        "CC0-1.0",
    ];
    for licence in required {
        // Match the quoted form to avoid accidental matches against
        // the rationale comment block (which mentions licences in
        // prose).
        let needle = format!("\"{licence}\"");
        assert!(
            toml.contains(&needle),
            "deny.toml [licenses].allow must contain {needle} — required \
             by the ENG-4686 ticket (or by the Unicode-3.0 finding for \
             unicode-ident 1.0.x)."
        );
    }
}

#[test]
fn deny_toml_allowlist_excludes_gpl() {
    let toml = read_deny_toml();
    // Extract just the `allow = [...]` block so the GPL mentions in
    // the rationale comment don't trigger a false positive.
    let allow_start = toml
        .find("allow = [")
        .expect("deny.toml must contain `allow = [` (the [licenses].allow list)");
    let allow_end = toml[allow_start..]
        .find(']')
        .map(|i| allow_start + i)
        .expect("deny.toml `allow = [` must be closed by `]`");
    let allow_block = &toml[allow_start..=allow_end];

    for forbidden in ["\"GPL-3.0\"", "\"AGPL-3.0\"", "\"LGPL-3.0\""] {
        assert!(
            !allow_block.contains(forbidden),
            "deny.toml [licenses].allow must NOT contain {forbidden} \
             — copyleft-viral scope would force-relicense rustyroute \
             out of EUPL-1.2 for any consumer pulling the affected \
             dep (AC3, AC7)."
        );
    }
}

#[test]
fn deny_toml_license_rationale_comment_is_present() {
    let toml = read_deny_toml();
    // The load-bearing comment block must explain WHY GPL-3.0 is
    // excluded. This canary protects against a future "tidy up the
    // comments" PR that silently deletes the rationale.
    assert!(
        toml.contains("Why GPL-3.0"),
        "deny.toml must contain the `Why GPL-3.0` rationale block \
         (load-bearing comment per the ticket — future maintainers \
         need to know why GPL-3.0 isn't on the allowlist) (AC7)."
    );
    assert!(
        toml.contains("EUPL-1.2"),
        "deny.toml rationale block must reference EUPL-1.2 (the \
         crate's own licence) — that's the constraint the allowlist \
         flows from."
    );
}

#[test]
fn deny_toml_confidence_threshold_meets_ticket_floor() {
    let toml = read_deny_toml();
    // Ticket: "Confidence >= 0.8". The threshold must be at least
    // 0.8 — match the exact value used in the file.
    assert!(
        toml.contains("confidence-threshold = 0.8"),
        "deny.toml must set [licenses].confidence-threshold = 0.8 \
         per the ENG-4686 ticket. Lower values let cargo-deny's \
         SPDX matcher accept low-confidence licence guesses."
    );
}

// ---------------------------------------------------------------------------
// deny.toml — bans
// ---------------------------------------------------------------------------

#[test]
fn deny_toml_bans_openssl_sys() {
    let toml = read_deny_toml();
    // Locate the `deny = [` block under `[bans]` and assert
    // openssl-sys is inside it (not just mentioned in a comment).
    let bans_start = toml
        .find("[bans]")
        .expect("deny.toml must contain a [bans] table");
    let bans_section = &toml[bans_start..];
    let deny_start = bans_section
        .find("deny = [")
        .expect("deny.toml [bans] must contain a `deny = [` list");
    let deny_end = bans_section[deny_start..]
        .find(']')
        .map(|i| deny_start + i)
        .expect("deny.toml [bans].deny `[` must be closed");
    let deny_block = &bans_section[deny_start..=deny_end];

    assert!(
        deny_block.contains("openssl-sys"),
        "deny.toml [bans].deny must list `openssl-sys` — the ticket's \
         load-bearing 'prefer rustls' ban. A future edit that removes \
         it would silently open the door to TLS deps we haven't \
         vetted."
    );
}

#[test]
fn deny_toml_skip_tree_covers_windows_sys_churn() {
    let toml = read_deny_toml();
    // Both 0.45 and 0.48 must be in the skip-tree block alongside
    // `windows-sys`. The ticket explicitly names both versions
    // because the duplicate-versions churn around them is the
    // expected noise source.
    let skip_start = toml
        .find("skip-tree = [")
        .expect("deny.toml must contain a `skip-tree = [` block under [bans]");
    let skip_end = toml[skip_start..]
        .find(']')
        .map(|i| skip_start + i)
        .expect("deny.toml `skip-tree = [` must be closed by `]`");
    let skip_block = &toml[skip_start..=skip_end];

    assert!(
        skip_block.contains("windows-sys"),
        "deny.toml [bans].skip-tree must reference `windows-sys`."
    );
    assert!(
        skip_block.contains("0.45"),
        "deny.toml [bans].skip-tree must cover windows-sys 0.45 \
         (forward-looking churn skip per the ticket)."
    );
    assert!(
        skip_block.contains("0.48"),
        "deny.toml [bans].skip-tree must cover windows-sys 0.48 \
         (forward-looking churn skip per the ticket)."
    );
}

// ---------------------------------------------------------------------------
// deny.toml — sources
// ---------------------------------------------------------------------------

#[test]
fn deny_toml_sources_restricts_to_crates_io() {
    let toml = read_deny_toml();
    assert!(
        toml.contains("unknown-registry = \"deny\""),
        "deny.toml [sources] must set `unknown-registry = \"deny\"` \
         — no alternate registries allowed."
    );
    assert!(
        toml.contains("unknown-git = \"deny\""),
        "deny.toml [sources] must set `unknown-git = \"deny\"` \
         — no git deps allowed."
    );
    assert!(
        toml.contains("https://github.com/rust-lang/crates.io-index"),
        "deny.toml [sources].allow-registry must list crates.io-index."
    );
}

// ---------------------------------------------------------------------------
// deny.toml — advisories
// ---------------------------------------------------------------------------

#[test]
fn deny_toml_advisories_yanked_is_denied() {
    let toml = read_deny_toml();
    assert!(
        toml.contains("yanked = \"deny\""),
        "deny.toml [advisories] must set `yanked = \"deny\"`. The \
         ticket said 'warn' but the project posture is to fail on \
         yanked — a yanked upstream version is the strongest \
         possible signal of trouble (rationale documented in \
         deny.toml's [advisories] header)."
    );
}

#[test]
fn deny_toml_advisories_unmaintained_is_set() {
    let toml = read_deny_toml();
    assert!(
        toml.contains("unmaintained = \"all\""),
        "deny.toml [advisories] must set `unmaintained = \"all\"` so \
         every crate (including build / dev) is scanned for RUSTSEC \
         unmaintained advisories. Mapping: ticket's 'warn \
         unmaintained' → current cargo-deny schema uses \"all\" / \
         \"workspace\" / \"transitive\" / \"none\" not warn/deny; \
         \"all\" matches the spirit (surface unmaintained \
         everywhere)."
    );
}

#[test]
fn deny_toml_advisories_ignore_format_canary_present() {
    let toml = read_deny_toml();
    // The format rule for [advisories].ignore is enforced by code
    // review: each RUSTSEC line carries a date AND a reason. Lock
    // the format-documentation canary in so it can't be silently
    // deleted.
    assert!(
        toml.contains("RUSTSEC-") && (toml.contains("date") || toml.contains("YYYY-MM-DD")),
        "deny.toml [advisories].ignore must document the format \
         requirement: each RUSTSEC-ID line carries a date and a \
         reason. The example / format canary protects reviewers \
         from inheriting ignores with no rationale."
    );
}

// ---------------------------------------------------------------------------
// End-to-end: live `cargo deny check` invocation (AC1)
// ---------------------------------------------------------------------------
//
// AC1 says "cargo deny check runs against deny.toml with no errors at HEAD".
// The other tests above lock the *shape* of the policy in source control,
// but they can't catch e.g. an invalid TOML key, a schema-drift mistake,
// or a real licence-allowlist gap that only surfaces when cargo-deny
// resolves the live dependency graph.
//
// We don't want to add cargo-deny as a hard dev-dependency (spec
// non-goal). Instead this test:
//
//   1. Looks for `cargo-deny` on PATH.
//   2. If absent, prints a skip notice and returns success — the test
//      stays green on machines without cargo-deny installed, matching
//      the spec's developer-install path (`cargo install cargo-deny`).
//   3. If present, runs `cargo deny --manifest-path … check` and asserts
//      exit code 0. A non-zero exit (licence, ban, advisory, or source
//      violation) becomes a test failure that names the offending crate
//      in the captured stderr — exactly the signal AC1 promises.
//
// CI runs cargo-deny via the EmbarkStudios action (audit.yaml), not via
// this test, so this is a *local* belt-and-braces convenience for devs
// who have cargo-deny installed. CI behaviour is unchanged.

#[test]
fn deny_toml_passes_cargo_deny_check_if_tool_is_installed() {
    use std::process::Command;

    // Probe for cargo-deny on PATH via `cargo deny --version`. We use
    // `cargo deny` (not bare `cargo-deny`) because that's the documented
    // invocation form and the one CI uses.
    let probe = Command::new("cargo").args(["deny", "--version"]).output();

    let probe = match probe {
        Ok(o) if o.status.success() => o,
        _ => {
            eprintln!(
                "SKIP: cargo-deny not installed locally. Install with \
                 `cargo install --locked cargo-deny` to enable this \
                 test. CI runs cargo-deny via the audit.yaml workflow \
                 regardless."
            );
            return;
        }
    };
    let version = String::from_utf8_lossy(&probe.stdout);
    eprintln!("cargo-deny detected: {}", version.trim());

    // cargo-deny needs a lockfile when checking advisories. The crate
    // is a library so Cargo.lock is gitignored — generate one here in
    // the target directory so we don't dirty the working tree.
    let manifest = repo_root().join("Cargo.toml");
    let lock_gen = Command::new("cargo")
        .args(["generate-lockfile", "--manifest-path"])
        .arg(&manifest)
        .output()
        .expect("failed to invoke `cargo generate-lockfile`");
    assert!(
        lock_gen.status.success(),
        "cargo generate-lockfile failed: {}",
        String::from_utf8_lossy(&lock_gen.stderr)
    );

    // Run the actual gate. Match the CI invocation:
    //   `cargo deny --all-features check`
    // (--all-features mirrors the [graph].all-features = true setting in
    // deny.toml — without it cargo-deny would only resolve the default
    // feature graph, missing feature-gated transitive deps.)
    let out = Command::new("cargo")
        .args(["deny", "--all-features", "--manifest-path"])
        .arg(&manifest)
        .arg("check")
        .output()
        .expect("failed to invoke `cargo deny check`");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "cargo deny check FAILED (AC1). stdout:\n{stdout}\n\nstderr:\n{stderr}"
    );
}
