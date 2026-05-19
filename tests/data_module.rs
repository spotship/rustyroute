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
