//! GeoPackage SQLite reader. Depends on `rusqlite`, declared under
//! both `[build-dependencies]` (for `build.rs`) and `[dev-dependencies]`
//! (so `tests/tampered_gpkg_panic.rs` can re-include this module via
//! `#[path]` and drive the real reader against a tampered `.gpkg`
//! copy). This file is never compiled into the library crate.

use crate::build::geometry::parse_gpb_linestring;
use crate::build::gpkg::RawEdge;
use std::path::Path;

pub fn iter_edges(path: &Path) -> Result<Vec<RawEdge>, String> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_URI
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("open sqlite: {e}"))?;

    let mut stmt = conn
        .prepare("SELECT fid, geometry, pass FROM type ORDER BY fid")
        .map_err(|e| format!("prepare: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let fid: i64 = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            let pass: Option<String> = row.get(2)?;
            Ok((fid, blob, pass))
        })
        .map_err(|e| format!("query: {e}"))?;

    let mut out = Vec::new();
    for r in rows {
        let (fid, blob, pass) = r.map_err(|e| format!("row: {e}"))?;
        let points = parse_gpb_linestring(&blob).map_err(|e| format!("parse fid={fid}: {e}"))?;
        if points.len() < 2 {
            return Err(format!(
                "fid={fid}: LineString has {} point(s); expected >=2",
                points.len()
            ));
        }
        out.push(RawEdge { fid, pass, points });
    }
    Ok(out)
}
