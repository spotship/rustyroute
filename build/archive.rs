//! Serialise a `Graph` to `{OUT_DIR}/data/{N}km.rkyv`.
//!
//! File layout (matches `src/graph.rs` docs):
//!   bytes 0..4   : ASCII magic b"RRG1"
//!   bytes 4..8   : SCHEMA_VERSION as u32 LE
//!   bytes 8..    : rkyv bytes of Archived<Graph>

use crate::graph::{GraphData, MAGIC, SCHEMA_VERSION};
use std::path::Path;

pub fn write_archive(path: &Path, graph: &GraphData) -> Result<(), String> {
    let payload =
        rkyv::to_bytes::<rkyv::rancor::Error>(graph).map_err(|e| format!("rkyv serialise: {e}"))?;

    let mut buf = Vec::with_capacity(8 + payload.len());
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    buf.extend_from_slice(&payload);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    std::fs::write(path, &buf).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}
