#![no_main]
//! ENG-4691: fuzz `rustyroute::Graph::route` argument composition against a
//! fixed graph.
//!
//! Contract: for a valid graph, any `(from, to, blocked)` composition must
//! only ever return `Ok(Route)` or a typed `RouteError` — never panic. Coords
//! are validated inside `route` (`src/loader.rs:426`), and blocked ids that do
//! not exist simply never filter anything, so arbitrary inputs are safe by
//! construction; this target proves it empirically.

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use std::collections::HashSet;
use std::sync::OnceLock;

/// Structured fuzz input. The f64 endpoints span NaN/±inf and the full range,
/// exercising coord validation; libFuzzer's coverage feedback learns in-range
/// coordinates over time to reach the Dijkstra path. Both endpoints are fuzzed
/// independently (self-route and cross-node paths).
#[derive(Debug)]
struct RouteInput {
    from: (f64, f64),
    to: (f64, f64),
    blocked: Vec<u32>,
}

/// Hand-written `Arbitrary` (rather than derive) so the `blocked` set length is
/// bounded. `route` treats unknown edge ids as no-op filters, so blocked-set
/// *size* has no bearing on routing correctness — but a derived unbounded
/// `Vec<u32>` would grow with the fuzzer's input size (especially under
/// OSS-Fuzz), producing large `Vec`/`HashSet` allocations that manifest as
/// slow units or OOMs unrelated to `Graph::route`. The count is read from a
/// single `u8`, capping the set at 255 elements.
impl<'a> Arbitrary<'a> for RouteInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let from = <(f64, f64)>::arbitrary(u)?;
        let to = <(f64, f64)>::arbitrary(u)?;
        let count = u8::arbitrary(u)? as usize; // ≤ 255 — bounds memory use
        let mut blocked = Vec::with_capacity(count);
        for _ in 0..count {
            blocked.push(u32::arbitrary(u)?);
        }
        Ok(RouteInput { from, to, blocked })
    }
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
