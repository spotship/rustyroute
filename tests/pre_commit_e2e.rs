//! ENG-4687 end-to-end tests: shell out to the real `pre-commit` tool
//! and assert against the user-observable behaviour of the hooks.
//!
//! Scope vs. `tests/pre_commit_config.rs`
//! --------------------------------------
//! `pre_commit_config.rs` locks the YAML's *invariants* via string
//! assertions (cheap, no external dependencies, runs on every `cargo
//! test`). That file proves AC5 ("invariants locked against silent
//! drift").
//!
//! The tests in this file cover the *runtime* acceptance criteria that
//! require actually executing the hooks:
//!
//!   - AC2: `pre-commit run --all-files` passes on the current tree.
//!   - AC3: Committing a fmt-bad `.rs` file fails the
//!     `cargo-fmt-check` hook with a non-zero exit and a diff.
//!   - AC4: `pre-commit run --hook-stage manual cargo-clippy` runs
//!     clippy.
//!
//! Skip semantics
//! --------------
//! These tests shell out to the system `pre-commit` binary. If it is
//! not on `PATH` (e.g. a Rust-only contributor without Python), the
//! test prints a skip message to stderr and `return`s early without
//! asserting — matches the spec's "AC1 manual" framing where
//! pre-commit install is a per-clone setup, not a cargo-test prereq.
//! Cargo still reports these as passing (no panic == pass for a
//! `#[test] fn` returning `()`), so the skip is invisible to the
//! suite's pass/fail count. The CI workflow's dedicated `pre-commit`
//! job (`.github/workflows/ci.yaml`) installs `pre-commit` via `pip`
//! and then runs `cargo test --test pre_commit_e2e` so AC2/AC3/AC4
//! are exercised for real on every PR; the other CI jobs (fmt,
//! clippy, test-matrix, ...) do NOT install pre-commit, so this file
//! silently skips there by design.
//!
//! Isolation
//! ---------
//! AC3 mutates a worktree and creates a commit. To avoid touching the
//! checked-out rustyroute repo, the test materialises a throwaway git
//! repo via `tempfile::tempdir()` (which auto-cleans on drop, even on
//! panic) with the *real* `.pre-commit-config.yaml` copied in, plus
//! the minimum Cargo scaffolding so `cargo fmt --check` has something
//! to check.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the repo root (the directory containing this crate's
/// `Cargo.toml`).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Returns true if a `--version` probe of `bin` succeeds. Used to test
/// for `pre-commit` and `git` on PATH without taking a dependency on
/// the `which` crate.
fn bin_on_path(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// Build a PATH value with `cargo`'s parent directory prepended to
/// the current `PATH`, using the platform's native separator (`:` on
/// Unix, `;` on Windows). Pre-commit hooks shell out to `cargo` and
/// `rustfmt`/`clippy-driver`; on Windows runners those binaries live
/// next to the `cargo` we were launched with (typically
/// `~/.rustup/toolchains/.../bin`), not on the inherited PATH.
fn cargo_augmented_path() -> OsString {
    let cargo = cargo_bin();
    let cargo_dir = Path::new(&cargo).parent().map(Path::to_path_buf);

    let mut entries: Vec<PathBuf> = Vec::new();
    if let Some(dir) = cargo_dir
        && !dir.as_os_str().is_empty()
    {
        entries.push(dir);
    }
    if let Some(path) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&path));
    }

    std::env::join_paths(&entries).expect("PATH entries must not contain the platform separator")
}

/// If pre-commit or git is missing, print why and return false.
/// Callers should early-return when this is false.
fn prereqs_available(test_name: &str) -> bool {
    if !bin_on_path("pre-commit") {
        eprintln!(
            "skipping {test_name}: pre-commit not found on PATH \
             (install with `pip install pre-commit`)"
        );
        return false;
    }
    if !bin_on_path("git") {
        eprintln!("skipping {test_name}: git not found on PATH");
        return false;
    }
    true
}

/// Run a command, capture stdout+stderr, return (status, combined output).
fn run(cmd: &mut Command) -> (std::process::ExitStatus, String) {
    let out = cmd.output().expect("spawn command");
    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&out.stdout));
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status, combined)
}

