//! ENG-4679: runtime wrapper around the rkyv-archived [`GraphData`].
//!
//! This file owns the public `Graph` type, `LoadError`, `from_bytes`,
//! and `load`. It is deliberately a separate file from `src/graph.rs`
//! (which holds only the rkyv schema) because three integration tests
//! (`tests/group_assignment.rs`, `tests/tampered_gpkg_panic.rs`,
//! `tests/build_helpers_csr.rs`) re-include `src/graph.rs` via
//! `#[path]`. Putting the runtime here means those test crates do not
//! need to stub `crate::data` or pull in `memmap2`.
//!
//! [`GraphData`]: crate::graph::GraphData

use crate::graph::{ArchivedGraphData, MAGIC, SCHEMA_VERSION};
use std::collections::HashSet;
use std::path::PathBuf;

/// Undirected edge id: index into [`GraphData::edge_endpoints`] and
/// [`GraphData::undirected_weights`]. The A→B and B→A half-edges of one
/// undirected edge share the same `EdgeId`.
///
/// [`GraphData::edge_endpoints`]: crate::graph::GraphData::edge_endpoints
/// [`GraphData::undirected_weights`]: crate::graph::GraphData::undirected_weights
pub type EdgeId = u32;

/// Node id: index into [`GraphData::nodes`] and the CSR row-pointer
/// table [`GraphData::node_offsets`].
///
/// [`GraphData::nodes`]: crate::graph::GraphData::nodes
/// [`GraphData::node_offsets`]: crate::graph::GraphData::node_offsets
pub type NodeId = u32;

/// A shortest path returned by [`Graph::route`].
///
/// Coordinates are `(lat, lng)` decimal degrees — one per node along
/// the path, in traversal order — widened from the graph's `f32` node
/// coordinates to `f64` for the public API. HTTP-layer callers convert
/// to `GeoJSON` `[lng, lat]` order at the boundary (Design 019
/// §"Per request").
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// `(lat, lng)` decimal degrees, one per node along the path.
    pub coordinates: Vec<(f64, f64)>,
    /// Total path length in kilometres, summed from the canonical
    /// undirected edge weights (not from the scaled integer search
    /// cost, to avoid drift).
    pub distance_km: f64,
    /// Undirected edge ids traversed, in order. Empty for a self-route.
    pub edge_ids: Vec<EdgeId>,
}

/// Errors returned by [`Graph::route`] and [`Graph::edges_for_groups`].
#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    /// `from` was not a finite `(lat, lng)` within
    /// `lat ∈ [-90, 90]`, `lng ∈ [-180, 180]`.
    #[error("from coordinate out of bounds: {0:?}")]
    BadFromCoord((f64, f64)),
    /// `to` was not a finite `(lat, lng)` within
    /// `lat ∈ [-90, 90]`, `lng ∈ [-180, 180]`.
    #[error("to coordinate out of bounds: {0:?}")]
    BadToCoord((f64, f64)),
    /// A name passed to [`Graph::edges_for_groups`] did not match any
    /// of the baked-in edge groups.
    #[error("unknown edge group: {0}")]
    UnknownGroup(String),
    /// No path exists between the snapped endpoints given the blocked
    /// edge set (e.g. every route to an island was blocked).
    #[error("no route found")]
    NoRoute,
}

