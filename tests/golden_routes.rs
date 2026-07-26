//! ENG-4681 golden-route integration tests. Pins the absolute sea
//! distance of named real-world routes at multiple grid resolutions so
//! any future shift in graph data, the CSR build, snapping, or the
//! Dijkstra cost model is caught as a regression.
//!
//! Expected distances are pinned to the library's CURRENT output (the
//! golden baseline), with per-row real-world references and sources in
//! `tests/fixtures/routes.json`. See the ENG-4681 spec for why three of
//! the ticket's estimated km were recalibrated (§2a) and why the Menai
//! route uses Anglesey-straddling coordinates instead of Liverpool->Dublin
//! (§2b).
//!
//! The gate is `wasm32`-only, deliberately: unlike `route_smoke.rs` and
//! `env_data_dir.rs` (which name `data::BYTES_50KM`, a feature-gated
//! symbol), this binary only calls `Graph::load` — resolution-order step
//! 2, `$OUT_DIR/data/{N}km.rkyv`, which `build/mod.rs` writes for every
//! entry in `RESOLUTIONS` regardless of which `data-*` features are on.
//! So no data feature is required, and the goldens stay available under
//! partial feature sets. `Graph::load` itself is
//! `#[cfg(not(target_arch = "wasm32"))]`, which is what the gate tracks.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashSet;
use std::sync::OnceLock;

use rustyroute::{EdgeId, Graph};
use serde::Deserialize;

/// Grid resolutions rustyroute ships data for. `graph()` has one cache slot
/// per entry; `fixtures_parse_and_are_wellformed` rejects any fixture row
/// declaring a resolution outside this set.
const SUPPORTED_RESOLUTIONS: [u32; 5] = [5, 10, 20, 50, 100];

#[derive(Debug, Deserialize)]
struct Fixtures {
    routes: Vec<RouteFixture>,
}

/// One golden row. `expected_km`/`tol`/`tol_100km` are `Option` because the
/// Menai baseline+blocked pair assert an inequality, not an absolute, and
/// carry `null`. Serde cannot express "all set or all null", so
/// `fixtures_parse_and_are_wellformed` enforces that pairing instead.
/// Documentation-only JSON fields (`name`, `real_world_km`, `source`) are
/// ignored by serde and deliberately not modelled here.
#[derive(Debug, Deserialize)]
struct RouteFixture {
    key: String,
    from: [f64; 2],
    to: [f64; 2],
    blocked: Vec<String>,
    resolutions: Vec<u32>,
    expected_km: Option<f64>,
    tol: Option<f64>,
    tol_100km: Option<f64>,
}

/// Parse `tests/fixtures/routes.json` once for the whole test binary.
/// `include_str!` embeds the fixture at compile time, so the test does not
/// depend on the process working directory.
fn fixtures() -> &'static Fixtures {
    static F: OnceLock<Fixtures> = OnceLock::new();
    F.get_or_init(|| {
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/routes.json"
        ));
        serde_json::from_str(raw).expect("parse tests/fixtures/routes.json")
    })
}

/// Look up a golden row by key.
fn fixture(key: &str) -> &'static RouteFixture {
    fixtures()
        .routes
        .iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no fixture row with key `{key}`"))
}

/// Load (and cache) the graph for a resolution. `Graph::load(n)` reads
/// `$OUT_DIR/data/{n}km.rkyv`, which `build.rs` writes for every
/// resolution, so every n in {5,10,20,50,100} resolves under any feature
/// set — including `--no-default-features`.
fn graph(res: u32) -> &'static Graph {
    static G5: OnceLock<Graph> = OnceLock::new();
    static G10: OnceLock<Graph> = OnceLock::new();
    static G20: OnceLock<Graph> = OnceLock::new();
    static G50: OnceLock<Graph> = OnceLock::new();
    static G100: OnceLock<Graph> = OnceLock::new();
    let slot = match res {
        5 => &G5,
        10 => &G10,
        20 => &G20,
        50 => &G50,
        100 => &G100,
        other => panic!("unsupported resolution {other}km"),
    };
    slot.get_or_init(|| Graph::load(res).unwrap_or_else(|e| panic!("Graph::load({res}): {e:?}")))
}

