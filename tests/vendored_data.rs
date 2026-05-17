//! Integration tests that lock in the vendored Eurostat MARNET GeoPackage
//! files and their attribution paperwork. These exist because the data IS
//! the contract for ENG-4677 — the `build.rs` introduced in ENG-4678 and
//! downstream consumers depend on these exact bytes. If a refactor
//! accidentally modifies, renames, or removes a vendored file, or if
//! attribution paperwork drifts out of sync, these tests fail loudly.
//!
//! No new runtime dependencies are introduced by THIS file — SHA-256 is
//! implemented inline per FIPS 180-4 because the ENG-4677 ticket forbade
//! adding `Cargo.toml` deps. The implementation lives only in `tests/`
//! and is not compiled into the crate. ENG-4678 has since added runtime
//! deps for the graph build; the inline SHA-256 stays for hermeticity.
//!
//! Acceptance criteria mapping (see `.ship/tasks/eng-4677-.../plan/spec.md`):
//!   AC1: file presence + filenames        -> `files_exist_with_expected_names`
//!   AC1+AC3: documented byte sizes        -> `file_sizes_match_spec_and_readme`
//!   AC2: byte content matches upstream    -> `sha256_matches_readme_and_pinned_values`
//!   AC3: README provenance complete       -> `readme_documents_required_provenance`
//!   AC4: NOTICE credits Eurostat MARNET   -> `notice_credits_eurostat_marnet`
//!   AC6: no Git LFS configured            -> `no_git_lfs_for_vendored_data`
//!   (AC5 — `cargo build` succeeds — is implicit: this test crate only
//!    compiles + runs if the workspace builds. CI's `cargo build --offline`
//!    covers the offline part.)
//!
//! Supporting / sanity checks (not tied to a numbered AC):
//!   format sanity for .gpkg bytes  -> `gpkg_magic_header_present`
//!   self-tests for inline SHA-256  -> `sha256_known_answer_empty`,
//!                                     `sha256_known_answer_abc`,
//!                                     `sha256_known_answer_longer`

use std::fs;
use std::path::PathBuf;

/// Pinned upstream commit on `eurostat/searoute`. Sourced from the spec; do
/// not edit casually — changing this without re-vendoring breaks provenance.
const UPSTREAM_COMMIT: &str = "88a2e568a8e0144d1f5a81c3931a7bc2bcce6901";

/// Vendored files, with the (filename, expected byte size, expected SHA-256)
/// recorded at vendor time. These three values are independent integrity
/// witnesses — the size matches the upstream git tree blob size, and the
/// SHA-256 matches the documented value in `vendor/eurostat-marnet/README.md`.
const VENDORED: &[(&str, u64, &str)] = &[
    (
        "marnet_plus_5km.gpkg",
        6_963_200,
        "75ca6cc130b3748f568196a9caf62889e9f096ec358f65223b4095ac070be0e7",
    ),
    (
        "marnet_plus_10km.gpkg",
        4_661_248,
        "738cf880623dee8be3616a3ca46b565cc47a6548003ebd4438fea309dc7fa4f9",
    ),
    (
        "marnet_plus_20km.gpkg",
        2_891_776,
        "8d909ba49d5062e1bdd7b305ee768e3366f1eab2d04b4ac448dfbdf9bf71a915",
    ),
    (
        "marnet_plus_50km.gpkg",
        1_552_384,
        "44a309ffa0fbdc02e00616ed6a9e20ad92c773994b8f4502995cee9ffdf96acf",
    ),
    (
        "marnet_plus_100km.gpkg",
        1_024_000,
        "d0cd431c40efc2aa2cb1c218e0bf0590ef8dd0ba30eee157abe848900b16f3af",
    ),
];

fn repo_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is the workspace root for integration tests in a
    // single-package crate.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn vendor_dir() -> PathBuf {
    repo_root().join("vendor").join("eurostat-marnet")
}

#[test]
fn files_exist_with_expected_names() {
    let dir = vendor_dir();
    assert!(
        dir.is_dir(),
        "vendor directory missing: {} (AC1)",
        dir.display()
    );
    for (name, _, _) in VENDORED {
        let p = dir.join(name);
        assert!(p.is_file(), "vendored file missing: {} (AC1)", p.display());
    }
}

#[test]
fn file_sizes_match_spec_and_readme() {
    for (name, expected_size, _) in VENDORED {
        let p = vendor_dir().join(name);
        let actual = fs::metadata(&p)
            .unwrap_or_else(|e| panic!("stat {}: {e}", p.display()))
            .len();
        assert_eq!(
            actual, *expected_size,
            "size drift for {name}: expected {expected_size} bytes, got {actual} \
             (AC1+AC2 — bytes must match upstream commit {UPSTREAM_COMMIT})"
        );
    }
}