/// Errors returned by [`Graph::from_bytes`] and [`Graph::load`].
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// Requested resolution is not in the supported set {5, 10, 20,
    /// 50, 100} km.
    #[error("unknown resolution {0}km (allowed: 5, 10, 20, 50, 100)")]
    UnknownResolution(u32),

    /// The requested resolution is allowed, but no source (env var,
    /// in-tree OUT_DIR, or static feature) was available. Enable the
    /// matching `data-{N}km` feature or set `$RUSTYROUTE_DATA_DIR`.
    #[error(
        "data not available for {0}km — enable the `data-{0}km` feature or \
         set $RUSTYROUTE_DATA_DIR"
    )]
    DataNotAvailable(u32),

    /// `$RUSTYROUTE_DATA_DIR` was set, but the expected file is
    /// missing at that location.
    #[error("data file missing: {0}")]
    DataFileMissing(PathBuf),

    /// First four bytes did not match the `b"RRG1"` magic prefix.
    #[error("bad magic: expected b\"RRG1\", got {0:?}")]
    BadMagic([u8; 4]),

    /// Schema version (bytes 4..8 as little-endian u32) does not
    /// match this build's `SCHEMA_VERSION`.
    #[error("unsupported schema version {0} (this build supports {SCHEMA_VERSION})")]
    UnsupportedSchema(u32),

    /// I/O error from file open or mmap. Only reachable on native
    /// (non-wasm) targets via [`Graph::load`].
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// rkyv's checked `access` rejected the payload. Indicates either
    /// truncation, byte tampering, or alignment problems.
    #[error("invalid rkyv archive: {0}")]
    InvalidArchive(rkyv::rancor::Error),
}

/// Owned handle to a loaded graph archive. Owns its backing buffer
/// (an mmap on native, a `&'static [u8]` for `from_bytes` and wasm
/// targets) and exposes `archived(&self) -> &ArchivedGraphData` tied
/// to the handle's lifetime.
///
/// `Graph` is `Send + Sync` (both backings are). It is NOT `Clone`:
/// consumers who need multiple handles should wrap in
/// [`std::sync::Arc`].
///
/// # Long-lived handle pattern (Design 019)
///
/// For applications that load the graph once and use it for the
/// process lifetime (routefinder), stash the `Graph` in a
/// `OnceLock` and `Box::leak` it to obtain `&'static Graph`:
///
/// ```ignore
/// use std::sync::OnceLock;
/// use rustyroute::Graph;
///
/// fn graph() -> &'static Graph {
///     static G: OnceLock<&'static Graph> = OnceLock::new();
///     G.get_or_init(|| {
///         let g = Graph::load(50).expect("load graph");
///         Box::leak(Box::new(g))
///     })
/// }
/// ```
///
/// This deliberately leaks the graph for the process lifetime — that
/// is the trade-off for avoiding per-call lifetime annotations on
/// downstream routing APIs.
pub struct Graph {
    backing: GraphBacking,
    resolution_km: u32,
}

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match &self.backing {
            #[cfg(not(target_arch = "wasm32"))]
            GraphBacking::Mmap(m) => format!("Mmap({} bytes)", m.len()),
            GraphBacking::Static(s) => format!("Static({} bytes)", s.len()),
        };
        f.debug_struct("Graph")
            .field("resolution_km", &self.resolution_km)
            .field("backing", &kind)
            .finish()
    }
}

enum GraphBacking {
    #[cfg(not(target_arch = "wasm32"))]
    Mmap(memmap2::Mmap),
    Static(&'static [u8]),
}

impl Graph {
    /// Construct a graph by validating and accessing a static byte
    /// slice. Works on every target including `wasm32`.
    ///
    /// Validates the 4-byte magic, the 4-byte little-endian schema
    /// version, and then runs rkyv's checked `access` on the
    /// remainder. The returned handle's `resolution_km()` is `0`
    /// because the archive bytes do not carry the resolution; use
    /// [`Graph::load`] when you need that field populated.
    pub fn from_bytes(bytes: &'static [u8]) -> Result<Graph, LoadError> {
        validate_header(bytes)?;
        // Checked access — surfaces InvalidArchive on tampering.
        let _ = rkyv::access::<ArchivedGraphData, rkyv::rancor::Error>(&bytes[8..])
            .map_err(LoadError::InvalidArchive)?;
        Ok(Graph {
            backing: GraphBacking::Static(bytes),
            resolution_km: 0,
        })
    }

