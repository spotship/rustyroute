//! Unit tests for build/groups.rs — exercises the empty-group panic
//! and the pass-tag-to-public-name mapping with synthetic inputs.

#[path = "../build/geometry.rs"]
pub mod geometry;
#[path = "../build/gpkg.rs"]
pub mod gpkg;
#[path = "../build/csr.rs"]
pub mod csr;
#[path = "../build/groups.rs"]
mod groups;
#[path = "../src/graph.rs"]
pub mod graph;

// The four modules above all need each other; the `#[path]` imports
// wire them into this test crate's namespace. Inside groups.rs the
// real crate uses `crate::build::geometry`; in this test build the
// module is just `geometry` (no `build` parent). To keep the source
// of `groups.rs` untouched, this test uses the alias pattern:
//
//   mod build { pub use super::{geometry, gpkg, csr}; }
//
// so `crate::build::geometry` resolves correctly.
mod build {
    pub use super::{csr, geometry, gpkg};
}

use crate::gpkg::RawEdge;
use crate::graph::GroupEntry;

fn synth_raw(fid: i64, pass: Option<&str>, points: Vec<(f64, f64)>) -> RawEdge {
    RawEdge {
        fid,
        pass: pass.map(str::to_string),
        points,
    }
}

#[test]
#[should_panic(expected = "edge group `suezCanal` is empty")]
fn empty_group_panics_with_clear_message() {
    // Synthetic input with all groups EXCEPT suez present.
    // We construct one edge per pass tag (and one Menai edge), but no Suez row.
    let mut raw: Vec<RawEdge> = Vec::new();
    let known_tags = [
        ("panama", (-79.5, 9.0, -79.6, 9.0)),
        ("malacca", (103.0, 1.5, 103.1, 1.5)),
        ("gibraltar", (-5.5, 35.9, -5.4, 35.95)),
        ("dover", (1.4, 51.0, 1.5, 51.05)),
        ("bering", (-169.0, 65.5, -168.9, 65.55)),
        ("magellan", (-71.0, -53.5, -70.9, -53.45)),
        ("babelmandeb", (43.2, 12.6, 43.3, 12.65)),
        ("kiel", (9.95, 53.95, 9.96, 53.96)),
        ("corinth", (22.95, 37.95, 22.96, 37.96)),
        ("northwest", (-95.0, 73.0, -94.9, 73.05)),
        ("northeast", (60.0, 73.0, 60.1, 73.05)),
    ];
    let mut fid = 1i64;
    for (tag, (a, b, c, d)) in known_tags {
        raw.push(synth_raw(fid, Some(tag), vec![(a, b), (c, d)]));
        fid += 1;
    }
    // Menai-strait edge inside the bbox
    raw.push(synth_raw(fid, None, vec![(-4.1, 53.2), (-4.05, 53.25)]));

    let csr = csr::build_csr(&raw);
    // Should panic: suez is missing.
    let _ = groups::assign_groups(&raw, &csr, 100);
}

#[test]
fn happy_path_assigns_thirteen_groups() {
    let mut raw: Vec<RawEdge> = Vec::new();
    let known_tags = [
        ("suez", (32.5, 30.0, 32.6, 30.05)),
        ("panama", (-79.5, 9.0, -79.6, 9.0)),
        ("malacca", (103.0, 1.5, 103.1, 1.5)),
        ("gibraltar", (-5.5, 35.9, -5.4, 35.95)),
        ("dover", (1.4, 51.0, 1.5, 51.05)),
        ("bering", (-169.0, 65.5, -168.9, 65.55)),
        ("magellan", (-71.0, -53.5, -70.9, -53.45)),
        ("babelmandeb", (43.2, 12.6, 43.3, 12.65)),
        ("kiel", (9.95, 53.95, 9.96, 53.96)),
        ("corinth", (22.95, 37.95, 22.96, 37.96)),
        ("northwest", (-95.0, 73.0, -94.9, 73.05)),
        ("northeast", (60.0, 73.0, 60.1, 73.05)),
    ];
    let mut fid = 1i64;
    for (tag, (a, b, c, d)) in known_tags {
        raw.push(synth_raw(fid, Some(tag), vec![(a, b), (c, d)]));
        fid += 1;
    }
    raw.push(synth_raw(fid, None, vec![(-4.1, 53.2), (-4.05, 53.25)]));

    let csr = csr::build_csr(&raw);
    let groups = groups::assign_groups(&raw, &csr, 100);
    assert_eq!(groups.len(), 13);
    let names: Vec<&str> = groups
        .iter()
        .map(|g: &GroupEntry| g.name.as_str())
        .collect();
    assert_eq!(names[0], "suezCanal");
    assert_eq!(names[12], "menaiStrait");
    for g in &groups {
        assert!(
            !g.edge_ids.is_empty(),
            "group {} empty in happy-path test",
            g.name
        );
    }
}

#[test]
#[should_panic(expected = "unknown pass value")]
fn unknown_pass_panics() {
    let mut raw: Vec<RawEdge> = Vec::new();
    // Include all 12 known tags so they don't panic first.
    let known_tags = [
        ("suez", (32.5, 30.0, 32.6, 30.05)),
        ("panama", (-79.5, 9.0, -79.6, 9.0)),
        ("malacca", (103.0, 1.5, 103.1, 1.5)),
        ("gibraltar", (-5.5, 35.9, -5.4, 35.95)),
        ("dover", (1.4, 51.0, 1.5, 51.05)),
        ("bering", (-169.0, 65.5, -168.9, 65.55)),
        ("magellan", (-71.0, -53.5, -70.9, -53.45)),
        ("babelmandeb", (43.2, 12.6, 43.3, 12.65)),
        ("kiel", (9.95, 53.95, 9.96, 53.96)),
        ("corinth", (22.95, 37.95, 22.96, 37.96)),
        ("northwest", (-95.0, 73.0, -94.9, 73.05)),
        ("northeast", (60.0, 73.0, 60.1, 73.05)),
    ];
    let mut fid = 1i64;
    for (tag, (a, b, c, d)) in known_tags {
        raw.push(synth_raw(fid, Some(tag), vec![(a, b), (c, d)]));
        fid += 1;
    }
    raw.push(synth_raw(fid, None, vec![(-4.1, 53.2), (-4.05, 53.25)]));
    raw.push(synth_raw(
        fid + 1,
        Some("madeup"),
        vec![(0.0, 0.0), (0.1, 0.1)],
    ));

    let csr = csr::build_csr(&raw);
    let _ = groups::assign_groups(&raw, &csr, 100);
}
