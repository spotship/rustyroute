//! Top-level build orchestration. ENG-4678.

#[path = "gpkg.rs"]
pub mod gpkg;
#[path = "gpkg_io.rs"]
pub mod gpkg_io;
#[path = "geometry.rs"]
pub mod geometry;
#[path = "csr.rs"]
pub mod csr;
#[path = "groups.rs"]
pub mod groups;

use std::path::PathBuf;

pub(crate) const RESOLUTIONS: &[u32] = &[5, 10, 20, 50, 100];

pub fn run() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo"),
    );

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
        "build/gpkg_io.rs",
        "build/geometry.rs",
        "build/csr.rs",
        "build/groups.rs",
        "src/graph.rs",
    ] {
        println!("cargo:rerun-if-changed={f}");
    }

    for n in RESOLUTIONS {
        let gpkg = manifest_dir.join(format!(
            "vendor/eurostat-marnet/marnet_plus_{}km.gpkg",
            n
        ));
        let raw_edges = gpkg_io::iter_edges(&gpkg)
            .unwrap_or_else(|e| panic!("read {}: {e}", gpkg.display()));
        let csr_built = csr::build_csr(&raw_edges);
        let _groups = groups::assign_groups(&raw_edges, &csr_built, *n);
        // (archive write wired in Task 6)
    }
}