    /// Load a graph by resolution_km. Tries, in order:
    /// 1. `$RUSTYROUTE_DATA_DIR/{N}km.rkyv` (if env var is set;
    ///    missing file → [`LoadError::DataFileMissing`])
    /// 2. `$OUT_DIR/data/{N}km.rkyv` baked at rustyroute compile time
    /// 3. `data::BYTES_{N}KM` static fallback (if the matching
    ///    `data-{N}km` feature is enabled)
    /// 4. [`LoadError::DataNotAvailable`]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(resolution_km: u32) -> Result<Graph, LoadError> {
        const ALLOWED: &[u32] = &[5, 10, 20, 50, 100];
        if !ALLOWED.contains(&resolution_km) {
            return Err(LoadError::UnknownResolution(resolution_km));
        }

        // Step 1: explicit env-var override. Uses `var_os` so a
        // non-UTF8 directory path is honoured rather than silently
        // treated as unset. Attempts `File::open` directly (no
        // `path.exists()` pre-check) to avoid a TOCTOU race and to
        // distinguish NotFound (→ `DataFileMissing`) from permission
        // or other I/O errors (→ `Io`).
        if let Some(dir) = std::env::var_os("RUSTYROUTE_DATA_DIR") {
            let path = PathBuf::from(dir).join(format!("{resolution_km}km.rkyv"));
            return match std::fs::File::open(&path) {
                Ok(file) => Self::load_file(file, resolution_km),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    Err(LoadError::DataFileMissing(path))
                }
                Err(e) => Err(LoadError::Io(e)),
            };
        }

        // Step 2: baked-in OUT_DIR from rustyroute's own build.rs.
        // option_env! evaluates at compile time of THIS crate.
        // Test-only override (see test_override module below) allows
        // unit tests to skip this step. NotFound falls through to
        // step 3; other I/O errors surface immediately.
        if !test_override::skip_out_dir()
            && let Some(out_dir) = option_env!("OUT_DIR")
        {
            let path = PathBuf::from(out_dir).join(format!("data/{resolution_km}km.rkyv"));
            match std::fs::File::open(&path) {
                Ok(file) => return Self::load_file(file, resolution_km),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(LoadError::Io(e)),
            }
        }

        // Step 3: static fallback (feature-gated).
        if let Some(bytes) = crate::data::bytes_for(resolution_km) {
            let mut g = Self::from_bytes(bytes)?;
            g.resolution_km = resolution_km;
            return Ok(g);
        }

        Err(LoadError::DataNotAvailable(resolution_km))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_file(file: std::fs::File, resolution_km: u32) -> Result<Graph, LoadError> {
        // SAFETY: memmap2::Mmap::map is unsafe because the kernel can
        // change the underlying file bytes out from under us. We treat
        // the mmap as immutable for the lifetime of the Graph: this
        // crate never writes through the mapping, and the .rkyv files
        // live under OUT_DIR (build script output) or a user-managed
        // data dir, where the operator is responsible for not mutating
        // them in place. Magic + version + checked rkyv access run
        // immediately after mapping, so any post-mapping tampering
        // surfaces as SIGBUS on access — the documented best-effort
        // guarantee for read-only mmaps.
        #[allow(unsafe_code)]
        let mmap = unsafe { memmap2::Mmap::map(&file)? };

        validate_header(&mmap)?;
        let _ = rkyv::access::<ArchivedGraphData, rkyv::rancor::Error>(&mmap[8..])
            .map_err(LoadError::InvalidArchive)?;

        Ok(Graph {
            backing: GraphBacking::Mmap(mmap),
            resolution_km,
        })
    }

    /// Resolution in kilometres. Returns 0 for graphs constructed via
    /// [`Graph::from_bytes`] (the archive header does not carry the
    /// resolution; only [`Graph::load`] populates it).
    pub fn resolution_km(&self) -> u32 {
        self.resolution_km
    }

