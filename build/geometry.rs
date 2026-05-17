//! Parse GeoPackage Binary (GPB) + WKB LineString, compute haversine
//! distances, and clip line segments against an axis-aligned bbox via
//! Liang-Barsky.
//!
//! All vendored MARNET geometries are big-endian (GPB flag 0x02; WKB
//! byte 0 = 0x00). The parser still reads `envelope_indicator` from
//! flags to compute the WKB offset generically — robust against any
//! future re-vendor that uses a different envelope shape.

/// Earth radius (km). The IUGG mean radius — sufficient precision for a
/// 5 km grid.
pub const EARTH_RADIUS_KM: f64 = 6371.0088;

pub fn parse_gpb_linestring(blob: &[u8]) -> Result<Vec<(f64, f64)>, String> {
    if blob.len() < 8 || &blob[0..2] != b"GP" {
        return Err("missing GP magic".into());
    }
    // Flags byte (byte 3):
    //   bit 0: WKB byte order (0=BE, 1=LE) — applies to BOTH the GPB
    //          envelope and the WKB body per GeoPackage spec.
    //   bits 1-3: envelope indicator (0=none, 1=XY, 2=XYZ, 3=XYM, 4=XYZM)
    //   bit 4: empty flag (1=empty)
    let flags = blob[3];
    if (flags & 0x01) != 0 {
        return Err("vendored MARNET data is expected to be big-endian; \
                    got little-endian flag bit. Re-vendor or update parser."
            .into());
    }
    if (flags & 0x10) != 0 {
        // Empty-geometry flag: WKB body may be absent or carry a zero
        // point count. Reject up-front so the caller gets a clear,
        // deterministic error instead of an opaque "blob too short" or
        // empty point list further down.
        return Err("GPB empty flag set; expected non-empty LineString".into());
    }
    let env_indicator = ((flags >> 1) & 0x07) as usize;
    let env_floats: usize = match env_indicator {
        0 => 0,
        1 => 4,
        2 => 6,
        3 => 6,
        4 => 8,
        _ => return Err(format!("invalid envelope_indicator {env_indicator}")),
    };
    let wkb_off = 8 + env_floats * 8;

    if blob.len() < wkb_off + 9 {
        return Err("blob shorter than WKB header".into());
    }
    // WKB body
    let wkb_order = blob[wkb_off];
    if wkb_order != 0 {
        return Err("WKB body is little-endian; this codebase expects big-endian".into());
    }
    let wkb_type = u32::from_be_bytes(blob[wkb_off + 1..wkb_off + 5].try_into().unwrap());
    if wkb_type != 2 {
        return Err(format!("expected LineString (type=2), got {wkb_type}"));
    }
    let npts = u32::from_be_bytes(blob[wkb_off + 5..wkb_off + 9].try_into().unwrap()) as usize;
    let coord_off = wkb_off + 9;
    let need = coord_off + npts * 16;
    if blob.len() < need {
        return Err(format!("blob too short: need {need}, got {}", blob.len()));
    }
    let mut pts = Vec::with_capacity(npts);
    for i in 0..npts {
        let off = coord_off + i * 16;
        let lng = f64::from_be_bytes(blob[off..off + 8].try_into().unwrap());
        let lat = f64::from_be_bytes(blob[off + 8..off + 16].try_into().unwrap());
        pts.push((lng, lat));
    }
    Ok(pts)
}

pub fn haversine_km(lng1: f64, lat1: f64, lng2: f64, lat2: f64) -> f64 {
    let (lat1_r, lat2_r) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + lat1_r.cos() * lat2_r.cos() * (dlng / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().asin()
}

/// Polyline-summed haversine: cumulative distance along consecutive
/// vertex pairs.
#[allow(dead_code)] // used by build/csr.rs; not by every test #[path] re-include
pub fn polyline_length_km(points: &[(f64, f64)]) -> f64 {
    points
        .windows(2)
        .map(|w| haversine_km(w[0].0, w[0].1, w[1].0, w[1].1))
        .sum()
}

/// Liang-Barsky line-vs-AABB intersection test. Returns true iff the
/// segment from (p0x,p0y) to (p1x,p1y) intersects the axis-aligned
/// bounding box [xmin,xmax] × [ymin,ymax] (closed).
#[allow(clippy::too_many_arguments)]
pub fn liang_barsky_intersects(
    p0x: f64,
    p0y: f64,
    p1x: f64,
    p1y: f64,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
) -> bool {
    let dx = p1x - p0x;
    let dy = p1y - p0y;
    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;
    let edges = [
        (-dx, p0x - xmin),
        (dx, xmax - p0x),
        (-dy, p0y - ymin),
        (dy, ymax - p0y),
    ];
    for &(p, q) in &edges {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return false;
                }
                if r > t0 {
                    t0 = r;
                }
            } else {
                if r < t0 {
                    return false;
                }
                if r < t1 {
                    t1 = r;
                }
            }
        }
    }
    t0 <= t1
}

/// True iff any segment of `points` intersects the bbox.
#[allow(dead_code)] // used by build/groups.rs; not by every test #[path] re-include
pub fn polyline_intersects_bbox(
    points: &[(f64, f64)],
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
) -> bool {
    points
        .windows(2)
        .any(|w| liang_barsky_intersects(w[0].0, w[0].1, w[1].0, w[1].1, xmin, xmax, ymin, ymax))
}

// Tests for this module live in `tests/build_helpers.rs` (see Task 4
// Step 1). They re-include this file via #[path] so they can exercise
// the helpers without dragging in rusqlite or the rest of build/.
