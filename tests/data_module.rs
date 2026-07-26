//! ENG-4679: smoke-test the `rustyroute::data` module's feature-gated
//! BYTES_50KM const. Verifies the const is present under default
//! features and starts with the RRG1 magic + schema version 1 prefix.

#[cfg(feature = "data-50km")]
#[test]
fn bytes_50km_present_and_well_formed() {
    use rustyroute::graph::{MAGIC, SCHEMA_VERSION};
    let bytes: &[u8] = rustyroute::data::BYTES_50KM;
    assert!(bytes.len() > 8, "BYTES_50KM smaller than 8-byte header");
    assert_eq!(&bytes[0..4], MAGIC, "BYTES_50KM missing RRG1 magic");
    let ver = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(ver, SCHEMA_VERSION, "BYTES_50KM wrong schema version");
}

/// ENG-4688: `bytes_for` must carry the same
/// `#[cfg(not(target_arch = "wasm32"))]` gate as its only caller,
/// `Graph::load` in `src/loader.rs`. Without it the function is
/// unreachable on wasm32 and rustc's `dead_code` lint fires during the
/// `wasm` CI smoke build.
///
/// Asserted as source text because this test binary is only ever
/// compiled for the host — a `cfg!(target_arch)` check here could never
/// observe the wasm case. Same approach as `tests/lint_state.rs`, which
/// string-asserts the `deny(unsafe_code)` attribute in `src/lib.rs`.
#[test]
fn bytes_for_is_gated_to_non_wasm_targets() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/data.rs");
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let idx = src
        .find("pub(crate) fn bytes_for")
        .expect("src/data.rs must define `pub(crate) fn bytes_for`");
    // Walk backwards over the contiguous run of doc-comment and
    // attribute lines immediately above the signature.
    let has_gate = src[..idx]
        .lines()
        .rev()
        .take_while(|l| {
            let t = l.trim_start();
            t.starts_with("#[") || t.starts_with("//")
        })
        .any(|l| l.contains("#[cfg(not(target_arch = \"wasm32\"))]"));
    assert!(
        has_gate,
        "`bytes_for` must carry #[cfg(not(target_arch = \"wasm32\"))]: its only \
         caller is `Graph::load` (src/loader.rs), which is gated the same way, \
         so on wasm32 the function is dead code and rustc warns during the \
         `wasm` CI smoke build."
    );
}
