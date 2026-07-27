//! ENG-4682: end-to-end tests for the `rustyroute` CLI binary.
//!
//! Gated on the `cli` feature because the binary only exists then (its
//! `[[bin]]` target carries `required-features = ["cli"]`). When the
//! feature is off the whole file is stripped before macro expansion, so
//! the `CARGO_BIN_EXE_rustyroute` lookup below is never evaluated.
//!
//! Cargo builds bin targets whose required features are satisfied
//! before running integration tests, which is what makes
//! `CARGO_BIN_EXE_<name>` resolve to a real executable.
//!
//! No JSON parser dev-dependency — same convention as
//! tests/ci_workflow.rs and tests/release_dist_workflow.rs, which assert
//! on structure with string checks.

#![cfg(feature = "cli")]

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_rustyroute");

/// Marseille and Shanghai — the ticket's worked example. Far apart, and
/// connected in the MARNET graph at every resolution.
const MARSEILLE: &str = "43.30,5.37";
const SHANGHAI: &str = "31.23,121.47";

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {BIN}: {e}"))
}

fn code(out: &Output) -> i32 {
    out.status
        .code()
        .expect("rustyroute exited via signal, not a status code")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// AC3: default format is GeoJSON, exit 0, with the documented
/// properties.
#[test]
fn geojson_default_marseille_to_shanghai() {
    let out = run(&["route", "--from", MARSEILLE, "--to", SHANGHAI]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let s = stdout(&out);
    for needle in [
        "\"type\":\"FeatureCollection\"",
        "\"type\":\"Feature\"",
        "\"type\":\"LineString\"",
        "\"coordinates\":[[",
        "\"distance_km\":",
        "\"resolution\":50",
    ] {
        assert!(s.contains(needle), "missing {needle} in stdout: {s}");
    }
}

/// AC4: the `cli` feature enables data-100km, so a non-default
/// resolution resolves from the static slices.
#[test]
fn resolution_100_is_available() {
    let out = run(&[
        "route",
        "--resolution",
        "100",
        "--from",
        MARSEILLE,
        "--to",
        SHANGHAI,
    ]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("\"resolution\":100"));
}

/// AC5: an unknown `--block` group exits 1 and lists the valid names on
/// stderr.
#[test]
fn unknown_block_group_exits_1_and_lists_names() {
    let out = run(&["route", "--from", "0,0", "--to", "0,0", "--block", "foo"]);
    assert_eq!(code(&out), 1, "stdout: {}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("foo"), "stderr must name the offender: {err}");
    // Every baked-in group must be offered, not just a couple.
    for name in rustyroute::EDGE_GROUPS {
        assert!(err.contains(name), "stderr must list `{name}`: {err}");
    }
}

/// A valid group is accepted (guards against the pre-flight check
/// rejecting everything).
#[test]
fn known_block_group_is_accepted() {
    let out = run(&[
        "route",
        "--from",
        MARSEILLE,
        "--to",
        SHANGHAI,
        "--block",
        "suezCanal",
    ]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
}

/// `--block` entries are trimmed, and empty ones mean "block nothing"
/// rather than "unknown group".
///
/// `value_delimiter = ','` splits on the comma alone, so the natural
/// shell form `--block "suezCanal, menaiStrait"` leaves a leading space
/// on every name after the first — without trimming those are rejected
/// as unknown groups. Empty entries arise from `--block ""`, a trailing
/// comma, and whitespace-only values.
#[test]
fn block_entries_are_trimmed_and_empties_ignored() {
    for arg in [
        "",
        "   ",
        "suezCanal,",
        " suezCanal ",
        "suezCanal, menaiStrait",
    ] {
        let out = run(&[
            "route", "--from", MARSEILLE, "--to", SHANGHAI, "--block", arg,
        ]);
        assert_eq!(
            code(&out),
            0,
            "`--block {arg:?}` should be accepted. stderr: {}",
            stderr(&out)
        );
    }
}

/// AC6: `--format line` emits one `lng,lat` per line.
///
/// The magnitude assertions are the regression guard for the
/// `(lat, lng)` -> `lng, lat` swap: Marseille is lat ~43, lng ~5, so a
/// transposed first line would fail both bounds.
#[test]
fn format_line_emits_lng_lat_pairs() {
    let out = run(&[
        "route", "--from", MARSEILLE, "--to", SHANGHAI, "--format", "line",
    ]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let s = stdout(&out);
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(lines.len() >= 2, "expected a multi-point path, got: {s}");

    for line in &lines {
        let (lng, lat) = line
            .split_once(',')
            .unwrap_or_else(|| panic!("line is not `lng,lat`: {line}"));
        let lng: f64 = lng.trim().parse().expect("lng parses");
        let lat: f64 = lat.trim().parse().expect("lat parses");
        assert!((-180.0..=180.0).contains(&lng), "lng out of range: {line}");
        assert!((-90.0..=90.0).contains(&lat), "lat out of range: {line}");
    }

    let (lng, lat) = lines[0].split_once(',').expect("first line splits");
    let lng: f64 = lng.trim().parse().expect("lng parses");
    let lat: f64 = lat.trim().parse().expect("lat parses");
    assert!(
        (0.0..=15.0).contains(&lng),
        "first position's longitude should be near Marseille's 5.37 — \
         lat/lng look transposed: {}",
        lines[0]
    );
    assert!(
        (38.0..=48.0).contains(&lat),
        "first position's latitude should be near Marseille's 43.30 — \
         lat/lng look transposed: {}",
        lines[0]
    );
}

/// AC6: `--format json` is a compact object with the two documented keys.
#[test]
fn format_json_is_compact_object() {
    let out = run(&[
        "route", "--from", MARSEILLE, "--to", SHANGHAI, "--format", "json",
    ]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let s = stdout(&out);
    assert!(s.starts_with("{\"distance_km\":"), "stdout: {s}");
    assert!(s.contains("\"coordinates\":[["), "stdout: {s}");
    assert_eq!(
        s.trim_end().lines().count(),
        1,
        "compact json must be a single line: {s}"
    );
}

/// D2: a self-route still produces valid GeoJSON — RFC 7946 requires a
/// LineString to carry two or more positions, so the single snapped
/// position is emitted twice.
#[test]
fn self_route_emits_valid_two_position_linestring() {
    let out = run(&["route", "--from", "0,0", "--to", "0,0"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("\"distance_km\":0.000"), "stdout: {s}");

    let key = "\"coordinates\":";
    let start = s.find(key).expect("coordinates key present") + key.len();
    let end = s[start..].find("]]").expect("coordinates array closes") + start + 2;
    let coords = &s[start..end];
    assert_eq!(
        coords.matches("],[").count(),
        1,
        "LineString needs exactly two positions for a self-route: {coords}"
    );
}

/// AC7: an unsupported resolution exits 1, NOT clap's default 2 (which
/// would collide with "no route").
#[test]
fn invalid_resolution_exits_1() {
    let out = run(&[
        "route",
        "--resolution",
        "7",
        "--from",
        MARSEILLE,
        "--to",
        SHANGHAI,
    ]);
    assert_eq!(
        code(&out),
        1,
        "bad --resolution must exit 1, not clap's default 2. stderr: {}",
        stderr(&out)
    );
}

/// AC8: out-of-bounds coordinates exit 1.
#[test]
fn out_of_bounds_coordinate_exits_1() {
    let out = run(&["route", "--from", "91,0", "--to", SHANGHAI]);
    assert_eq!(code(&out), 1, "stdout: {}", stdout(&out));
}

/// A malformed coordinate is a parse error, also exit 1.
#[test]
fn malformed_coordinate_exits_1() {
    let out = run(&["route", "--from", "not-a-coord", "--to", SHANGHAI]);
    assert_eq!(code(&out), 1, "stdout: {}", stdout(&out));
}

/// Regression guard for `allow_hyphen_values`: without it clap treats a
/// leading `-` as the start of a flag and rejects the argument.
///
/// Asserts only that the exit code is NOT 1. Dropping
/// `allow_hyphen_values` makes clap report an unknown flag, which this
/// CLI maps to exit 1 — so `!= 1` captures exactly the regression this
/// test exists for. Asserting exit 0 instead would additionally require
/// that the graph connects these two ports, which is a property of the
/// bundled MARNET data rather than of argument parsing; a coarse
/// resolution or a data refresh could then fail this test for a reason
/// unrelated to its purpose.
#[test]
fn negative_coordinates_are_accepted() {
    // Cape Town -> Buenos Aires: both southern, one western.
    let out = run(&["route", "--from", "-33.92,18.42", "--to", "-34.60,-58.37"]);
    assert_ne!(
        code(&out),
        1,
        "negative coordinates must parse (allow_hyphen_values); \
         exit 1 means clap rejected them as flags. stderr: {}",
        stderr(&out)
    );
}

/// AC9: `--help` succeeds and prints to stdout.
#[test]
fn help_exits_zero() {
    let out = run(&["--help"]);
    assert_eq!(code(&out), 0);
    assert!(
        stdout(&out).contains("route"),
        "help must list the subcommand"
    );
}

/// AC1/AC12 structural half: the feature gate and the single-line clap
/// dependency are both load-bearing, and neither is observable from a
/// test that only runs the binary.
#[test]
fn cargo_toml_gates_the_bin_and_keeps_clap_on_one_line() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let toml =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    assert!(
        toml.contains("[[bin]]"),
        "Cargo.toml must declare a [[bin]] target"
    );
    assert!(
        toml.contains("required-features = [\"cli\"]"),
        "the bin must be gated on `cli` — that is what makes a default \
         `cargo build` skip it and never compile clap"
    );
    assert!(
        toml.contains("\"dep:clap\""),
        "the `cli` feature must pull clap in via `dep:clap`"
    );

    // tests/changelog.rs:32-43 returns the FIRST line trimming to a
    // `version`-prefixed string as the crate version. A multi-line clap
    // inline table would add another candidate.
    let version_lines: Vec<&str> = toml
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("version"))
        .collect();
    assert_eq!(
        version_lines.len(),
        1,
        "exactly one `version`-prefixed line may exist in Cargo.toml \
         (keep the clap dependency on a single line); found: {version_lines:?}"
    );
}
