//! ENG-4683 AC1, end to end: boot `examples/axum_server` as a real
//! process and drive it over a real socket.
//!
//! The ticket's first acceptance criterion is a runtime one —
//! "`cargo run --example axum_server` boots; `curl …` returns valid
//! GeoJSON with coordinates in `[lng, lat]` order" — and until this file
//! existed it was only ever checked by hand. `tests/readme_contract.rs`
//! proves the README and the example agree *textually*; this proves the
//! example actually works.
//!
//! What each test guards:
//!   AC1 coordinate order  -> `route_returns_geojson_in_lng_lat_order`
//!   AC1 valid GeoJSON     -> `self_route_still_emits_a_valid_linestring`
//!   blocked-edge plumbing -> `blocking_suez_lengthens_the_route`
//!   error contract        -> `bad_input_is_rejected_with_400`
//!
//! Deliberately no HTTP client dependency: a hand-written GET over
//! `TcpStream` keeps this test free of reqwest/hyper and of any version
//! coupling to axum's own stack. The crate already treats extra
//! dependencies as a cost (see `deny.toml`'s licence gate and the wasm
//! job's dependency reasoning in `.github/workflows/ci.yaml`).
//!
//! Gated off wasm32 because it spawns a process and opens sockets;
//! `tests/golden_routes.rs` carries the same gate for the same reason.
#![cfg(not(target_arch = "wasm32"))]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Owns the spawned server so it is killed even if a test panics.
struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Locate the compiled example.
///
/// `cargo test` (unfiltered) builds examples alongside test targets, so
/// under a normal run — and in CI, which runs
/// `cargo test --all-features` — the binary is already this test
/// binary's sibling in `target/<profile>/examples/` and freshly built.
///
/// Two cases have to be handled or this test becomes flaky rather than
/// merely red:
///
/// 1. **Absent.** `cargo test --test axum_example_e2e` may not build
///    examples at all.
/// 2. **Stale.** Worse, a *previously* built binary can still be
///    sitting there after `examples/axum_server.rs` was edited. That is
///    not a hypothetical: the first run of this file picked up a
///    pre-`PORT` binary, so all four tests fought over the hardcoded
///    port 3000 and three died with an empty stdout. A stale binary
///    must never be silently trusted.
///
/// So the sibling is used only when it is newer than the example
/// source; otherwise we rebuild. The rebuild targets a dedicated
/// directory because the outer `cargo test` holds the lock on the main
/// `target/` for the duration of the run — the same reason
/// `tests/feature_matrix.rs` and `tests/downstream_consumer_smoke.rs`
/// use their own target dirs.
fn example_binary() -> PathBuf {
    let name = if cfg!(windows) {
        "axum_server.exe"
    } else {
        "axum_server"
    };
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = manifest.join("examples").join("axum_server.rs");

    // current_exe is target/<profile>/deps/<test>-<hash>; the examples
    // directory is its sibling one level up.
    let mut dir = std::env::current_exe().expect("current_exe");
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    let candidate = dir.join("examples").join(name);
    if is_fresh(&candidate, &source) {
        return candidate;
    }

    let target_dir = std::env::var("OUT_DIR")
        .map(|s| PathBuf::from(s).join("axum_example_e2e_target"))
        .unwrap_or_else(|_| std::env::temp_dir().join("rustyroute_axum_example_e2e_target"));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(&cargo)
        .arg("build")
        .arg("--example")
        .arg("axum_server")
        .arg("--manifest-path")
        .arg(manifest.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .env("CARGO_TERM_COLOR", "never")
        .status()
        .expect("spawn cargo build --example axum_server");
    assert!(status.success(), "failed to build the axum_server example");

    let built = target_dir.join("debug").join("examples").join(name);
    assert!(
        built.exists(),
        "example binary still missing after build: {}",
        built.display()
    );
    built
}

/// `true` when `bin` exists and is at least as new as `source`.
fn is_fresh(bin: &PathBuf, source: &PathBuf) -> bool {
    let modified = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (modified(bin), modified(source)) {
        (Some(b), Some(s)) => b >= s,
        // If either timestamp is unavailable, rebuild rather than guess.
        _ => false,
    }
}

/// Boot the example on an OS-assigned port and wait until it reports
/// the port it bound. No sleep-and-hope: the readiness signal is the
/// server's own stdout line.
fn start_server() -> Server {
    let mut child = Command::new(example_binary())
        .env("PORT", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn axum_server example");

    let stdout = child.stdout.take().expect("piped stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("read the server's listening line");

    // "listening on http://0.0.0.0:34567"
    let port: u16 = line
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("could not parse a port out of {line:?}: {e}"));

    Server { child, port }
}

/// Minimal HTTP/1.1 GET. Returns `(status_code, body)`.
fn get(port: u16, path_and_query: &str) -> (u16, String) {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("connect to the example server");
    write!(
        stream,
        "GET {path_and_query} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .expect("write request");
    stream.flush().expect("flush request");

    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("read response");

    let (head, body) = raw
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("malformed HTTP response: {raw:?}"));
    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("no status code in {head:?}"));
    (status, body.to_string())
}

fn coordinates(body: &str) -> Vec<Vec<f64>> {
    let v: serde_json::Value = serde_json::from_str(body).expect("response is JSON");
    assert_eq!(v["type"], "FeatureCollection", "body was: {body}");
    let feature = &v["features"][0];
    assert_eq!(feature["geometry"]["type"], "LineString");
    serde_json::from_value(feature["geometry"]["coordinates"].clone()).expect("coordinates array")
}

fn distance_km(body: &str) -> f64 {
    let v: serde_json::Value = serde_json::from_str(body).expect("response is JSON");
    v["features"][0]["properties"]["distance_km"]
        .as_f64()
        .unwrap_or_else(|| panic!("no numeric distance_km in {body}"))
}

/// AC1. The whole point of the example: `Route.coordinates` is
/// `(lat, lng)` (src/loader.rs:41-42) and GeoJSON positions are
/// `[lng, lat]`, so the response must carry the swap.
///
/// Marseille (43.30 N, 5.37 E) → Shanghai. The first position must open
/// with the *longitude* ~5.19. If the swap were missing or inverted the
/// first element would be ~43.23, which this asserts against explicitly
/// rather than just checking "two numbers came back".
#[test]
fn route_returns_geojson_in_lng_lat_order() {
    let server = start_server();
    let (status, body) = get(
        server.port,
        "/route?fromLatLng=43.30,5.37&toLatLng=31.23,121.47",
    );
    assert_eq!(status, 200, "body was: {body}");

    let coords = coordinates(&body);
    assert!(
        coords.len() > 2,
        "expected a multi-point path, got {coords:?}"
    );

    let first = &coords[0];
    assert_eq!(first.len(), 2, "a GeoJSON position is [lng, lat]");
    let (lng, lat) = (first[0], first[1]);
    assert!(
        (4.0..7.0).contains(&lng),
        "first element must be the LONGITUDE near Marseille (~5.19), got {lng} \
         — the (lat, lng) -> [lng, lat] swap is missing or inverted"
    );
    assert!(
        (42.0..45.0).contains(&lat),
        "second element must be the LATITUDE near Marseille (~43.23), got {lat}"
    );

    // Every position must be a well-formed [lng, lat] pair in range.
    for p in &coords {
        assert_eq!(p.len(), 2, "malformed position {p:?}");
        assert!(
            (-180.0..=180.0).contains(&p[0]),
            "longitude out of range: {p:?}"
        );
        assert!(
            (-90.0..=90.0).contains(&p[1]),
            "latitude out of range: {p:?}"
        );
    }

    assert!(
        distance_km(&body) > 10_000.0,
        "Marseille -> Shanghai is a long way; got {} km",
        distance_km(&body)
    );
}

/// AC1, the "valid GeoJSON" half. `Graph::route` returns exactly ONE
/// coordinate when both endpoints snap to the same node
/// (src/loader.rs:442-449, pinned by tests/route_smoke.rs:28-38), but
/// RFC 7946 §3.1.4 requires a LineString to have two or more positions.
/// Without the example's pad this response would be invalid GeoJSON.
#[test]
fn self_route_still_emits_a_valid_linestring() {
    let server = start_server();
    let (status, body) = get(
        server.port,
        "/route?fromLatLng=43.30,5.37&toLatLng=43.30,5.37",
    );
    assert_eq!(status, 200, "body was: {body}");

    let coords = coordinates(&body);
    assert!(
        coords.len() >= 2,
        "RFC 7946 3.1.4: a LineString needs >= 2 positions, got {}: {coords:?}",
        coords.len()
    );
    assert_eq!(
        coords[0], coords[1],
        "a degenerate self-route repeats its single point"
    );
    assert_eq!(distance_km(&body), 0.0, "a self-route covers no distance");
}

/// The `block=` parameter must actually reach
/// `Graph::edges_for_groups` + `Graph::route`. Blocking the Suez Canal
/// forces the route around the Cape of Good Hope, so the distance must
/// grow — an inequality, so the test does not pin a golden number that
/// `tests/golden_routes.rs` already owns.
#[test]
fn blocking_suez_lengthens_the_route() {
    let server = start_server();
    let q = "/route?fromLatLng=43.30,5.37&toLatLng=31.23,121.47";
    let (open_status, open_body) = get(server.port, q);
    let (blocked_status, blocked_body) = get(server.port, &format!("{q}&block=suezCanal"));
    assert_eq!(open_status, 200);
    assert_eq!(blocked_status, 200, "body was: {blocked_body}");

    assert!(
        distance_km(&blocked_body) > distance_km(&open_body),
        "blocking suezCanal ({} km) must exceed the open route ({} km)",
        distance_km(&blocked_body),
        distance_km(&open_body)
    );
}

/// The error contract the example documents: unparseable coordinates
/// and unknown edge groups are client errors, not 500s or panics. The
/// second case also proves an unknown group is rejected rather than
/// silently ignored.
#[test]
fn bad_input_is_rejected_with_400() {
    let server = start_server();

    let (status, body) = get(server.port, "/route?fromLatLng=abc&toLatLng=0,0");
    assert_eq!(status, 400, "body was: {body}");
    assert!(
        body.contains("fromLatLng"),
        "the error should name the offending parameter, got: {body}"
    );

    let (status, body) = get(
        server.port,
        "/route?fromLatLng=43.30,5.37&toLatLng=31.23,121.47&block=notAGroup",
    );
    assert_eq!(status, 400, "body was: {body}");
    assert!(
        body.contains("notAGroup"),
        "the error should name the unknown group, got: {body}"
    );
}