    /// Access the rkyv-archived form of the graph. The reference is
    /// tied to `&self` — do not attempt to outlive the Graph handle.
    /// Re-runs rkyv's checked access each call; cache into a local
    /// `let g = self.archived();` if you intend to hot-loop.
    pub fn archived(&self) -> &ArchivedGraphData {
        let payload: &[u8] = match &self.backing {
            #[cfg(not(target_arch = "wasm32"))]
            GraphBacking::Mmap(m) => &m[8..],
            GraphBacking::Static(s) => &s[8..],
        };
        rkyv::access::<ArchivedGraphData, rkyv::rancor::Error>(payload)
            .expect("validated on construction; payload bytes are immutable")
    }

    /// Number of distinct nodes in the graph.
    pub fn node_count(&self) -> u32 {
        self.archived().nodes.len() as u32
    }

    /// Number of undirected edges (distinct
    /// `(src_node_id, dst_node_id)` endpoints).
    pub fn edge_count(&self) -> u32 {
        self.archived().edge_endpoints.len() as u32
    }

    /// Number of directed half-edges in the CSR adjacency. For
    /// non-self-loop undirected edges this is `2 * edge_count`; for
    /// self-loops the forward half is emitted once and the reverse
    /// is suppressed, so `directed_edge_count` ranges between
    /// `edge_count` (all self-loops) and `2 * edge_count`.
    pub fn directed_edge_count(&self) -> u32 {
        self.archived().edges.len() as u32
    }
}

// =====================================================================
// ENG-4680: routing API — Dijkstra shortest path with an in-line
// blocked-edge filter, plus edge-group lookup.
//
// The routing types (`Route` / `RouteError` / `EdgeId` / `NodeId`) and
// the `impl Graph` live here rather than in `src/graph.rs`: they are
// runtime API (this file's stated role — see graph.rs's module doc),
// they need `Graph` / `archived()` (defined here), and keeping them out
// of `graph.rs` avoids perturbing the build script and the three test
// crates that re-include `graph.rs` standalone via `#[path]`.
// =====================================================================

/// Multiplier converting kilometres to integer micro-kilometres (µkm).
/// Dijkstra runs on integer cost so its `Ord` is total and sidesteps
/// float NaN ordering; µkm (1 mm) resolution is far finer than the
/// graph's ~km edge weights.
const UKM_PER_KM: f64 = 1_000_000.0;

/// Earth radius (km), matching `build/geometry.rs::EARTH_RADIUS_KM` so
/// nearest-node snapping uses the same metric that produced the baked
/// edge weights.
const EARTH_RADIUS_KM: f64 = 6371.0088;

/// Scale a kilometre weight to integer µkm for Dijkstra's cost.
fn scale_km(weight_km: f32) -> u64 {
    // Non-negative by construction (haversine distances); `round` keeps
    // the nearest µkm. The cast is saturating-safe: the largest single
    // edge is well under 2^64 µkm.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (f64::from(weight_km) * UKM_PER_KM).round() as u64
    }
}

/// Haversine great-circle distance in km between two `(lat, lng)`
/// points in decimal degrees.
///
/// Uses the same formula and [`EARTH_RADIUS_KM`] as the build-time
/// `build/geometry.rs::haversine_km` (a build-only module, not linked
/// into the library), but note the **argument order differs**: the
/// build function takes `(lng, lat)`, whereas this one takes
/// `(lat, lng)` to match the public [`Graph::route`] coordinate order.
fn haversine_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let (lat1_r, lat2_r) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + lat1_r.cos() * lat2_r.cos() * (dlng / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().asin()
}

/// `true` if `(lat, lng)` is finite and within valid geographic bounds
/// (`lat ∈ [-90, 90]`, `lng ∈ [-180, 180]`).
fn coord_in_bounds((lat, lng): (f64, f64)) -> bool {
    lat.is_finite()
        && lng.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lng)
}

