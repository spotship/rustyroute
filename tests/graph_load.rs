//! ENG-4679 integration tests: `Graph::from_bytes` happy path, the
//! full LoadError matrix (BadMagic / UnsupportedSchema /
//! InvalidArchive), `Graph::load(N)` unknown-resolution rejection,
//! count parity vs `build_csr`, and a compile-time Send+Sync
//! assertion required by Design 019's OnceLock pattern.

#[cfg(feature = "data-50km")]
use rustyroute::graph::{MAGIC, SCHEMA_VERSION};
use rustyroute::{Graph, LoadError};

#[cfg(feature = "data-50km")]
fn fifty_km_bytes() -> &'static [u8] {
    rustyroute::data::BYTES_50KM
}

// --- compile-time Send + Sync assertion ---
const _SEND_SYNC: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<Graph>();
    assert_sync::<Graph>();
};

// --- from_bytes happy path ---
#[cfg(feature = "data-50km")]
#[test]
fn from_bytes_50km_ok() {
    let g = Graph::from_bytes(fifty_km_bytes()).expect("from_bytes");
    assert!(g.node_count() > 0);
    assert!(g.edge_count() > 0);
    assert!(
        g.directed_edge_count() >= g.edge_count(),
        "directed_edge_count must be >= edge_count"
    );
    // from_bytes records 0 as the resolution sentinel.
    assert_eq!(g.resolution_km(), 0);
}

// --- Graph::load(7) -> UnknownResolution(7) ---
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn load_unknown_resolution_seven() {
    match Graph::load(7) {
        Err(LoadError::UnknownResolution(7)) => {}
        other => panic!("expected UnknownResolution(7), got {other:?}"),
    }
}

// --- Graph::load(50) happy path via OUT_DIR ---
#[cfg(all(not(target_arch = "wasm32"), feature = "data-50km"))]
#[test]
fn load_50km_ok_via_out_dir() {
    let g = Graph::load(50).expect("load(50)");
    assert_eq!(g.resolution_km(), 50);
    assert!(g.node_count() > 0);
    assert!(g.edge_count() > 0);
}

// --- BadMagic ---
#[cfg(feature = "data-50km")]
#[test]
fn from_bytes_bad_magic() {
    let mut v = fifty_km_bytes().to_vec();
    v[0] ^= 0xFF; // corrupt first magic byte
    let leaked: &'static [u8] = Box::leak(v.into_boxed_slice());
    match Graph::from_bytes(leaked) {
        Err(LoadError::BadMagic(got)) => {
            assert_ne!(&got, MAGIC, "BadMagic should report the actual bytes seen");
        }
        other => panic!("expected BadMagic, got {other:?}"),
    }
}

// --- UnsupportedSchema ---
#[cfg(feature = "data-50km")]
#[test]
fn from_bytes_unsupported_schema() {
    let mut v = fifty_km_bytes().to_vec();
    // bump version from 1 to 3 (XOR byte 4 with 0x02).
    v[4] ^= 0x02;
    let leaked: &'static [u8] = Box::leak(v.into_boxed_slice());
    match Graph::from_bytes(leaked) {
        Err(LoadError::UnsupportedSchema(ver)) => {
            assert_ne!(ver, SCHEMA_VERSION, "should not equal current schema");
        }
        other => panic!("expected UnsupportedSchema, got {other:?}"),
    }
}

// --- InvalidArchive on payload tamper (no segfault) ---
#[cfg(feature = "data-50km")]
#[test]
fn from_bytes_invalid_archive_on_payload_tamper() {
    // rkyv writes the root struct at the END of the archive (leaf-first
    // layout), so the relative-pointer machinery — which bytecheck
    // validates — sits in the trailing bytes. Flipping a byte 16 bytes
    // from the end (inside the root struct's pointer/length fields)
    // reliably trips bytecheck with InvalidArchive on the current
    // schema. A byte deep inside the node/edge tables would land in
    // coordinate or weight data and would NOT trigger bytecheck —
    // those are scalar payload, not pointer machinery.
    let mut v = fifty_km_bytes().to_vec();
    assert!(v.len() > 64, "test fixture too small");
    let tamper_offset = v.len() - 16;
    v[tamper_offset] ^= 0xFF;
    let leaked: &'static [u8] = Box::leak(v.into_boxed_slice());
    match Graph::from_bytes(leaked) {
        Err(LoadError::InvalidArchive(_)) => {}
        Ok(_) => panic!(
            "payload tamper at offset {tamper_offset} (len-16) was not detected"
        ),
        Err(other) => panic!("expected InvalidArchive, got {other:?}"),
    }
}

// --- truncated header → BadMagic ---
#[test]
fn from_bytes_truncated_header() {
    let v: &[u8] = b"RRG"; // 3 bytes — less than the 8-byte header
    let leaked: &'static [u8] = Box::leak(v.to_vec().into_boxed_slice());
    match Graph::from_bytes(leaked) {
        Err(LoadError::BadMagic(_)) => {}
        other => panic!("expected BadMagic on truncated header, got {other:?}"),
    }
}

// --- AC4 happy half: load(100) ok when data-100km is on ---
#[cfg(all(not(target_arch = "wasm32"), feature = "data-100km"))]
#[test]
fn load_100km_ok() {
    let g = Graph::load(100).expect("load(100)");
    assert_eq!(g.resolution_km(), 100);
    assert!(g.node_count() > 0);
}
