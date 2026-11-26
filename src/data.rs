//! Feature-gated static byte slices for each pre-baked graph
//! resolution.
//!
//! Each `BYTES_{N}KM` const is `include_bytes!`-baked at rustyroute's
//! compile time from `$OUT_DIR/data/{N}km.rkyv` produced by `build.rs`.
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
//! `Aligned4`-wrapped static. The wrapper carries a zero-sized
//! `_align: [u32; 0]` field whose `u32` element type has alignment
//! 4; combined with `#[repr(C)]`, that lifts the struct's alignment
//! to 4 without contributing any bytes, and the `data: [u8; N]`
//! field at offset 0 inherits the boundary. The leading 8-byte
//! file prefix (magic + schema version) then preserves the same
//! alignment for the rkyv payload at byte 8.
//!
//! # Single-include invariant
//!
//! Each rkyv archive is referenced by exactly one `include_bytes!`
//! call per resolution. The const-generic length parameter of
//! `Aligned4` comes from the build-script-generated
//! `$OUT_DIR/data_lens.rs` (which writes one `DATA_LEN_{N}KM:
//! usize` per resolution from the archive's on-disk size), not from
//! a second `include_bytes!(...).len()` expansion. This avoids the
//! risk of rustc embedding the archive bytes twice when the
//! optimiser cannot prove the two expansions are identical.

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

// `DATA_LEN_{5,10,20,50,100}KM` consts emitted by `build/mod.rs`.
// Driving the `Aligned4<{N}>` const-generic from these constants
// (instead of a second `include_bytes!(...).len()`) keeps each
// archive referenced by exactly one `include_bytes!` per resolution.
include!(concat!(env!("OUT_DIR"), "/data_lens.rs"));

// Each resolution expands to two items: a private `Aligned4`-wrapped
// const that drives the alignment (see file-level docs), and a public
// `&'static [u8]` slice borrowed from its `data` field. Keeping both
// behind one macro avoids five copies of the same `include_bytes!`
// boilerplate drifting out of sync.
// The `#[doc = concat!(...)]` on the public slice is what satisfies the
// crate's `#![deny(missing_docs)]` for all five resolutions in one edit
// (ENG-4684). Documenting the generator rather than five generated sites
// follows the same reasoning as `build/registry.rs`'s doc comment on the
// generated `EDGE_GROUPS` const. `$res` exists purely so the rendered
// prose can name the resolution in human form.
macro_rules! define_bytes {
    ($feature:literal, $raw:ident, $public:ident, $len:ident, $res:literal, $path:literal) => {
        #[cfg(feature = $feature)]
        const $raw: Aligned4<{ $len }> = Aligned4 {
            _align: [],
            data: *include_bytes!(concat!(env!("OUT_DIR"), $path)),
        };
        #[cfg(feature = $feature)]
        #[doc = concat!(
                    "The complete ", $res, " km graph archive, as a 4-byte-aligned ",
                    "`&'static [u8]`.\n\n",
                    "`include_bytes!`-baked at rustyroute's own compile time from ",
                    "`$OUT_DIR", $path, "`, and therefore present in the library binary ",
                    "whether or not the consumer has a data directory. Requires the ",
                    "`", $feature, "` Cargo feature (`data-50km` is the only one enabled ",
                    "by `default`).\n\n",
                    "The bytes are the on-disk archive format described in ",
                    "[`crate::graph`]: a 4-byte `b\"RRG1\"` magic, a little-endian `u32` ",
                    "[`SCHEMA_VERSION`](crate::graph::SCHEMA_VERSION), then the rkyv ",
                    "payload. Pass the whole slice to ",
                    "[`Graph::from_bytes`](crate::Graph::from_bytes) — which validates the ",
                    "prefix and the payload — rather than slicing it by hand.\n\n",
                    "Alignment matters: rkyv's relative pointers need a 4-byte-aligned ",
                    "payload, so the underlying static is wrapped in `Aligned4` (see the ",
                    "module docs). Copying these bytes into a `Vec<u8>` may lose that ",
                    "alignment and make `from_bytes` fail with ",
                    "[`LoadError::InvalidArchive`](crate::LoadError::InvalidArchive).",
                )]
        pub const $public: &[u8] = &$raw.data;
    };
}

define_bytes!(
    "data-5km",
    RAW_5KM,
    BYTES_5KM,
    DATA_LEN_5KM,
    "5",
    "/data/5km.rkyv"
);
define_bytes!(
    "data-10km",
    RAW_10KM,
    BYTES_10KM,
    DATA_LEN_10KM,
    "10",
    "/data/10km.rkyv"
);
define_bytes!(
    "data-20km",
    RAW_20KM,
    BYTES_20KM,
    DATA_LEN_20KM,
    "20",
    "/data/20km.rkyv"
);
define_bytes!(
    "data-50km",
    RAW_50KM,
    BYTES_50KM,
    DATA_LEN_50KM,
    "50",
    "/data/50km.rkyv"
);
define_bytes!(
    "data-100km",
    RAW_100KM,
    BYTES_100KM,
    DATA_LEN_100KM,
    "100",
    "/data/100km.rkyv"
);

/// Internal helper used by `Graph::load`'s fallback step. Returns
/// `None` when the given resolution is not compiled in (feature not
/// enabled). Outside the {5,10,20,50,100} set, also returns `None` —
/// callers validate the resolution separately first.
///
/// Native-only (ENG-4688). Its sole caller is `Graph::load` in
/// `src/loader.rs`, which is itself gated to non-wasm32 targets;
/// without the matching gate here the function is unreachable on
/// wasm32 and rustc's `dead_code` lint fires during the `wasm` CI
/// smoke build. wasm consumers use `Graph::from_bytes`, which takes
/// the byte slice directly and never goes through this helper.
#[cfg(not(target_arch = "wasm32"))]
// When no `data-*` feature is enabled, every match arm below is
// `cfg`-ed out and the parameter goes unused.
#[allow(unused_variables)]
// NOT made `const fn`, even though it could be. `tests/data_module.rs`'s
// ENG-4688 wasm-gate guard asserts on the literal source text
// `"pub(crate) fn bytes_for"` and walks the attribute run above it to
// prove the `#[cfg(not(target_arch = "wasm32"))]` is still attached.
// Adding `const` renames the signature out from under that guard, which
// would trade a live regression test for a nursery cosmetic.
#[allow(clippy::missing_const_for_fn)]
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