/// Build a throwaway git repo in `dir` with:
///   - the real `.pre-commit-config.yaml` from this repo
///   - a minimal `Cargo.toml` so `cargo fmt --check` has something to do
///   - the `tests/downstream_consumer/` sub-package layout (because the
///     hook entry runs `cargo fmt --manifest-path tests/downstream_consumer/Cargo.toml --check`
///     — if the manifest is missing the hook fails for an unrelated reason)
///   - an initial commit so subsequent `git commit` invocations work
///
/// Returns the path to the seeded repo root.
fn seed_tmp_repo(dir: &Path) -> PathBuf {
    let repo = dir.to_path_buf();

    // git init + identity (pre-commit requires a git repo; commit needs author).
    let (st, out) = run(Command::new("git").arg("init").arg("-q").arg(&repo));
    assert!(st.success(), "git init failed: {out}");
    for (k, v) in [
        ("user.email", "e2e@rustyroute.test"),
        ("user.name", "E2E Test"),
        ("commit.gpgsign", "false"),
    ] {
        let (st, out) = run(Command::new("git")
            .current_dir(&repo)
            .args(["config", k, v]));
        assert!(st.success(), "git config {k} failed: {out}");
    }

    // Copy the real .pre-commit-config.yaml.
    let cfg_src = repo_root().join(".pre-commit-config.yaml");
    let cfg_dst = repo.join(".pre-commit-config.yaml");
    fs::copy(&cfg_src, &cfg_dst).expect("copy .pre-commit-config.yaml");

    // Minimal root Cargo.toml + src/lib.rs (formatted).
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\n\
         name = \"e2e_root\"\n\
         version = \"0.0.0\"\n\
         edition = \"2024\"\n",
    )
    .unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(
        repo.join("src/lib.rs"),
        "pub fn hello() -> u32 {\n    42\n}\n",
    )
    .unwrap();

    // Stub the downstream_consumer sub-package because the hook entry
    // references `tests/downstream_consumer/Cargo.toml` explicitly.
    let sub = repo.join("tests/downstream_consumer");
    fs::create_dir_all(sub.join("src")).unwrap();
    fs::write(
        sub.join("Cargo.toml"),
        "[package]\n\
         name = \"e2e_sub\"\n\
         version = \"0.0.0\"\n\
         edition = \"2024\"\n",
    )
    .unwrap();
    fs::write(sub.join("src/lib.rs"), "pub fn ping() -> u32 {\n    1\n}\n").unwrap();

    // Initial commit so future commits have a parent.
    let (st, out) = run(Command::new("git").current_dir(&repo).args(["add", "-A"]));
    assert!(st.success(), "git add failed: {out}");
    let (st, out) = run(Command::new("git").current_dir(&repo).args([
        "commit",
        "-q",
        "--no-verify",
        "-m",
        "seed",
    ]));
    assert!(st.success(), "git commit (seed) failed: {out}");

    repo
}

// ---------------------------------------------------------------------------
// AC2: pre-commit run --all-files passes on a clean rustyroute tree
// ---------------------------------------------------------------------------
//
// This runs against the ACTUAL repo root (not a tempdir) — that is the
// point of AC2: the rustyroute working tree should be hook-clean.
//
// If the suite fails, the test prints pre-commit's stdout/stderr so the
// failing hook id is visible in CI logs. The test snapshots `git diff
// --quiet` before and after the run; if the tree was clean going in
// and pre-commit modified any files, that counts as a hook-modified-tree
// failure even if pre-commit somehow exited 0 — an auto-fixer that
// "succeeds" by silently rewriting files would still break AC2's
// "main-equivalent tree is hook-clean" contract. We capture the diff
// (for the error message), restore the tree, then assert.

