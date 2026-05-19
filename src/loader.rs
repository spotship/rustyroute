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
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::path::PathBuf;

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

        // Step 1: explicit env-var override.
        if let Ok(dir) = std::env::var("RUSTYROUTE_DATA_DIR") {
            let path = PathBuf::from(dir).join(format!("{resolution_km}km.rkyv"));
            if !path.exists() {
                return Err(LoadError::DataFileMissing(path));
            }
            return Self::load_path(&path, resolution_km);
        }

        // Step 2: baked-in OUT_DIR from rustyroute's own build.rs.
        // option_env! evaluates at compile time of THIS crate.
        // Test-only override (see test_override module below) allows
        // unit tests to skip this step.
        if !test_override::skip_out_dir()
            && let Some(out_dir) = option_env!("OUT_DIR")
        {
            let path = PathBuf::from(out_dir).join(format!("data/{resolution_km}km.rkyv"));
            if path.exists() {
                return Self::load_path(&path, resolution_km);
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
    fn load_path(path: &Path, resolution_km: u32) -> Result<Graph, LoadError> {
        let file = std::fs::File::open(path)?;
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

    /// Test-only hook. Crate-internal to this module so external code
    /// cannot reach for it. Used by `#[cfg(test)]` blocks below.
    #[cfg(test)]
    pub(super) fn set_skip_out_dir(v: bool) {
        SKIP_OUT_DIR.store(v, Ordering::Release);
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
        test_override::set_skip_out_dir(true);
        let g = Graph::load(50).expect("load(50) via static fallback");
        assert_eq!(g.resolution_km(), 50);
        test_override::set_skip_out_dir(false);
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
        test_override::set_skip_out_dir(true);
        match Graph::load(50) {
            Err(LoadError::DataNotAvailable(50)) => {}
            other => panic!("expected DataNotAvailable(50), got {other:?}"),
        }
        test_override::set_skip_out_dir(false);
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
