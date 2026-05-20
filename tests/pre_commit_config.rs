//! ENG-4687: lock the structural invariants of `.pre-commit-config.yaml`.
//!
//! Why these tests exist
//! ---------------------
//! `.pre-commit-config.yaml` is the artifact of ENG-4687. Most of its
//! behavior can only be observed by invoking the `pre-commit` tool
//! itself (rustfmt failure prints a diff, hook installation creates a
//! `.git/hooks/pre-commit` shim). Those are not in scope for `cargo
//! test`.
//!
//! What IS in scope here is the small set of invariants whose silent
//! drift would turn the gate into a no-op or pull in inappropriate
//! Python-specific hooks. Mirroring `tests/ci_workflow.rs`, we lock
//! these with string-level assertions instead of adding a YAML parser
//! dev-dependency.
//!
//! Invariants locked:
//!   - File exists at the crate root.
//!   - `default_stages: [pre-commit]` is present.
//!   - Top-level `exclude:` covers target/, data/, vendored .gpkg,
//!     and *.rkyv.
//!   - `pre-commit-hooks` is pinned to v5.0.0 (matches backend's
//!     pinning convention).
//!   - Required baseline hook ids are present.
//!   - Both `cargo fmt --check` hooks (root + downstream) carry the
//!     three per-block contract attributes (`language: system`,
//!     `pass_filenames: false`, `types: [rust]`) — plus the `entry:`
//!     line, asserted separately above as an exact match.
//!   - `cargo clippy` hook is on `stages: [manual]`.
//!   - Python-specific backend hooks/repos are ABSENT.

use std::fs;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".pre-commit-config.yaml")
}

fn read_config() -> String {
    let p = config_path();
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {}: {e}", p.display()))
}

// ---------------------------------------------------------------------------
// File presence
// ---------------------------------------------------------------------------

#[test]
fn pre_commit_config_exists_at_crate_root() {
    let p = config_path();
    assert!(
        p.exists(),
        ".pre-commit-config.yaml must exist at the crate root (got: {})",
        p.display()
    );
}

// ---------------------------------------------------------------------------
// default_stages
// ---------------------------------------------------------------------------

#[test]
fn default_stages_is_pre_commit() {
    let cfg = read_config();
    assert!(
        cfg.contains("default_stages: [pre-commit]"),
        "`.pre-commit-config.yaml` must declare `default_stages: [pre-commit]` \
         so unmarked hooks run on commit (not on push or manual). \
         Without it, hook stage routing is implementation-defined."
    );
}

// ---------------------------------------------------------------------------
// Top-level exclude
// ---------------------------------------------------------------------------

#[test]
fn top_level_exclude_covers_target() {
    let cfg = read_config();
    assert!(
        cfg.contains("^target/"),
        "top-level `exclude:` must cover `^target/` so Cargo build output is skipped"
    );
}

#[test]
fn top_level_exclude_covers_data_dir() {
    let cfg = read_config();
    assert!(
        cfg.contains("^data/"),
        "top-level `exclude:` must cover `^data/` so generated graph archives are skipped"
    );
}

#[test]
fn top_level_exclude_covers_rkyv() {
    let cfg = read_config();
    assert!(
        cfg.contains("\\.rkyv$"),
        "top-level `exclude:` must cover `\\.rkyv$` so generated rkyv archives are skipped"
    );
}

#[test]
fn top_level_exclude_covers_vendored_gpkg() {
    let cfg = read_config();
    assert!(
        cfg.contains("vendor/eurostat-marnet/.*\\.gpkg$"),
        "top-level `exclude:` must cover `vendor/eurostat-marnet/.*\\.gpkg$` so the \
         intentionally vendored MARNET GeoPackages are skipped — without this, \
         `check-added-large-files` and `end-of-file-fixer` would trip on them."
    );
}

// ---------------------------------------------------------------------------
// pre-commit-hooks revision pin
// ---------------------------------------------------------------------------

#[test]
fn pre_commit_hooks_pinned_to_v5_0_0() {
    let cfg = read_config();
    assert!(
        cfg.contains("repo: https://github.com/pre-commit/pre-commit-hooks"),
        "must include the upstream pre-commit/pre-commit-hooks repo"
    );
    assert!(
        cfg.contains("rev: v5.0.0"),
        "pre-commit-hooks must be pinned to `rev: v5.0.0` (not master, not a \
         later version) to match the workspace convention in spotship/backend."
    );
}

// ---------------------------------------------------------------------------
// Baseline hook ids present
// ---------------------------------------------------------------------------

#[test]
fn baseline_hooks_present() {
    let cfg = read_config();
    for needle in [
        "id: trailing-whitespace",
        "id: end-of-file-fixer",
        "id: check-json",
        "id: check-toml",
        "id: check-yaml",
        "id: check-merge-conflict",
        "id: check-case-conflict",
        "id: detect-private-key",
    ] {
        assert!(
            cfg.contains(needle),
            "`.pre-commit-config.yaml` must include `{needle}` from the v5.0.0 baseline"
        );
    }
}

// ---------------------------------------------------------------------------
// cargo fmt --check hook contract
// ---------------------------------------------------------------------------