#[test]
fn ac2_pre_commit_run_all_files_passes_on_current_tree() {
    if !prereqs_available("ac2_pre_commit_run_all_files_passes_on_current_tree") {
        return;
    }
    let root = repo_root();

    // Snapshot working-tree state before running. `git status
    // --porcelain` is empty only when the tree has no unstaged
    // changes, no staged changes, AND no untracked files — `git diff
    // --quiet` alone would treat a tree with staged or untracked
    // work as "clean" and the drift-recovery path below would then
    // `git checkout -- .` over a contributor's in-progress work.
    let (pre_status_st, pre_status_out) = run(Command::new("git")
        .current_dir(&root)
        .args(["status", "--porcelain"]));
    let pre_clean = pre_status_st.success() && pre_status_out.is_empty();

    let (st, output) = run(Command::new("pre-commit").current_dir(&root).args([
        "run",
        "--all-files",
        "--color",
        "never",
    ]));

    // If a hook is an auto-fixer (end-of-file-fixer, trailing-whitespace,
    // mixed-line-ending) it will both modify files AND return non-zero.
    // Capture the post-run drift (if we started clean) so we can both
    // restore the worktree and fail the assertion with a useful diff.
    let post_drift: Option<String> = if pre_clean {
        let (post_status_st, post_status_out) = run(Command::new("git")
            .current_dir(&root)
            .args(["status", "--porcelain"]));
        if post_status_st.success() && post_status_out.is_empty() {
            None
        } else {
            let (_diff_st, diff) = run(Command::new("git").current_dir(&root).args(["diff"]));
            // Safe to fully reset here: the pre-check used
            // `git status --porcelain` to confirm the tree was truly
            // clean (no unstaged, no staged, no untracked), so anything
            // we see now must have come from pre-commit. Restore
            // tracked-file edits AND remove any untracked artifacts
            // (`git checkout -- .` alone leaves untracked files behind).
            let _ = run(Command::new("git")
                .current_dir(&root)
                .args(["checkout", "--", "."]));
            let _ = run(Command::new("git")
                .current_dir(&root)
                .args(["clean", "-fd"]));
            Some(if diff.is_empty() {
                post_status_out
            } else {
                diff
            })
        }
    } else {
        // We started with local changes (unstaged edits, staged work,
        // or untracked files). Don't touch the worktree, don't assert
        // on drift — the contributor's own changes would dominate the
        // diff. AC2's exit-status check below is still meaningful.
        None
    };

    assert!(
        st.success(),
        "AC2 violated: `pre-commit run --all-files` failed on the current tree.\n\
         A failing hook means a contributor running `git commit` will be blocked\n\
         until the offending file is fixed. The acceptance criterion requires the\n\
         main-equivalent tree to be hook-clean.\n\n\
         pre-commit output:\n{output}"
    );

    // Even if pre-commit exited 0, a hook may have silently rewritten
    // a file. That violates AC2's "the main-equivalent tree is
    // hook-clean" guarantee just as much as a non-zero exit would.
    assert!(
        post_drift.is_none(),
        "AC2 violated: a pre-commit hook modified files on the current tree even \
         though pre-commit exited 0. AC2 requires the tree to be hook-clean — an \
         auto-fixer rewriting files is a fail even without a non-zero exit.\n\n\
         tree diff after pre-commit run:\n{}\n\n\
         pre-commit output:\n{output}",
        post_drift.as_deref().unwrap_or("")
    );
}

// ---------------------------------------------------------------------------
// AC3: cargo-fmt-check fails on staged-but-misformatted Rust
// ---------------------------------------------------------------------------
//
// Materialise a throwaway repo, introduce a mis-indented `.rs` file,
// stage it, and run `pre-commit run cargo-fmt-check`. We expect a
// non-zero exit AND output that contains the substring `Diff` (rustfmt
// prints `Diff in <path>` when `--check` finds drift). We do NOT
// require any specific diff text — only that the hook flags the drift.