#[test]
fn gpkg_magic_header_present() {
    // GeoPackage files are SQLite 3 databases. SQLite's file format begins
    // with the 16-byte ASCII string "SQLite format 3\0". This is a cheap
    // sanity check that file content wasn't replaced by a different format
    // (e.g. a JSON placeholder, a text README, or a corrupted truncation).
    // Only the first 16 bytes are needed — reading the whole multi-MB file
    // would be wasteful given `sha256_matches_readme_and_pinned_values`
    // already covers full-content integrity.
    use std::io::Read;
    const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
    for (name, _, _) in VENDORED {
        let p = vendor_dir().join(name);
        let mut head = [0u8; SQLITE_MAGIC.len()];
        let mut f = fs::File::open(&p).unwrap_or_else(|e| panic!("open {}: {e}", p.display()));
        f.read_exact(&mut head)
            .unwrap_or_else(|e| panic!("{name} truncated below SQLite header length: {e}"));
        assert_eq!(
            &head, SQLITE_MAGIC,
            "{name} does not start with SQLite/GeoPackage magic header"
        );
    }
}

#[test]
fn sha256_matches_readme_and_pinned_values() {
    for (name, _, expected_hex) in VENDORED {
        let p = vendor_dir().join(name);
        let bytes = fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        let actual = sha256_hex(&bytes);
        assert_eq!(
            actual.as_str(),
            *expected_hex,
            "SHA-256 drift for {name} (AC2 — bytes must match upstream \
             commit {UPSTREAM_COMMIT}). Expected {expected_hex}, got {actual}"
        );
    }
}

#[test]
fn readme_documents_required_provenance() {
    let readme_path = vendor_dir().join("README.md");
    let readme = fs::read_to_string(&readme_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", readme_path.display()));

    // AC3: source URL (the raw.githubusercontent.com URL or repo URL)
    assert!(
        readme.contains("https://github.com/eurostat/searoute"),
        "README missing upstream repository URL (AC3)"
    );

    // AC3: pinned source commit SHA
    assert!(
        readme.contains(UPSTREAM_COMMIT),
        "README missing pinned upstream commit SHA {UPSTREAM_COMMIT} (AC3)"
    );

    // AC3: license declaration
    assert!(
        readme.contains("EUPL-1.2"),
        "README missing EUPL-1.2 license declaration (AC3)"
    );

    // AC3: explicit "unmodified" / "byte-for-byte" statement
    let lower = readme.to_lowercase();
    assert!(
        lower.contains("byte-for-byte") || lower.contains("unmodified"),
        "README must state files are unmodified / byte-for-byte (AC3)"
    );

    // AC3: download date — look for an ISO-8601 calendar date YYYY-MM-DD with
    // a plausible 21st-century year and a real month/day range. A pure shape
    // check (`/^\d{4}-\d{2}-\d{2}$/`) would happily accept `9999-99-99` and
    // give false confidence that a real date was recorded.
    let has_iso_date = readme.split_whitespace().any(|tok| {
        let t = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
        if t.len() != 10 || &t[4..5] != "-" || &t[7..8] != "-" {
            return false;
        }
        let (Ok(y), Ok(m), Ok(d)) = (
            t[..4].parse::<u32>(),
            t[5..7].parse::<u32>(),
            t[8..10].parse::<u32>(),
        ) else {
            return false;
        };
        (2000..=2099).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d)
    });
    assert!(
        has_iso_date,
        "README missing a plausible ISO-8601 download date YYYY-MM-DD (AC3)"
    );

    // AC3: provenance chain mentions Eurostat and Oak Ridge (the upstream chain)
    assert!(
        readme.contains("Eurostat"),
        "README missing Eurostat in provenance chain (AC3)"
    );
    assert!(
        lower.contains("oak ridge"),
        "README missing Oak Ridge National Labs in provenance chain (AC3)"
    );

    // AC3: each file must be referenced by name in the README, with its size
    // and SHA-256 also documented. The size check is row-level (filename and
    // size on the same line, size as a standalone token) so that
    // `readme.contains("1024000")` cannot pass by matching a substring inside
    // an unrelated longer number such as `10240000`.
    let size_re = |size: u64| {
        let s = size.to_string();
        move |line: &str| {
            line.split(|c: char| !c.is_ascii_digit())
                .any(|tok| tok == s)
        }
    };
    for (name, size, sha) in VENDORED {
        assert!(
            readme.contains(name),
            "README does not mention vendored file `{name}` (AC3)"
        );
        let row_has_size = readme
            .lines()
            .any(|line| line.contains(name) && size_re(*size)(line));
        assert!(
            row_has_size,
            "README does not document size {size} on the same row as {name} (AC3)"
        );
        assert!(
            readme.contains(sha),
            "README does not document SHA-256 {sha} for {name} (AC3)"
        );
    }
}