/// Distance for a fixture route at a given resolution, resolving the row's
/// blocked group names to an `EdgeId` set.
fn distance_at(row: &RouteFixture, res: u32) -> f64 {
    let g = graph(res);
    let blocked: HashSet<EdgeId> = if row.blocked.is_empty() {
        HashSet::new()
    } else {
        g.edges_for_groups(row.blocked.iter().map(String::as_str))
            .unwrap_or_else(|e| panic!("edges_for_groups({:?}): {e:?}", row.blocked))
    };
    g.route((row.from[0], row.from[1]), (row.to[0], row.to[1]), &blocked)
        .unwrap_or_else(|e| panic!("route {} @ {res}km: {e:?}", row.key))
        .distance_km
}

/// Relative-error tolerance check (ticket-mandated form).
fn within(dist: f64, expected: f64, tol: f64) -> bool {
    (dist - expected).abs() / expected < tol
}

/// Tolerance for a row at a given resolution: `tol_100km` at 100 km,
/// else `tol`.
fn tol_for(row: &RouteFixture, res: u32) -> f64 {
    if res == 100 {
        row.tol_100km.or(row.tol).expect("tol_100km or tol")
    } else {
        row.tol.expect("tol")
    }
}

/// Assert a distance-pinned golden at every resolution it declares.
fn assert_golden(key: &str) {
    let row = fixture(key);
    let expected = row.expected_km.expect("expected_km for a pinned golden");
    for &res in &row.resolutions {
        let dist = distance_at(row, res);
        let tol = tol_for(row, res);
        assert!(
            within(dist, expected, tol),
            "golden {key} @ {res}km: dist={dist:.1} expected={expected:.1} tol={tol}"
        );
    }
}

#[test]
fn fixtures_parse_and_are_wellformed() {
    let keys: Vec<&str> = fixtures().routes.iter().map(|r| r.key.as_str()).collect();
    for k in [
        "marseille_shanghai_suez",
        "marseille_shanghai_cape",
        "rotterdam_new_york",
        "singapore_yokohama",
        "hamburg_self",
        "menai_allowed",
        "menai_blocked",
    ] {
        assert!(keys.contains(&k), "fixture missing key `{k}`");
    }
    // Keys must be unique — a duplicate would let one row silently shadow
    // another (lookups return the first match).
    let mut seen = HashSet::new();
    for r in &fixtures().routes {
        assert!(
            seen.insert(r.key.as_str()),
            "duplicate fixture key `{}`",
            r.key
        );
    }
    for r in &fixtures().routes {
        // Every row must declare at least one resolution, and only supported
        // ones — otherwise a golden test could pass without routing anything
        // (empty sweep) or panic in `graph()` (unsupported resolution).
        assert!(
            !r.resolutions.is_empty(),
            "fixture `{}` has an empty `resolutions` list",
            r.key
        );
        for &res in &r.resolutions {
            assert!(
                SUPPORTED_RESOLUTIONS.contains(&res),
                "fixture `{}` declares unsupported resolution {res}km",
                r.key
            );
        }
        // Coordinates rounded to 4 decimals (ticket requirement).
        for v in [r.from[0], r.from[1], r.to[0], r.to[1]] {
            let scaled = v * 10_000.0;
            assert!(
                (scaled - scaled.round()).abs() < 1e-6,
                "coord {v} in `{}` not rounded to 4 decimals",
                r.key
            );
        }
        // Distance-schema invariants. The two row classes are disjoint: a
        // *pinned* row carries `expected_km` and is asserted absolutely by
        // `assert_golden`; an *inequality-only* row carries all three
        // distance fields as null and is only ever compared against a
        // sibling. A half-populated row would surface as a panic deep in
        // `tol_for`/`within` inside some unrelated test, naming neither the
        // fixture nor the missing field — so reject it here, where the
        // message can point straight at the offending row.
        // `expected_km` and `tol` are what make a row *pinned*; neither is
        // meaningful alone, so they must appear together or not at all.
        assert!(
            r.expected_km.is_some() == r.tol.is_some(),
            "fixture `{}`: `expected_km` and `tol` must be set together",
            r.key
        );
        // An inequality-only row carries no distance fields at all — a
        // stray `tol_100km` would be dead config that reads as a golden.
        assert!(
            r.expected_km.is_some() || r.tol_100km.is_none(),
            "fixture `{}` has no `expected_km`, so `tol_100km` must be null",
            r.key
        );
        if let Some(expected) = r.expected_km {
            assert!(
                expected.is_finite() && expected >= 0.0,
                "fixture `{}` has a non-finite or negative `expected_km` {expected}",
                r.key
            );
            if expected == 0.0 {
                // `within` divides by `expected`, so a zero golden can only
                // be asserted exactly (see `hamburg_self_is_zero`) — and
                // zero km is only meaningful when both endpoints are
                // literally the same point.
                assert!(
                    r.from == r.to,
                    "fixture `{}` pins `expected_km` 0 but its endpoints differ",
                    r.key
                );
            } else {
                // `within` compares with a strict `<`, so a zero tolerance
                // on a non-zero golden could never pass.
                assert!(
                    r.tol.is_some_and(|t| t > 0.0),
                    "fixture `{}` pins a non-zero `expected_km`, so `tol` must be > 0",
                    r.key
                );
            }
        }
    }
}

