//! Top-level build orchestration. ENG-4678.

#[path = "gpkg.rs"]
pub mod gpkg;
#[path = "geometry.rs"]
pub mod geometry;
#[path = "csr.rs"]
pub mod csr;

use std::path::PathBuf;

pub(crate) const RESOLUTIONS: &[u32] = &[5, 10, 20, 50, 100];

pub fn run() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo"),
    );

    // Rerun directives — change these if you add new build-time inputs.
    for n in RESOLUTIONS {
        println!(
            "cargo:rerun-if-changed=vendor/eurostat-marnet/marnet_plus_{}km.gpkg",
            n
        );
    }
    for f in &[
        "build.rs",
        "build/mod.rs",
        "build/gpkg.rs",
        "build/geometry.rs",
        "build/csr.rs",
        "src/graph.rs",
    ] {
        println!("cargo:rerun-if-changed={f}");
    }

    // Read + CSR each resolution; output writing is added in Task 6.
    for n in RESOLUTIONS {
        let gpkg = manifest_dir.join(format!(
            "vendor/eurostat-marnet/marnet_plus_{}km.gpkg",
            n
        ));
        let raw_edges = gpkg::iter_edges(&gpkg)
            .unwrap_or_else(|e| panic!("read {}: {e}", gpkg.display()));
        let _csr_built = csr::build_csr(&raw_edges);
        // (group assignment + archive write wired in later tasks)
    }
}