#[test]
fn notice_credits_eurostat_marnet() {
    let notice_path = repo_root().join("NOTICE");
    let notice = fs::read_to_string(&notice_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", notice_path.display()));

    // AC4: NOTICE must specifically credit Eurostat for the MARNET data,
    // not just for the algorithm.
    assert!(
        notice.contains("Eurostat"),
        "NOTICE missing Eurostat attribution (AC4)"
    );
    assert!(
        notice.contains("MARNET"),
        "NOTICE must credit Eurostat for MARNET data specifically (AC4)"
    );

    // AC4: NOTICE must reference the pinned source commit so the data
    // provenance is unambiguous.
    assert!(
        notice.contains(UPSTREAM_COMMIT),
        "NOTICE missing pinned upstream commit SHA {UPSTREAM_COMMIT} (AC4)"
    );

    // AC4: the obsolete "does not vendor" sentence must be removed.
    assert!(
        !notice.contains("does not vendor"),
        "NOTICE still contains obsolete 'does not vendor' sentence (AC4)"
    );

    // AC4: NOTICE must mention each of the five vendored filenames so the
    // attribution lines up with what's actually on disk.
    for (name, _, _) in VENDORED {
        assert!(
            notice.contains(name),
            "NOTICE missing reference to vendored file `{name}` (AC4)"
        );
    }
}

#[test]
fn no_git_lfs_for_vendored_data() {
    // AC6: Git LFS must not be configured. There must be no .gitattributes
    // entry that routes the vendored .gpkg files through LFS, and the files
    // themselves must not be LFS pointer stubs.
    let root = repo_root();

    // Inspect both possible .gitattributes locations.
    for ga in &[
        root.join(".gitattributes"),
        vendor_dir().join(".gitattributes"),
    ] {
        if ga.exists() {
            let content =
                fs::read_to_string(ga).unwrap_or_else(|e| panic!("read {}: {e}", ga.display()));
            assert!(
                !content.contains("filter=lfs"),
                "{} configures Git LFS — forbidden by AC6",
                ga.display()
            );
        }
    }

    // LFS pointer stubs are tiny text files (<200 bytes) that start with
    // "version https://git-lfs.github.com/spec/". A real .gpkg here is
    // megabytes of SQLite binary, so any such stub means LFS replaced the
    // real bytes.
    const LFS_POINTER_PREFIX: &[u8] = b"version https://git-lfs";
    for (name, _, _) in VENDORED {
        let p = vendor_dir().join(name);
        let mut head = vec![0u8; LFS_POINTER_PREFIX.len()];
        use std::io::Read;
        let mut f = fs::File::open(&p).unwrap_or_else(|e| panic!("open {}: {e}", p.display()));
        let n = f
            .read(&mut head)
            .unwrap_or_else(|e| panic!("read head {}: {e}", p.display()));
        head.truncate(n);
        assert_ne!(
            head.as_slice(),
            LFS_POINTER_PREFIX,
            "{name} appears to be a Git LFS pointer stub — AC6 forbids LFS"
        );
    }
}

// -----------------------------------------------------------------------------
// SHA-256 (FIPS 180-4) — inline so we don't add a dependency. Constant-time
// is not required (no secret data); the inputs are public vendored bytes.
// Algorithm reference: https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf
// -----------------------------------------------------------------------------

fn sha256_hex(data: &[u8]) -> String {
    let digest = sha256(data);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push(HEX[(b >> 4) as usize]);
        s.push(HEX[(b & 0x0f) as usize]);
    }
    s
}

const HEX: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

fn sha256(msg: &[u8]) -> [u8; 32] {
    // Pre-processing: pad the message so total length is a multiple of 512
    // bits. Append 0x80, then zeros, then the 64-bit big-endian length.
    let bit_len = (msg.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(msg.len() + 72);
    padded.extend_from_slice(msg);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut h = H0;
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

// Built-in unit tests for the SHA-256 implementation itself. If these break,
// the implementation regressed and the vendored-data tests above can't be
// trusted.
#[test]
fn sha256_known_answer_empty() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn sha256_known_answer_abc() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn sha256_known_answer_longer() {
    // FIPS 180-2 test vector (56-byte input straddles the 55-byte boundary).
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}