/// Marseille -> Shanghai via the Suez Canal, all five resolutions.
/// expected 16,354 km (spec §5); ±1% at 5/10/20/50, ±5% at 100 km.
#[test]
fn marseille_shanghai_suez() {
    assert_golden("marseille_shanghai_suez");
}

/// Marseille -> Shanghai with `suezCanal` blocked (round the Cape of Good
/// Hope), all five resolutions. expected 25,048 km; ±2% at 5/10/20/50,
/// ±5% at 100 km.
#[test]
fn marseille_shanghai_cape() {
    assert_golden("marseille_shanghai_cape");
}

/// Rotterdam -> New York at 50 km, ±1%. expected 6,185 km (spec §5).
#[test]
fn rotterdam_new_york() {
    assert_golden("rotterdam_new_york");
}

/// Singapore -> Yokohama at 50 km, ±1%. expected 5,523 km (spec §5).
#[test]
fn singapore_yokohama() {
    assert_golden("singapore_yokohama");
}

/// Hamburg -> Hamburg self-route at 50 km. `from == to` snaps to one node
/// and returns exactly 0.0 (src/loader.rs:443-449). The relative-error
/// form would divide by zero, so assert exact equality here.
#[test]
fn hamburg_self_is_zero() {
    let row = fixture("hamburg_self");
    let dist = distance_at(row, 50);
    assert_eq!(
        dist, 0.0,
        "hamburg self-route must be exactly 0.0, got {dist}"
    );
}

/// Menai baseline (open) — Conwy Bay -> Cardigan Bay round Anglesey. Pins
/// that the open route exists and is positive at every resolution; the
/// absolute distance is the baseline for `menai_blocked_strictly_longer`.
#[test]
fn menai_allowed_baseline() {
    let row = fixture("menai_allowed");
    for &res in &row.resolutions {
        let dist = distance_at(row, res);
        assert!(
            dist > 0.0,
            "menai baseline @ {res}km must be > 0, got {dist}"
        );
    }
}

/// Blocking `menaiStrait` forces the detour around Anglesey, so the route is
/// strictly longer than the open baseline at every resolution (ticket
/// acceptance: route(blocked={menaiStrait}) > route(blocked={})).
#[test]
fn menai_blocked_strictly_longer() {
    let open = fixture("menai_allowed");
    let blocked = fixture("menai_blocked");
    // The comparison is only meaningful if both rows describe the SAME route
    // and differ solely in the blocked set. Guard against a fixture edit that
    // silently diverges them (endpoints or resolution sweep), which would let
    // this test compare two different routes and pass/fail for the wrong
    // reason.
    assert_eq!(
        open.from, blocked.from,
        "menai rows must share the `from` endpoint"
    );
    assert_eq!(
        open.to, blocked.to,
        "menai rows must share the `to` endpoint"
    );
    assert_eq!(
        open.resolutions, blocked.resolutions,
        "menai rows must share the resolution sweep"
    );
    assert!(
        open.blocked.is_empty(),
        "menai_allowed baseline must have no blocked groups"
    );
    assert!(
        !blocked.blocked.is_empty(),
        "menai_blocked must block at least one group"
    );
    for &res in &blocked.resolutions {
        let open_km = distance_at(open, res);
        let blocked_km = distance_at(blocked, res);
        assert!(
            blocked_km > open_km,
            "menai @ {res}km: blocked={blocked_km:.3} must be strictly > open={open_km:.3}"
        );
    }
}