impl Graph {
    /// Snap a `(lat, lng)` coordinate to the nearest node by haversine
    /// distance via a linear scan over the node table. Returns `None`
    /// only for an empty graph.
    ///
    /// Linear scan is acceptable up to the 5 km resolution (~few
    /// hundred k nodes); a k-d tree upgrade is deferred to ENG-4690.
    fn nearest_node(&self, (lat, lng): (f64, f64)) -> Option<NodeId> {
        let mut best: Option<(NodeId, f64)> = None;
        for (i, nc) in self.archived().nodes.iter().enumerate() {
            let d = haversine_km(
                lat,
                lng,
                f64::from(nc.lat.to_native()),
                f64::from(nc.lng.to_native()),
            );
            if best.is_none_or(|(_, bd)| d < bd) {
                // `i` is a valid node index, so it fits `u32` by schema.
                #[allow(clippy::cast_possible_truncation)]
                let id = i as NodeId;
                best = Some((id, d));
            }
        }
        best.map(|(id, _)| id)
    }

    /// Shortest path from `from` to `to` (both `(lat, lng)` decimal
    /// degrees), snapped to the nearest graph node, avoiding every
    /// undirected edge in `blocked`.
    ///
    /// Blocking is enforced in-line by the successor closure — the
    /// graph is neither mutated nor copied.
    ///
    /// # Errors
    ///
    /// - [`RouteError::BadFromCoord`] / [`RouteError::BadToCoord`] if an
    ///   endpoint is non-finite or outside `lat ∈ [-90, 90]`,
    ///   `lng ∈ [-180, 180]`.
    /// - [`RouteError::NoRoute`] if no unblocked path connects the
    ///   snapped endpoints (or the graph is empty).
    ///
    /// # Panics
    ///
    /// Never in practice. Reconstruction re-locates the forward edge of
    /// each hop Dijkstra already relaxed across; the internal
    /// `expect` guards that invariant and firing it would signal graph
    /// corruption, not a caller error.
    pub fn route(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        blocked: &HashSet<EdgeId>,
    ) -> Result<Route, RouteError> {
        if !coord_in_bounds(from) {
            return Err(RouteError::BadFromCoord(from));
        }
        if !coord_in_bounds(to) {
            return Err(RouteError::BadToCoord(to));
        }

        let start = self.nearest_node(from).ok_or(RouteError::NoRoute)?;
        let goal = self.nearest_node(to).ok_or(RouteError::NoRoute)?;

        let g = self.archived();
        let node_latlng = |n: NodeId| -> (f64, f64) {
            let nc = &g.nodes[n as usize];
            (f64::from(nc.lat.to_native()), f64::from(nc.lng.to_native()))
        };

        // Self-route: both endpoints snap to the same node.
        if start == goal {
            return Ok(Route {
                coordinates: vec![node_latlng(start)],
                distance_km: 0.0,
                edge_ids: Vec::new(),
            });
        }

        let offsets = g.node_offsets.as_slice();
        let edges = g.edges.as_slice();

        // CSR neighbours of `n`, skipping blocked undirected edges, cost
        // in integer µkm. `move` captures the (Copy) slice references and
        // `blocked` so the closure can outlive this stack frame inside
        // Dijkstra.
        let successors = move |&n: &NodeId| -> Vec<(NodeId, u64)> {
            let lo = offsets[n as usize].to_native() as usize;
            let hi = offsets[n as usize + 1].to_native() as usize;
            // Preallocate the CSR row width (`hi - lo`, the pre-filter
            // upper bound) so this hot-loop closure never reallocates:
            // a filtered `collect()` can only see the iterator's lower
            // size hint (0) and would grow the Vec repeatedly.
            let mut out = Vec::with_capacity(hi - lo);
            for e in &edges[lo..hi] {
                if !blocked.contains(&e.edge_id.to_native()) {
                    out.push((e.target.to_native(), scale_km(e.weight_km.to_native())));
                }
            }
            out
        };

        let (path, _cost) =
            pathfinding::directed::dijkstra::dijkstra(&start, successors, |&n| n == goal)
                .ok_or(RouteError::NoRoute)?;

        // Reconstruct undirected edge ids and the canonical distance by
        // walking consecutive node pairs. `distance_km` is summed from
        // `undirected_weights` (the canonical f32, widened) rather than
        // from the scaled integer cost, to avoid drift.
        let weights = g.undirected_weights.as_slice();
        let mut edge_ids: Vec<EdgeId> = Vec::with_capacity(path.len().saturating_sub(1));
        let mut distance_km = 0.0_f64;
        for pair in path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let lo = offsets[a as usize].to_native() as usize;
            let hi = offsets[a as usize + 1].to_native() as usize;
            // The min-weight unblocked a→b half-edge is the one Dijkstra
            // relaxed across for this hop.
            let chosen = edges[lo..hi]
                .iter()
                .filter(|e| e.target.to_native() == b && !blocked.contains(&e.edge_id.to_native()))
                .min_by(|x, y| x.weight_km.to_native().total_cmp(&y.weight_km.to_native()))
                .expect("a Dijkstra path hop always has an unblocked forward edge");
            let id = chosen.edge_id.to_native();
            distance_km += f64::from(weights[id as usize].to_native());
            edge_ids.push(id);
        }

