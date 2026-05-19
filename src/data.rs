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
//! the rustc placement of `[u8; N]` static items, which is byte
//! alignment in the general case. Today's archived schema
//! (`ArchivedGraphData` in [`crate::graph`]) requires 4-byte
//! alignment for its rkyv relative-pointer machinery — bytecheck
//! returns `UnalignedPointer` rather than UB when the alignment is
//! wrong, but that means `Graph::from_bytes` would fail at runtime
//! on a byte-aligned placement.
//!
//! To guarantee 4-byte alignment, each `BYTES_{N}KM` const is
//! exposed as a `&'static [u8]` slice taken from an
//! `Aligned4`-wrapped static. The static's `_align: u32` field
//! forces rustc to place the wrapper at a 4-byte boundary, which
//! the leading byte array inherits. The leading 8-byte file prefix
//! (magic + schema version) then preserves the same alignment for
//! the rkyv payload at byte 8.

/// Wrapper that forces a 4-byte aligned layout. The `_align` field's
/// type (`[u32; 0]`, alignment 4) drives the alignment of the
/// surrounding `repr(C)` struct without contributing any bytes; the
/// `data` field at offset 0 inherits that alignment. Unused when no
/// `data-*` feature is enabled.
#[repr(C)]
#[cfg_attr(
    not(any(
        feature = "data-5km",
        feature = "data-10km",
        feature = "data-20km",
        feature = "data-50km",
        feature = "data-100km"
    )),
    allow(dead_code)
)]
struct Aligned4<const N: usize> {
    _align: [u32; 0],
    data: [u8; N],
}

// Each resolution expands to two items: a private `Aligned4`-wrapped
// const that drives the alignment (see file-level docs), and a public
// `&'static [u8]` slice borrowed from its `data` field. Keeping both
// behind one macro avoids five copies of the same `include_bytes!`
// boilerplate drifting out of sync.
macro_rules! define_bytes {
    ($feature:literal, $raw:ident, $public:ident, $path:literal) => {
        #[cfg(feature = $feature)]
        const $raw: Aligned4<{ include_bytes!(concat!(env!("OUT_DIR"), $path)).len() }> =
            Aligned4 {
                _align: [],
                data: *include_bytes!(concat!(env!("OUT_DIR"), $path)),
            };
        #[cfg(feature = $feature)]
        pub const $public: &[u8] = &$raw.data;
    };
}

define_bytes!("data-5km", RAW_5KM, BYTES_5KM, "/data/5km.rkyv");
define_bytes!("data-10km", RAW_10KM, BYTES_10KM, "/data/10km.rkyv");
define_bytes!("data-20km", RAW_20KM, BYTES_20KM, "/data/20km.rkyv");
define_bytes!("data-50km", RAW_50KM, BYTES_50KM, "/data/50km.rkyv");
define_bytes!("data-100km", RAW_100KM, BYTES_100KM, "/data/100km.rkyv");

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
