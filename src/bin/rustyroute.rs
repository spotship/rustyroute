//! ENG-4682: optional `rustyroute` CLI for one-off route computation.
//!
//! Built only when the `cli` feature is enabled — see the `[[bin]]`
//! target's `required-features` in `Cargo.toml` — so library-only
//! consumers never compile `clap`. The `cli` feature additionally turns
//! on all five `data-{N}km` features: `Graph::load`'s `OUT_DIR` step
//! resolves to a build temp directory that no longer exists after
//! `cargo install`, leaving the static `data::BYTES_{N}KM` slices as the
//! only source, so an installed binary needs every resolution baked in
//! to honour any `--resolution`.
//!
//! # Exit codes
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | route found (also `--help`/`--version`, and a broken output pipe) |
//! | 1    | bad arguments |
//! | 2    | no route between the endpoints |
//! | 3    | graph data unavailable for the requested resolution |
//! | 4    | failed to write output |
//!
//! Results go to stdout; diagnostics go to stderr.

#![deny(unsafe_code)]

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand, ValueEnum};
use rustyroute::{EDGE_GROUPS, Graph, LoadError, Route, RouteError};
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;
use std::str::FromStr;

const EXIT_OK: u8 = 0;
const EXIT_BAD_ARGS: u8 = 1;
const EXIT_NO_ROUTE: u8 = 2;
const EXIT_NO_DATA: u8 = 3;
const EXIT_WRITE_ERROR: u8 = 4;

// Numeric output precision. Coordinates use 6 decimal places (~0.11 m,
// far finer than the coarsest 5 km graph) and `distance_km` uses 3
// (metre precision). Fixed precision keeps output deterministic instead
// of printing the long shortest-roundtrip digit strings that widening
// the graph's `f32` coordinates to `f64` produces (e.g.
// `5.369999885559082`). The values are written as literals in the
// format strings below rather than as consts: a `{x:.CONST$}` precision
// reference cannot be verified without a compiler, and this code was
// authored in an environment with no Rust toolchain.

#[derive(Debug, Parser)]
#[command(
    name = "rustyroute",
    version,
    about = "Maritime sea-routing from the shell",
    after_help = "Exit codes: 0 route found, 1 bad args, 2 no route, \
                  3 data unavailable, 4 output write error."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Compute the shortest maritime route between two points.
    Route(RouteArgs),
}

#[derive(Debug, Args)]
struct RouteArgs {
    /// Origin as `lat,lng` in decimal degrees (e.g. 43.30,5.37).
    #[arg(long, value_name = "LAT,LNG", allow_hyphen_values = true)]
    from: LatLng,

    /// Destination as `lat,lng` in decimal degrees (e.g. 31.23,121.47).
    #[arg(long, value_name = "LAT,LNG", allow_hyphen_values = true)]
    to: LatLng,

    /// Graph resolution in kilometres.
    #[arg(long, value_enum, default_value_t = Resolution::R50)]
    resolution: Resolution,

    /// Comma-separated edge groups to block (e.g. suezCanal,menaiStrait).
    #[arg(long, value_delimiter = ',', value_name = "GROUP[,GROUP...]")]
    block: Vec<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Geojson)]
    format: Format,
}

/// A `lat,lng` pair as accepted on the command line.
///
/// Geographic bounds are deliberately **not** checked here:
/// `Graph::route` already validates them and attributes the failure to
/// the offending endpoint (`RouteError::BadFromCoord` /
/// `BadToCoord`), so duplicating the rule would risk the two drifting.
#[derive(Clone, Copy, Debug)]
struct LatLng {
    lat: f64,
    lng: f64,
}

impl FromStr for LatLng {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (lat_str, lng_str) = s
            .split_once(',')
            .ok_or_else(|| format!("expected `lat,lng`, got `{s}`"))?;
        let lat = lat_str
            .trim()
            .parse::<f64>()
            .map_err(|e| format!("bad latitude `{lat_str}`: {e}"))?;
        let lng = lng_str
            .trim()
            .parse::<f64>()
            .map_err(|e| format!("bad longitude `{lng_str}`: {e}"))?;
        Ok(Self { lat, lng })
    }
}

impl From<LatLng> for (f64, f64) {
    fn from(v: LatLng) -> Self {
        (v.lat, v.lng)
    }
}

/// The five pre-baked graph resolutions. Modelled as a `ValueEnum` so
/// clap validates the value, lists the choices in `--help`, and rejects
/// anything else before `Graph::load` is ever called — which makes
/// `LoadError::UnknownResolution` unreachable in practice.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Resolution {
    #[value(name = "5")]
    R5,
    #[value(name = "10")]
    R10,
    #[value(name = "20")]
    R20,
    #[value(name = "50")]
    R50,
    #[value(name = "100")]
    R100,
}

impl Resolution {
    fn km(self) -> u32 {
        match self {
            Self::R5 => 5,
            Self::R10 => 10,
            Self::R20 => 20,
            Self::R50 => 50,
            Self::R100 => 100,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Format {
    /// Compact JSON object.
    Json,
    /// `GeoJSON` `FeatureCollection` with one `LineString` feature.
    Geojson,
    /// One `lng,lat` per line.
    Line,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            // `print` already routes --help/--version to stdout and real
            // usage errors to stderr. We must not use `Parser::parse`:
            // it exits with clap's own code 2 on a usage error, which
            // would collide with this CLI's "2 = no route".
            let _ = err.print();
            let code = match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => EXIT_OK,
                _ => EXIT_BAD_ARGS,
            };
            return ExitCode::from(code);
        }
    };