        let coordinates = path.iter().map(|&n| node_latlng(n)).collect();
        Ok(Route {
            coordinates,
            distance_km,
            edge_ids,
        })
    }

    /// Collect the union of undirected edge ids for the named edge
    /// groups (the 13 baked-in chokepoints/passages).
    ///
    /// # Errors
    ///
    /// [`RouteError::UnknownGroup`] if any name does not match a baked-in
    /// group.
    pub fn edges_for_groups<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<HashSet<EdgeId>, RouteError> {
        let groups = self.archived().groups.as_slice();
        let mut out = HashSet::new();
        for name in names {
            let entry = groups
                .iter()
                .find(|g| g.name.as_str() == name)
                .ok_or_else(|| RouteError::UnknownGroup(name.to_string()))?;
            out.extend(entry.edge_ids.iter().map(|id| id.to_native()));
        }
        Ok(out)
    }
}

fn validate_header(bytes: &[u8]) -> Result<(), LoadError> {
    if bytes.len() < 8 {
        // Truncated header. Report a zero array; callers wanting more
        // detail should check the file length separately.
        return Err(LoadError::BadMagic([0; 4]));
    }
    let magic: [u8; 4] = bytes[0..4].try_into().expect("4-byte slice");
    if &magic != MAGIC {
        return Err(LoadError::BadMagic(magic));
    }
    let ver = u32::from_le_bytes(bytes[4..8].try_into().expect("4-byte slice"));
    if ver != SCHEMA_VERSION {
        return Err(LoadError::UnsupportedSchema(ver));
    }
    Ok(())
}

// =====================================================================
// Test-only override: lets unit tests in this file skip step 2 of the
// `load` resolution order so they can exercise the static-fallback and
// `DataNotAvailable` branches deterministically even when the in-tree
// build wrote `$OUT_DIR/data/*.rkyv`.
// =====================================================================
#[cfg(not(target_arch = "wasm32"))]
mod test_override {
    use std::sync::atomic::{AtomicBool, Ordering};

    static SKIP_OUT_DIR: AtomicBool = AtomicBool::new(false);

    pub(super) fn skip_out_dir() -> bool {
        SKIP_OUT_DIR.load(Ordering::Acquire)
    }

    /// RAII guard that flips `SKIP_OUT_DIR` to `true` on construction
    /// and restores it to `false` on drop — including when the
    /// owning test panics, so a single failing assertion can't leak
    /// step-2-disabled state into other tests in the binary.
    /// Test-only; crate-internal to this module.
    #[cfg(test)]
    pub(super) struct SkipOutDirGuard;

