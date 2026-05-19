//! ENG-4679 E2E: prove the crate compiles cleanly under the full
//! Cargo feature matrix called out in the spec.
//!
//! The spec describes three feature shapes:
//! 1. Default features (`data-50km` on) — exercised by `cargo test`.
//! 2. `--no-default-features` (no data features) — the "downstream
//!    consumer with no data baked in" published-crate case.
//! 3. `--no-default-features --features data-100km` (partial features)
//!    — exercises AC4's "load(100) succeeds, load(50) is
//!    DataNotAvailable" matrix.
//!
//! The dev phase only exercises shape #1 directly. Shapes #2 and #3 are
//! described in the spec's AC3/AC4 commentary but never actually
//! compiled in CI. This test shells out to `cargo check` so any
//! future change that breaks the feature gating (a missing
//! `#[cfg(feature = "data-50km")]` guard, a non-default-feature
//! reference to a gated const, etc.) trips a test failure rather than
//! a CI surprise.
//!
//! `cargo check` is used instead of `cargo build` to keep the runtime
//! within seconds — we only need the type-check pass, not codegen.
//! A dedicated `CARGO_TARGET_DIR` keeps the recursive cargo from
//! contending with the outer build on `target/`.

#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;
use std::process::Command;

fn run_cargo_check(extra_args: &[&str]) -> std::process::Output {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = std::env::var("OUT_DIR")
        .map(|s| PathBuf::from(s).join("feature_matrix_target"))
        .unwrap_or_else(|_| std::env::temp_dir().join("rustyroute_feature_matrix_target"));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());

    let mut cmd = Command::new(&cargo);
    cmd.arg("check")
        .arg("--manifest-path")
        .arg(manifest.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--tests")
        .env("CARGO_TERM_COLOR", "never");
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.output().expect("spawn cargo check")
}

#[test]
fn cargo_check_no_default_features() {
    let out = run_cargo_check(&["--no-default-features"]);
    if !out.status.success() {
        panic!(
            "cargo check --no-default-features failed (exit {:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

#[test]
fn cargo_check_partial_feature_data_100km_only() {
    let out = run_cargo_check(&["--no-default-features", "--features", "data-100km"]);
    if !out.status.success() {
        panic!(
            "cargo check --no-default-features --features data-100km failed (exit {:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

#[test]
fn cargo_check_all_data_features_via_cli_meta() {
    // The `cli` meta-feature enables all five data resolutions. This
    // also exercises the case where all five BYTES_{N}KM consts are
    // simultaneously compiled in.
    let out = run_cargo_check(&["--no-default-features", "--features", "cli"]);
    if !out.status.success() {
        panic!(
            "cargo check --no-default-features --features cli failed (exit {:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}
