#![no_main]
//! ENG-4691: fuzz `rustyroute::Graph::route` argument composition against a
//! fixed graph.
//!
//! Contract: for a valid graph, any `(from, to, blocked)` composition must
//! only ever return `Ok(Route)` or a typed `RouteError` — never panic. Coords
//! are validated inside `route` (`src/loader.rs:426`), and blocked ids that do
//! not exist simply never filter anything, so arbitrary inputs are safe by
//! construction; this target proves it empirically.

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::collections::HashSet;
use std::sync::OnceLock;

/// Structured fuzz input. `arbitrary` yields f64s spanning NaN/±inf and the
/// full range, exercising coord validation; libFuzzer's coverage feedback
/// learns in-range coordinates over time to reach the Dijkstra path. Both
/// endpoints are fuzzed independently (self-route and cross-node paths).
#[derive(Arbitrary, Debug)]
struct RouteInput {
    from: (f64, f64),
    to: (f64, f64),
    blocked: Vec<u32>,
}

/// The fixed graph, built once from the baked 50 km archive (available via the
/// path dep's default `data-50km` feature).
fn graph() -> &'static rustyroute::Graph {
    static G: OnceLock<rustyroute::Graph> = OnceLock::new();
    G.get_or_init(|| {
        rustyroute::Graph::from_bytes(rustyroute::data::BYTES_50KM)
            .expect("baked 50km archive is valid")
    })
}

fuzz_target!(|input: RouteInput| {
    let blocked: HashSet<u32> = input.blocked.into_iter().collect();
    let _ = graph().route(input.from, input.to, &blocked);
});
