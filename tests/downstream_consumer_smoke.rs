//! ENG-4679: prove the published-crate distribution model by spawning
//! a fresh `cargo test` against the path-dep sub-package.
//!
//! Uses a dedicated `CARGO_TARGET_DIR` so the recursive cargo
//! invocation doesn't contend with the outer build on `target/`.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn downstream_consumer_subpackage_passes() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sub_manifest = manifest.join("tests/downstream_consumer/Cargo.toml");
    let target_dir = std::env::var("OUT_DIR")
        .map(|s| PathBuf::from(s).join("downstream_consumer_target"))
        .unwrap_or_else(|_| std::env::temp_dir().join("rustyroute_downstream_consumer_target"));

    // Use the same cargo that's running this test if CARGO is set.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());

    let status = Command::new(&cargo)
        .arg("test")
        .arg("--manifest-path")
        .arg(&sub_manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        // Allow CI to surface the sub-package's own stdout/stderr.
        .env("CARGO_TERM_COLOR", "never")
        .status()
        .expect("spawn cargo test");

    assert!(
        status.success(),
        "downstream_consumer sub-package cargo test failed (exit {status:?})"
    );
}
