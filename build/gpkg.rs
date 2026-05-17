//! Open a vendored `.gpkg` and iterate (fid, pass, points) triples.

use crate::build::geometry::parse_gpb_linestring;
use std::path::Path;

pub struct RawEdge {
    pub fid: i64,
    pub pass: Option<String>,
    /// LineString vertices in (lng, lat) order. Length >= 2.
    pub points: Vec<(f64, f64)>,
}

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
        let points =
            parse_gpb_linestring(&blob).map_err(|e| format!("parse fid={fid}: {e}"))?;
        if points.len() < 2 {
            panic!("build.rs: fid={fid} has <2 points; expected LineString");
        }
        out.push(RawEdge { fid, pass, points });
    }
    Ok(out)
}
