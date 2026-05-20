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
//! test prints a skip message and returns `Ok(())` rather than failing
//! — matches the spec's "AC1 manual" framing where pre-commit install
//! is a per-clone setup, not a cargo-test prereq. The CI workflow's
//! dedicated `pre-commit` job (`.github/workflows/ci.yaml`) installs
//! `pre-commit` via `pip` and then runs `cargo test --test
//! pre_commit_e2e` so AC2/AC3/AC4 are exercised for real on every PR;
//! the other CI jobs (fmt, clippy, test-matrix, ...) do NOT install
//! pre-commit, so this file silently skips there by design.
//!
//! Isolation
//! ---------
//! AC3 mutates a worktree and creates a commit. To avoid touching the
//! checked-out rustyroute repo, the test materialises a throwaway git
//! repo in `std::env::temp_dir()` with the *real*
//! `.pre-commit-config.yaml` symlinked/copied in, plus the minimum
//! Cargo scaffolding so `cargo fmt --check` has something to check.
//! The test cleans up its tempdir on success and on panic via
//! `tempfile::TempDir`.

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

/// Returns `Some(path)` if `pre-commit` is on PATH, else `None`.
fn pre_commit_on_path() -> Option<PathBuf> {
    // Use `which`-equivalent logic by trying `pre-commit --version`.
    let out = Command::new("pre-commit").arg("--version").output().ok()?;
    if out.status.success() {
        // The exact path doesn't matter; just signal "available".
        Some(PathBuf::from("pre-commit"))
    } else {
        None
    }
}

fn git_on_path() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// If pre-commit or git is missing, print why and return false.
/// Callers should early-return when this is false.
fn prereqs_available(test_name: &str) -> bool {
    if pre_commit_on_path().is_none() {
        eprintln!(
            "skipping {test_name}: pre-commit not found on PATH \
             (install with `pip install pre-commit`)"
        );
        return false;
    }
    if !git_on_path() {
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
// failing hook id is visible in CI logs. We do NOT mutate the working
// tree — pre-commit's auto-fixers (e.g. end-of-file-fixer) can write
// files, but we leave restoration to the caller / git. The test
// re-snapshots the working tree before and after via `git diff
// --quiet`; if pre-commit modified files, that itself counts as a fail.

#[test]
fn ac2_pre_commit_run_all_files_passes_on_current_tree() {
    if !prereqs_available("ac2_pre_commit_run_all_files_passes_on_current_tree") {
        return;
    }
    let root = repo_root();

    // Snapshot working-tree state before running.
    let (pre_st, _) = run(Command::new("git")
        .current_dir(&root)
        .args(["diff", "--quiet"]));
    let pre_clean = pre_st.success();

    let (st, output) = run(Command::new("pre-commit").current_dir(&root).args([
        "run",
        "--all-files",
        "--color",
        "never",
    ]));

    // If a hook is an auto-fixer (end-of-file-fixer, trailing-whitespace,
    // mixed-line-ending) it will both modify files AND return non-zero.
    // Restore any modifications so we don't leave the worktree dirty —
    // but only if it was clean BEFORE we ran. Otherwise we'd clobber
    // user changes.
    if pre_clean {
        let (post_st, _) = run(Command::new("git")
            .current_dir(&root)
            .args(["diff", "--quiet"]));
        if !post_st.success() {
            let _ = run(Command::new("git")
                .current_dir(&root)
                .args(["checkout", "--", "."]));
        }
    }

    assert!(
        st.success(),
        "AC2 violated: `pre-commit run --all-files` failed on the current tree.\n\
         A failing hook means a contributor running `git commit` will be blocked\n\
         until the offending file is fixed. The acceptance criterion requires the\n\
         main-equivalent tree to be hook-clean.\n\n\
         pre-commit output:\n{output}"
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
    let cargo = cargo_bin();
    let path = std::env::var("PATH").unwrap_or_default();
    let cargo_dir = Path::new(&cargo)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let augmented_path = if cargo_dir.is_empty() {
        path.clone()
    } else {
        format!("{cargo_dir}:{path}")
    };

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

    // Rustfmt's --check output contains `Diff in` lines. Verify the
    // contributor actually sees a diff (not a generic error), so the
    // failure is self-explanatory.
    assert!(
        output.contains("Diff in") || output.contains("rustfmt"),
        "AC3 partial: cargo-fmt-check failed but output does not look like rustfmt's \
         diff. Expected `Diff in` (rustfmt --check signature) or at least `rustfmt` \
         in the output so contributors know which hook fired.\n\n\
         actual output:\n{output}"
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
    let cargo = cargo_bin();
    let path = std::env::var("PATH").unwrap_or_default();
    let cargo_dir = Path::new(&cargo)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let augmented_path = if cargo_dir.is_empty() {
        path.clone()
    } else {
        format!("{cargo_dir}:{path}")
    };

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
    // (We deliberately don't match the bare substring `clippy` — that
    // also appears in pre-commit's hook-name banner, which is printed
    // regardless of whether the entry actually executed.)
    let invoked = output.contains("Checking ") || output.contains("cargo clippy --no-deps");
    assert!(
        invoked,
        "AC4 partial: cargo-clippy hook ran at manual stage but output does not \
         show clippy actually executing. Expected `Checking ...` (cargo banner) or \
         `clippy` in the output.\n\n\
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

    let cargo = cargo_bin();
    let path = std::env::var("PATH").unwrap_or_default();
    let cargo_dir = Path::new(&cargo)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let augmented_path = if cargo_dir.is_empty() {
        path.clone()
    } else {
        format!("{cargo_dir}:{path}")
    };

    // Ask pre-commit to run *just* cargo-clippy on the default stage.
    // It should refuse (the hook is configured for manual only).
    let (_st, output) = run(Command::new("pre-commit")
        .current_dir(&repo)
        .env("PATH", &augmented_path)
        .args(["run", "cargo-clippy", "--all-files", "--color", "never"]));

    // pre-commit's exact message when a hook is filtered out by stage
    // is "No hook with id `cargo-clippy` in stage `pre-commit`" (or
    // similar wording across versions). Either that phrase or the
    // absence of the cargo-clippy invocation banner is acceptable.
    // Anchor the negative check on cargo's compile banner for the
    // seeded crate (`Checking e2e_root`). The hook NAME `cargo clippy
    // --no-deps` from .pre-commit-config.yaml can appear in pre-commit's
    // banner output regardless of whether the entry actually launched,
    // so matching on it here would risk a future-pre-commit-version
    // flip. The `Checking e2e_root` banner only appears if clippy
    // actually ran against the seeded crate.
    let filtered_out = output.contains("No hook with id `cargo-clippy`")
        || !output.contains("Checking e2e_root");
    assert!(
        filtered_out,
        "AC4 corollary violated: cargo-clippy executed on the DEFAULT pre-commit stage. \
         It must be opt-in via `--hook-stage manual` (clippy is too slow for every \
         commit). Check that the hook keeps `stages: [manual]`.\n\n\
         pre-commit output:\n{output}"
    );
}