    #[cfg(test)]
    impl SkipOutDirGuard {
        pub(super) fn enable() -> Self {
            SKIP_OUT_DIR.store(true, Ordering::Release);
            Self
        }
    }

    #[cfg(test)]
    impl Drop for SkipOutDirGuard {
        fn drop(&mut self) {
            SKIP_OUT_DIR.store(false, Ordering::Release);
        }
    }
}

// =====================================================================
// Unit tests for `load`'s resolution-order branches (AC3, AC4).
//
// The `#[allow(unsafe_code)]` on the module is required because Rust
// 2024's `std::env::set_var` and `std::env::remove_var` are `unsafe`
// (process-wide mutable global state), and the crate-level
// `#![deny(unsafe_code)]` would otherwise reject these test-only
// blocks. The targeted allow keeps the deny-everywhere posture
// outside this one test module.
// =====================================================================
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes env-mutating tests in this binary. libtest runs
    /// tests in parallel by default, so multiple tests that touch
    /// `RUSTYROUTE_DATA_DIR` or `test_override::skip_out_dir` would
    /// race without this lock. Acquiring the guard provides the
    /// single-threaded mutation that Rust 2024's `unsafe`
    /// `set_var`/`remove_var` require for soundness.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// AC3: with no env var and OUT_DIR step disabled, the
    /// static-fallback satisfies `load(50)` under default features
    /// (`data-50km`).
    #[test]
    #[cfg(feature = "data-50km")]
    fn load_50km_falls_through_to_static() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: the ENV_LOCK guard above ensures this test is the
        // only thread mutating env state or `skip_out_dir` for its
        // duration, satisfying Rust 2024's single-threaded-mutation
        // requirement for `set_var`/`remove_var`.
        unsafe {
            std::env::remove_var("RUSTYROUTE_DATA_DIR");
        }
        let _skip = test_override::SkipOutDirGuard::enable();
        let g = Graph::load(50).expect("load(50) via static fallback");
        assert_eq!(g.resolution_km(), 50);
    }

    /// AC3: with no env var, OUT_DIR step disabled, and the
    /// `data-50km` feature disabled, `load(50)` returns
    /// `DataNotAvailable(50)`.
    #[test]
    #[cfg(not(feature = "data-50km"))]
    fn load_50km_data_not_available_when_feature_off() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: the ENV_LOCK guard above ensures this test is the
        // only thread mutating env state or `skip_out_dir` for its
        // duration, satisfying Rust 2024's single-threaded-mutation
        // requirement for `set_var`/`remove_var`.
        unsafe {
            std::env::remove_var("RUSTYROUTE_DATA_DIR");
        }
        let _skip = test_override::SkipOutDirGuard::enable();
        match Graph::load(50) {
            Err(LoadError::DataNotAvailable(50)) => {}
            other => panic!("expected DataNotAvailable(50), got {other:?}"),
        }
    }

    /// `$RUSTYROUTE_DATA_DIR` set to a non-existent dir → DataFileMissing.
    #[test]
    fn load_50km_data_file_missing_when_env_dir_empty() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join("rustyroute_test_nonexistent_dir");
        // SAFETY: the ENV_LOCK guard above ensures this test is the
        // only thread mutating env state for its duration, satisfying
        // Rust 2024's single-threaded-mutation requirement for
        // `set_var`/`remove_var`.
        unsafe {
            std::env::set_var("RUSTYROUTE_DATA_DIR", &tmp);
        }
        let res = Graph::load(50);
        unsafe {
            std::env::remove_var("RUSTYROUTE_DATA_DIR");
        }
        match res {
            Err(LoadError::DataFileMissing(p)) => {
                assert!(p.ends_with("50km.rkyv"));
            }
            other => panic!("expected DataFileMissing, got {other:?}"),
        }
    }
}
