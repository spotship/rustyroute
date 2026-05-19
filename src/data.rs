//! Feature-gated static byte slices for each pre-baked graph
//! resolution. Each `BYTES_{N}KM` const is `include_bytes!`-baked at
//! rustyroute's compile time from `$OUT_DIR/data/{N}km.rkyv` produced
//! by `build.rs`.
//!
//! These slices are the primary distribution mechanism for downstream
//! consumers: with the default `data-50km` feature, downstream code
//! can simply call
//! `rustyroute::Graph::from_bytes(rustyroute::data::BYTES_50KM)`.
//!
//! # Feature flags
//!
//! Each resolution is gated by a `data-{N}km` Cargo feature so that
//! disabling a resolution removes the corresponding ~MB-scale archive
//! from the published library binary. The default feature set enables
//! `data-50km` only.
//!
//! # Alignment
//!
//! `include_bytes!` returns a `&[u8]` whose alignment is governed by
//! the rustc placement of `[u8; N]` static items. Today's archived
//! schema (`ArchivedGraphData` in [`crate::graph`]) needs ≤4-byte
//! alignment; rkyv's safe `access` API surfaces any alignment
//! mismatch as a checked error rather than UB. See the alignment
//! section of `crate::graph` module docs for the full story.

#[cfg(feature = "data-5km")]
pub const BYTES_5KM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/data/5km.rkyv"));
#[cfg(feature = "data-10km")]
pub const BYTES_10KM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/data/10km.rkyv"));
#[cfg(feature = "data-20km")]
pub const BYTES_20KM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/data/20km.rkyv"));
#[cfg(feature = "data-50km")]
pub const BYTES_50KM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/data/50km.rkyv"));
#[cfg(feature = "data-100km")]
pub const BYTES_100KM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/data/100km.rkyv"));

/// Internal helper used by `Graph::load`'s fallback step. Returns
/// `None` when the given resolution is not compiled in (feature not
/// enabled). Outside the {5,10,20,50,100} set, also returns `None` —
/// callers validate the resolution separately first.
#[allow(unused_variables)] // when no data-* feature is enabled, param is unused
pub(crate) fn bytes_for(resolution_km: u32) -> Option<&'static [u8]> {
    match resolution_km {
        #[cfg(feature = "data-5km")]
        5 => Some(BYTES_5KM),
        #[cfg(feature = "data-10km")]
        10 => Some(BYTES_10KM),
        #[cfg(feature = "data-20km")]
        20 => Some(BYTES_20KM),
        #[cfg(feature = "data-50km")]
        50 => Some(BYTES_50KM),
        #[cfg(feature = "data-100km")]
        100 => Some(BYTES_100KM),
        _ => None,
    }
}
