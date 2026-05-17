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
#[path = "archive.rs"]
pub mod archive;
#[path = "registry.rs"]
pub mod registry;

use std::path::PathBuf;

pub(crate) const RESOLUTIONS: &[u32] = &[5, 10, 20, 50, 100];

pub fn run() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo"),
    );
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR set by cargo"));

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
        "build/archive.rs",
        "build/registry.rs",
        "src/graph.rs",
    ] {
        println!("cargo:rerun-if-changed={f}");
    }

    std::fs::create_dir_all(out_dir.join("data")).expect("create OUT_DIR/data");

    for n in RESOLUTIONS {
        let gpkg = manifest_dir.join(format!(
            "vendor/eurostat-marnet/marnet_plus_{}km.gpkg",
            n
        ));
        let raw_edges = gpkg_io::iter_edges(&gpkg)
            .unwrap_or_else(|e| panic!("read {}: {e}", gpkg.display()));
        let csr_built = csr::build_csr(&raw_edges);
        let group_vec = groups::assign_groups(&raw_edges, &csr_built, *n);
        let graph = csr_built.into_graph_with_groups(group_vec);
        let archive_path = out_dir.join("data").join(format!("{}km.rkyv", n));
        archive::write_archive(&archive_path, &graph)
            .unwrap_or_else(|e| panic!("write {}: {e}", archive_path.display()));
    }

    registry::write_edge_groups_rs(&out_dir).expect("write edge_groups.rs");
}
