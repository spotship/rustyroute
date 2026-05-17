//! Unit tests for the build-script geometry helpers, compiled into a
//! normal test binary so `cargo test` runs them. The helpers
//! themselves live under `build/` and are also compiled into
//! `build.rs`; this file re-includes them via #[path].

#[path = "../build/geometry.rs"]
mod geometry;

use geometry::{haversine_km, liang_barsky_intersects, parse_gpb_linestring};

fn synth_two_point_be(lng0: f64, lat0: f64, lng1: f64, lat1: f64) -> Vec<u8> {
    let mut blob = Vec::with_capacity(8 + 32 + 1 + 4 + 4 + 32);
    blob.extend_from_slice(b"GP");
    blob.push(0x00); // version
    blob.push(0x02); // flags: BE header, env_indicator=1, not empty
    blob.extend_from_slice(&4326u32.to_be_bytes());
    let xmin = lng0.min(lng1);
    let xmax = lng0.max(lng1);
    let ymin = lat0.min(lat1);
    let ymax = lat0.max(lat1);
    blob.extend_from_slice(&xmin.to_be_bytes());
    blob.extend_from_slice(&xmax.to_be_bytes());
    blob.extend_from_slice(&ymin.to_be_bytes());
    blob.extend_from_slice(&ymax.to_be_bytes());
    // WKB
    blob.push(0x00); // big endian
    blob.extend_from_slice(&2u32.to_be_bytes()); // LineString
    blob.extend_from_slice(&2u32.to_be_bytes()); // n points
    blob.extend_from_slice(&lng0.to_be_bytes());
    blob.extend_from_slice(&lat0.to_be_bytes());
    blob.extend_from_slice(&lng1.to_be_bytes());
    blob.extend_from_slice(&lat1.to_be_bytes());
    blob
}

#[test]
fn parses_be_two_point_linestring() {
    let blob = synth_two_point_be(-4.5, 53.2, -4.0, 53.3);
    let pts = parse_gpb_linestring(&blob).expect("parse");
    assert_eq!(pts.len(), 2);
    assert!((pts[0].0 - (-4.5)).abs() < 1e-12);
    assert!((pts[0].1 - 53.2).abs() < 1e-12);
    assert!((pts[1].0 - (-4.0)).abs() < 1e-12);
    assert!((pts[1].1 - 53.3).abs() < 1e-12);
}

#[test]
fn haversine_known_distance() {
    // London (51.5074, -0.1278) -> Paris (48.8566, 2.3522) ~ 343 km
    let d = haversine_km(-0.1278, 51.5074, 2.3522, 48.8566);
    assert!((d - 343.0).abs() < 5.0, "got {d}");
}

#[test]
fn liang_barsky_segment_in_bbox() {
    assert!(liang_barsky_intersects(
        -4.1, 53.2, -4.05, 53.25, -4.20, -4.00, 53.13, 53.30
    ));
}

#[test]
fn liang_barsky_segment_outside_bbox() {
    assert!(!liang_barsky_intersects(
        -4.1, 60.0, -4.05, 60.1, -4.20, -4.00, 53.13, 53.30
    ));
}

#[test]
fn liang_barsky_crossing_segment() {
    // The 100km menai edge: both endpoints outside but segment crosses bbox.
    // This is the case the empty-group panic would surface if we used a
    // weaker test (endpoint-in-bbox, midpoint-in-bbox) — see spec §
    // "Menai Strait derivation".
    assert!(liang_barsky_intersects(
        -5.0062109375,
        52.792460937499996,
        -4.002734375,
        53.30314453125,
        -4.20,
        -4.00,
        53.13,
        53.30
    ));
}

#[test]
fn liang_barsky_endpoint_outside_no_cross() {
    // Both endpoints north of bbox; segment doesn't cross.
    assert!(!liang_barsky_intersects(
        -4.1, 53.31, -4.05, 53.35, -4.20, -4.00, 53.13, 53.30
    ));
}
