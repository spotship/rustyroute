//! Classify edges into the 13 named groups.
//!
//! 1. For each of 12 known upstream `pass` values, every matching row
//!    belongs to the corresponding public group name.
//! 2. The 13th group `menaiStrait` is derived geometrically: any edge
//!    whose LineString intersects the bbox
//!    `lng ∈ [-4.20, -4.00], lat ∈ [53.13, 53.30]` (closed) using the
//!    Liang-Barsky line-vs-AABB clip test.
//! 3. Empty groups at any resolution are a HARD ERROR — panics with a
//!    message that names the group and resolution.
//! 4. Unknown non-null `pass` values are a HARD ERROR — panics with the
//!    offending value and fid.

use crate::build::csr::CsrBuilt;
use crate::build::geometry::polyline_intersects_bbox;
use crate::build::gpkg::RawEdge;
use crate::graph::GroupEntry;

pub const PASS_GROUPS: &[(&str, &str)] = &[
    ("suez", "suezCanal"),
    ("panama", "panamaCanal"),
    ("malacca", "malaccaStrait"),
    ("gibraltar", "gibraltarStrait"),
    ("dover", "doverStrait"),
    ("bering", "beringStrait"),
    ("magellan", "magellanStrait"),
    ("babelmandeb", "babElMandebStrait"),
    ("kiel", "kielCanal"),
    ("corinth", "corinthCanal"),
    ("northwest", "northwestPassage"),
    ("northeast", "northeastPassage"),
];

pub const MENAI_NAME: &str = "menaiStrait";
pub const MENAI_LNG_MIN: f64 = -4.20;
pub const MENAI_LNG_MAX: f64 = -4.00;
pub const MENAI_LAT_MIN: f64 = 53.13;
pub const MENAI_LAT_MAX: f64 = 53.30;

pub fn assign_groups(raw: &[RawEdge], csr: &CsrBuilt, res_km: u32) -> Vec<GroupEntry> {
    // 13 buckets: 12 pass-tag groups + menaiStrait.
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); PASS_GROUPS.len() + 1];

    for (edge_id, &raw_idx) in csr.raw_edge_index.iter().enumerate() {
        let re = &raw[raw_idx];
        // (a) pass-tag classification (only one tag per row per data inspection)
        if let Some(tag) = re.pass.as_deref() {
            match PASS_GROUPS.iter().position(|(k, _)| *k == tag) {
                Some(i) => buckets[i].push(edge_id as u32),
                None => panic!(
                    "build.rs: unknown pass value `{tag}` for fid={fid} at {res_km}km. \
                     Known tags: {known}. If upstream added a tag, update PASS_GROUPS \
                     and EDGE_GROUPS together.",
                    fid = re.fid,
                    known = PASS_GROUPS
                        .iter()
                        .map(|(k, _)| *k)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            }
        }
        // (b) menaiStrait: geometric bbox intersection (segment-vs-bbox)
        if polyline_intersects_bbox(
            &re.points,
            MENAI_LNG_MIN,
            MENAI_LNG_MAX,
            MENAI_LAT_MIN,
            MENAI_LAT_MAX,
        ) {
            buckets[PASS_GROUPS.len()].push(edge_id as u32);
        }
    }

    // Build GroupEntry list in fixed order: 12 pass-tag groups then menai.
    let names = PASS_GROUPS
        .iter()
        .map(|(_, public)| *public)
        .chain(std::iter::once(MENAI_NAME));
    let mut out: Vec<GroupEntry> = Vec::with_capacity(PASS_GROUPS.len() + 1);
    for (i, name) in names.enumerate() {
        let mut ids = std::mem::take(&mut buckets[i]);
        ids.sort_unstable();
        ids.dedup();
        out.push(GroupEntry {
            name: name.to_string(),
            edge_ids: ids,
        });
    }

    // (c) Non-empty assertions.
    for (i, g) in out.iter().enumerate() {
        if g.edge_ids.is_empty() {
            let how = if i < PASS_GROUPS.len() {
                format!("pass=`{}`", PASS_GROUPS[i].0)
            } else {
                format!(
                    "menaiStrait bbox lng∈[{:.2},{:.2}] lat∈[{:.2},{:.2}]",
                    MENAI_LNG_MIN, MENAI_LNG_MAX, MENAI_LAT_MIN, MENAI_LAT_MAX
                )
            };
            panic!(
                "build.rs: edge group `{name}` is empty at {res_km}km resolution.\n\
                 This indicates upstream data drift or a bbox/tag mismatch.\n\
                 Match rule: {how}.\n\
                 Inspect vendor/eurostat-marnet/marnet_plus_{res_km}km.gpkg\n\
                 (e.g. `python3 -c \"import sqlite3; ...\"`) to verify rows still exist.\n\
                 Removing or renaming a chokepoint edge in the vendored .gpkg is a \
                 build-blocking change — restore the bytes or update the spec.",
                name = g.name,
                res_km = res_km,
                how = how,
            );
        }
    }

    out
}