    let code = match cli.command {
        Command::Route(args) => run_route(&args),
    };
    ExitCode::from(code)
}

fn run_route(args: &RouteArgs) -> u8 {
    // Trim each name, then drop the empty ones. Trimming matters
    // because `--block "suezCanal, menaiStrait"` is natural shell input
    // and `value_delimiter` splits on the comma alone, leaving a
    // leading space on every name after the first. Dropping empties
    // then covers `--block ""`, a trailing comma
    // (`--block suezCanal,`), and whitespace-only entries — treating
    // those as "nothing to block" is kinder than reporting an unknown
    // group whose name prints as nothing.
    let requested: Vec<&str> = args
        .block
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .collect();

    // Validate `--block` against the baked-in registry before touching
    // the multi-megabyte graph, so a typo fails fast and identically
    // whether or not data for this resolution is compiled in.
    let unknown: Vec<&str> = requested
        .iter()
        .copied()
        .filter(|name| !EDGE_GROUPS.contains(name))
        .collect();
    if !unknown.is_empty() {
        let names = unknown.join(", ");
        eprintln!("error: unknown edge group(s): {names}");
        eprintln!("valid group names:");
        for name in EDGE_GROUPS {
            eprintln!("  {name}");
        }
        return EXIT_BAD_ARGS;
    }

    let resolution_km = args.resolution.km();
    let graph = match Graph::load(resolution_km) {
        Ok(graph) => graph,
        Err(err) => {
            eprintln!("error: {err}");
            return match err {
                // Unreachable: clap constrains --resolution to the
                // supported set before we get here.
                LoadError::UnknownResolution(_) => EXIT_BAD_ARGS,
                _ => EXIT_NO_DATA,
            };
        }
    };

    let blocked = match graph.edges_for_groups(requested.iter().copied()) {
        Ok(blocked) => blocked,
        Err(err) => {
            // Unreachable: the pre-flight check above already rejected
            // unknown names.
            eprintln!("error: {err}");
            return EXIT_BAD_ARGS;
        }
    };

    let route = match graph.route(args.from.into(), args.to.into(), &blocked) {
        Ok(route) => route,
        Err(err) => {
            eprintln!("error: {err}");
            return match err {
                RouteError::NoRoute => EXIT_NO_ROUTE,
                RouteError::BadFromCoord(_)
                | RouteError::BadToCoord(_)
                | RouteError::UnknownGroup(_) => EXIT_BAD_ARGS,
            };
        }
    };

    match write_route(&route, args.format, resolution_km) {
        Ok(()) => EXIT_OK,
        // `rustyroute route ... | head -5` closes the pipe early. That
        // is normal use of a pipe-friendly tool, not an error.
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => EXIT_OK,
        Err(err) => {
            eprintln!("error: writing output: {err}");
            EXIT_WRITE_ERROR
        }
    }
}

fn write_route(route: &Route, format: Format, resolution_km: u32) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    match format {
        Format::Geojson => write_geojson(&mut out, route, resolution_km)?,
        Format::Json => write_json(&mut out, route)?,
        Format::Line => write_lines(&mut out, route)?,
    }
    out.flush()
}

/// Write the path as a JSON array of `[lng, lat]` positions.
///
/// `Route::coordinates` is `(lat, lng)` (see `src/loader.rs`), while
/// `GeoJSON` and both JSON outputs use `lng, lat`. This function is the
/// single place that swap happens.
///
/// `min_positions` pads the array by repeating the last position until
/// it holds at least that many entries. A self-route yields exactly one
/// coordinate, and RFC 7946 §3.1.4 requires a `LineString` to have two
/// or more positions, so the `GeoJSON` writer asks for 2 and gets a valid
/// degenerate zero-length line instead of invalid output.
fn write_coordinates<W: Write>(out: &mut W, route: &Route, min_positions: usize) -> io::Result<()> {
    out.write_all(b"[")?;
    let emitted = route.coordinates.len().max(min_positions);
    for i in 0..emitted {
        if i > 0 {
            out.write_all(b",")?;
        }
        let idx = i.min(route.coordinates.len().saturating_sub(1));
        let (lat, lng) = route.coordinates[idx];
        write!(out, "[{lng:.6},{lat:.6}]")?;
    }
    out.write_all(b"]")
}

fn write_geojson<W: Write>(out: &mut W, route: &Route, resolution_km: u32) -> io::Result<()> {
    let distance_km = route.distance_km;
    out.write_all(
        b"{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\
          \"geometry\":{\"type\":\"LineString\",\"coordinates\":",
    )?;
    write_coordinates(out, route, 2)?;
    write!(
        out,
        "}},\"properties\":{{\"distance_km\":{distance_km:.3},\
         \"resolution\":{resolution_km}}}}}]}}"
    )?;
    out.write_all(b"\n")
}

fn write_json<W: Write>(out: &mut W, route: &Route) -> io::Result<()> {
    let distance_km = route.distance_km;
    write!(out, "{{\"distance_km\":{distance_km:.3},\"coordinates\":")?;
    write_coordinates(out, route, 0)?;
    out.write_all(b"}\n")
}

fn write_lines<W: Write>(out: &mut W, route: &Route) -> io::Result<()> {
    for &(lat, lng) in &route.coordinates {
        writeln!(out, "{lng:.6},{lat:.6}")?;
    }
    Ok(())
}