#[test]
fn cargo_fmt_check_hook_contract() {
    let cfg = read_config();

    // Two hooks: the root crate and the nested downstream_consumer
    // sub-package (its own [package], not a workspace member — so
    // the root `cargo fmt --check` does NOT traverse into it). Split
    // into two hooks (rather than chained via `bash -c`) so neither
    // entry depends on bash being on PATH — relevant on Windows
    // runners in the CI matrix.
    assert!(
        cfg.contains("id: cargo-fmt-check"),
        "must define a local hook with `id: cargo-fmt-check` covering the root crate"
    );
    assert!(
        cfg.contains("id: cargo-fmt-check-downstream"),
        "must define a second local hook with `id: cargo-fmt-check-downstream` for the \
         nested tests/downstream_consumer sub-package — the root `cargo fmt --check` \
         does not traverse into a non-workspace-member [package], so AC3 would silently \
         miss fmt drift in that sub-crate without an explicit hook."
    );

    // Neither entry may rely on a shell. `bash -c '...'` would fail
    // on Windows pre-commit runs that don't have bash on PATH.
    assert!(
        !cfg.contains("bash -c"),
        "cargo-fmt-check hooks must not use `bash -c` — bash is not guaranteed on \
         Windows runners. Split chained commands into separate hooks instead."
    );

    // Find every `entry:` line and check the cargo-fmt-check entries
    // verbatim (modulo leading whitespace). A `cfg.contains(...)`
    // substring check would let `entry: cargo fmt --check --all` pass
    // even though `--all` would silently expand coverage to workspace
    // members the hook does not intend to format here.
    let entries: Vec<&str> = cfg
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("entry:"))
        .collect();

    // The root hook must be `entry: cargo fmt --check` — exactly. No
    // trailing args (no `--all`, no extra manifest, etc.) — the
    // sub-package gets its own dedicated hook below.
    assert!(
        entries
            .iter()
            .any(|line| *line == "entry: cargo fmt --check"),
        "cargo-fmt-check (root) hook entry must be exactly `cargo fmt --check` — \
         without `--check`, the hook would silently reformat files instead of \
         failing on drift; with extra args (e.g. `--all`) the coverage would \
         silently shift. Found entries: {entries:?}"
    );

    // The downstream hook must target the sub-package's manifest
    // explicitly and use `--check`.
    assert!(
        entries.iter().any(|line| {
            *line == "entry: cargo fmt --manifest-path tests/downstream_consumer/Cargo.toml --check"
        }),
        "cargo-fmt-check-downstream hook entry must be exactly `cargo fmt \
         --manifest-path tests/downstream_consumer/Cargo.toml --check` — without \
         `--manifest-path`, cargo would find the root manifest and skip the \
         sub-package. Found entries: {entries:?}"
    );

    // The three contract attributes for a `cargo fmt`-style local hook.
    // Both fmt hooks (root + downstream) must carry these. We assert
    // per-hook (by locating each `- id:` line and reading up to the
    // next `- id:`) rather than counting occurrences across the whole
    // file — a global count of `>= 2` could be satisfied by other
    // hooks that also use `language: system` and `pass_filenames:
    // false` (e.g. cargo-clippy), masking a silently dropped attribute
    // on one of the fmt hooks.
    let lines: Vec<&str> = cfg.lines().collect();
    for hook_id in ["cargo-fmt-check", "cargo-fmt-check-downstream"] {
        let header = format!("- id: {hook_id}");
        let start = lines
            .iter()
            .position(|line| line.trim_start() == header)
            .unwrap_or_else(|| panic!("hook `{hook_id}` not found in config"));
        let end = lines[start + 1..]
            .iter()
            .position(|line| line.trim_start().starts_with("- id:"))
            .map(|offset| start + 1 + offset)
            .unwrap_or(lines.len());
        let block = &lines[start..end];

        for needle in ["language: system", "pass_filenames: false", "types: [rust]"] {
            assert!(
                block.iter().any(|line| line.contains(needle)),
                "hook `{hook_id}` must include `{needle}` in its own block — without \
                 it the hook would either need a managed Rust toolchain (`language: \
                 rust` isn't supported for `cargo fmt`), pass filenames as positional \
                 args (which `cargo fmt` rejects), or fire on every commit even when \
                 no Rust file is staged. Block scanned:\n{block:#?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// cargo clippy hook is manual-stage
// ---------------------------------------------------------------------------

#[test]
fn cargo_clippy_hook_is_manual_stage() {
    let cfg = read_config();
    assert!(
        cfg.contains("id: cargo-clippy"),
        "must define a local hook with `id: cargo-clippy`"
    );
    assert!(
        cfg.contains("entry: cargo clippy --no-deps -- -D warnings"),
        "cargo-clippy hook must use `entry: cargo clippy --no-deps -- -D warnings` — \
         matches the planning request and the manual smoke-test scope."
    );
    assert!(
        cfg.contains("stages: [manual]"),
        "cargo-clippy hook must be on `stages: [manual]` — clippy is too slow \
         for every commit, so it is opt-in via `pre-commit run --hook-stage manual`."
    );
}

// ---------------------------------------------------------------------------
// Python-specific backend hooks are ABSENT
// ---------------------------------------------------------------------------

#[test]
fn python_specific_backend_hooks_are_absent() {
    // Lowercase the config once so the forbidden-substring check
    // is case-insensitive. This catches both `djLint` (the upstream
    // repo casing) and `djlint` (the lowercase hook id) without
    // needing to enumerate every casing variant.
    let cfg = read_config().to_lowercase();
    for forbidden in [
        // Python-runtime hooks from pre-commit-hooks v5
        "id: debug-statements",
        "id: check-builtin-literals",
        "id: check-docstring-first",
        // Python tooling repos copied wholesale from backend
        "ruff-pre-commit",
        "django-upgrade",
        "djlint",
    ] {
        assert!(
            !cfg.contains(forbidden),
            "`.pre-commit-config.yaml` must NOT include `{forbidden}` (case-insensitive) \
             — this is a Python-specific hook/repo from backend's config and does not \
             apply to a Rust crate. Catches accidental wholesale copy of backend config."
        );
    }
}