#[test]
fn ac3_cargo_fmt_check_fails_on_misformatted_rust() {
    if !prereqs_available("ac3_cargo_fmt_check_fails_on_misformatted_rust") {
        return;
    }

    let tmp = tempfile::tempdir().expect("create tempdir");
    let repo = seed_tmp_repo(tmp.path());

    // Write a misformatted Rust file (double space, weird indentation)
    // — rustfmt will refuse this with `cargo fmt --check`.
    fs::write(repo.join("src/lib.rs"), "pub fn  bad( ) ->u32{42 }\n").unwrap();
    let (st, out) = run(Command::new("git")
        .current_dir(&repo)
        .args(["add", "src/lib.rs"]));
    assert!(st.success(), "stage bad file failed: {out}");

    // Pre-commit needs CARGO/rustfmt on PATH. The hook uses
    // `language: system`, so we just need the real cargo.
    let augmented_path = cargo_augmented_path();

    let (st, output) = run(Command::new("pre-commit")
        .current_dir(&repo)
        .env("PATH", &augmented_path)
        .args(["run", "cargo-fmt-check", "--color", "never"]));

    assert!(
        !st.success(),
        "AC3 violated: cargo-fmt-check passed on misformatted code.\n\
         The hook should reject `pub fn  bad( ) ->u32{{42 }}` because rustfmt's \n\
         `--check` mode exits non-zero when reformatting would change the file.\n\n\
         pre-commit output:\n{output}"
    );

    // Rustfmt's --check output emits `Diff in <path>` for each file
    // that would be reformatted. Require that exact signature — a
    // broader match (e.g. on the bare substring `rustfmt`) would let
    // an unrelated error like "rustfmt component not installed" pass
    // AC3 without the contributor seeing a meaningful diff.
    assert!(
        output.contains("Diff in"),
        "AC3 partial: cargo-fmt-check failed but output does not contain rustfmt's \
         `Diff in` signature. Without that, the hook reported a failure but the \
         contributor cannot tell which file needs reformatting. Likely cause: rustfmt \
         exited non-zero for a non-drift reason (e.g. component missing, parse error).\
         \n\nactual output:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// AC4: pre-commit run --hook-stage manual cargo-clippy invokes clippy
// ---------------------------------------------------------------------------
//
// Manual-stage hooks only run when invoked explicitly. We assert that
// `pre-commit run --hook-stage manual cargo-clippy` reaches the
// clippy binary at all — its pass/fail is not the point here (clippy
// on a 5-line tempdir crate is uninteresting). We confirm "clippy was
// invoked" by looking for clippy's banner in the output, which always
// includes `Checking e2e_root v0.0.0` (cargo) and either `clippy` in
// the entry name or clippy's own diagnostics.

#[test]
fn ac4_manual_stage_invokes_clippy() {
    if !prereqs_available("ac4_manual_stage_invokes_clippy") {
        return;
    }

    let tmp = tempfile::tempdir().expect("create tempdir");
    let repo = seed_tmp_repo(tmp.path());

    // We need cargo + clippy-driver on PATH inside the pre-commit
    // subprocess.
    let augmented_path = cargo_augmented_path();

    let (st, output) = run(Command::new("pre-commit")
        .current_dir(&repo)
        .env("PATH", &augmented_path)
        .args([
            "run",
            "--hook-stage",
            "manual",
            "cargo-clippy",
            "--color",
            "never",
        ]));

    // The hook should be RECOGNISED — pre-commit's "no hook with this
    // id" message exits non-zero with a clear diagnostic; we don't
    // want that.
    assert!(
        !output.contains("No hook with id `cargo-clippy`"),
        "AC4 violated: pre-commit did not recognise the cargo-clippy hook id at the \
         manual stage. The hook config may be missing `stages: [manual]` or have a \
         typo in the id.\n\n\
         pre-commit output:\n{output}"
    );

    // The hook should actually invoke cargo+clippy. cargo prints
    // `Checking <crate> v<version>` when it starts work, and the
    // literal entry `cargo clippy --no-deps` appears in pre-commit's
    // command echo. Either is sufficient evidence of invocation.
    // We anchor the positive check on cargo's own output (`Checking
    // <crate>`). The `cargo clippy --no-deps` substring is NOT a
    // sufficient signal — it's also the hook's `name:` field in
    // `.pre-commit-config.yaml`, which pre-commit prints regardless
    // of whether the entry actually executed (e.g. if the clippy
    // toolchain component is missing).
    assert!(
        output.contains("Checking "),
        "AC4 partial: cargo-clippy hook ran at manual stage but pre-commit output \
         does not contain cargo's `Checking <crate>` banner, so clippy did NOT \
         actually execute against the seeded crate. Likely cause: missing clippy \
         toolchain component, or the entry failed before reaching cargo.\n\n\
         hook exit: {st:?}\n\
         pre-commit output:\n{output}"
    );
}

// ---------------------------------------------------------------------------
// AC4 corollary: cargo-clippy does NOT run on the default pre-commit stage
// ---------------------------------------------------------------------------
//
// The spec is explicit: clippy is opt-in. If it crept onto the default
// stage, `pre-commit run --all-files` would invoke it (slow). Verify
// the negative.

#[test]
fn ac4_clippy_does_not_run_on_default_stage() {
    if !prereqs_available("ac4_clippy_does_not_run_on_default_stage") {
        return;
    }

    let tmp = tempfile::tempdir().expect("create tempdir");
    let repo = seed_tmp_repo(tmp.path());

    let augmented_path = cargo_augmented_path();

    // Ask pre-commit to run *just* cargo-clippy on the default stage.
    // It should refuse (the hook is configured for manual only).
    let (st, output) = run(Command::new("pre-commit")
        .current_dir(&repo)
        .env("PATH", &augmented_path)
        .args(["run", "cargo-clippy", "--all-files", "--color", "never"]));

    // Pre-commit emits a stage-mismatch diagnostic when asked to run
    // a hook that is not active in the current stage. The wording is
    // "No hook with id `cargo-clippy` in stage `pre-commit`" on
    // current versions; we match the stable prefix
    // `No hook with id `cargo-clippy`` to ride out trailing-wording
    // tweaks across versions. Pre-commit exits non-zero in that case
    // — assert both the diagnostic and the non-zero exit so a future
    // version that filters the hook silently (exit 0, no banner)
    // cannot pass the test by absence-of-evidence alone.
    assert!(
        !st.success(),
        "AC4 corollary partial: pre-commit exited 0 when asked to run `cargo-clippy` on \
         the default stage. The expected outcome is a non-zero exit with a \"No hook with \
         id\" diagnostic — silent success means clippy may actually have run on the \
         default stage.\n\nfull output:\n{output}"
    );
    assert!(
        output.contains("No hook with id `cargo-clippy`"),
        "AC4 corollary violated: pre-commit did not emit the expected stage-mismatch \
         diagnostic (`No hook with id \\`cargo-clippy\\``) when asked to run the hook \
         on the default stage. The hook must be opt-in via `--hook-stage manual` — \
         clippy is too slow for every commit. Check that the hook keeps \
         `stages: [manual]`.\n\nfull output:\n{output}"
    );
}
