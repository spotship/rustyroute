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
//!   - `cargo fmt --check` hook has the four contract attributes.
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

    // Hook id and entry must appear.
    assert!(
        cfg.contains("id: cargo-fmt-check"),
        "must define a local hook with `id: cargo-fmt-check`"
    );
    // The entry must run `cargo fmt --check` (with `--check`, not a
    // bare `cargo fmt` that would silently reformat files instead of
    // failing on drift). We assert the substring rather than the
    // literal full entry because the hook also runs `cargo fmt
    // --manifest-path tests/downstream_consumer/Cargo.toml --check`
    // (a non-workspace sub-package — see below).
    assert!(
        cfg.contains("cargo fmt --check"),
        "cargo-fmt-check hook entry must include `cargo fmt --check` — without `--check`, \
         the hook would silently reformat files instead of failing on drift."
    );
    // The nested `tests/downstream_consumer/` package is its own
    // `[package]` (not a workspace member), so the root `cargo fmt
    // --check` does NOT traverse into it. The hook must also format
    // that sub-package explicitly — otherwise AC3 (commit with bad
    // formatting fails) silently misses the sub-package.
    assert!(
        cfg.contains("--manifest-path tests/downstream_consumer/Cargo.toml --check"),
        "cargo-fmt-check hook must also run `cargo fmt --manifest-path \
         tests/downstream_consumer/Cargo.toml --check` — the nested downstream_consumer \
         package is not a workspace member, so the root invocation does not cover it."
    );

    // The four contract attributes for a `cargo fmt`-style local hook.
    // We assert their presence somewhere in the file rather than by
    // YAML structure to keep the test parser-free, matching
    // `tests/ci_workflow.rs`'s style.
    for needle in ["language: system", "pass_filenames: false", "types: [rust]"] {
        assert!(
            cfg.contains(needle),
            "cargo-fmt-check hook must include `{needle}` — without it the hook \
             would either need a managed Rust toolchain (`language: rust` isn't \
             supported for `cargo fmt`), pass filenames as positional args \
             (which `cargo fmt` rejects), or fire on every commit even when no \
             Rust file is staged."
        );
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
    let cfg = read_config();
    for forbidden in [
        // Python-runtime hooks from pre-commit-hooks v5
        "id: debug-statements",
        "id: check-builtin-literals",
        "id: check-docstring-first",
        // Python tooling repos copied wholesale from backend
        "ruff-pre-commit",
        "django-upgrade",
        "djLint",
    ] {
        assert!(
            !cfg.contains(forbidden),
            "`.pre-commit-config.yaml` must NOT include `{forbidden}` — this is a \
             Python-specific hook/repo from backend's config and does not apply to \
             a Rust crate. Catches accidental wholesale copy of backend config."
        );
    }
}
