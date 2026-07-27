//! ENG-4683: pin `README.md` to the code it documents.
//!
//! The README makes five claims that can silently rot, plus two
//! meta-claims about its own machinery:
//!   AC4: the 13 edge groups it tables -> `readme_edge_group_table_matches_edge_groups_exactly`
//!   AC4: the `pass` tags it credits   -> `readme_edge_group_table_pass_tags_match_pass_groups`
//!   AC5: the axum block it shows      -> `readme_axum_fence_matches_example_file`
//!   AC6: the menaiStrait bbox         -> `readme_documents_menai_bbox`
//!   AC8: the section inventory        -> `readme_sections_appear_in_ticket_order`
//!   AC2: the fences are still live    -> `readme_rust_fences_are_exactly_the_two_expected`
//!   AC2: the doctest anchor exists    -> `lib_rs_compiles_readme_as_doctests`
//!
//! Note which of the two AC2 tests is load-bearing.
//! `lib_rs_compiles_readme_as_doctests` only proves the
//! `#[cfg(doctest)]` anchor is present; it says nothing about whether
//! the fences it points at are still compiled. Retagging the quickstart
//! ```` ```rust,ignore ```` passes that test while silently disabling
//! AC2 entirely. `readme_rust_fences_are_exactly_the_two_expected` is
//! the gate that actually catches it — do not weaken or delete it.
//!
//! `build/groups.rs` is a build-only module and is not linked into this
//! test crate, so the bbox check asserts against its source text — the
//! same technique `tests/data_module.rs:26-50` and `tests/lint_state.rs`
//! use for build-time invariants with no runtime observable.

use std::path::PathBuf;

fn read(rel: &str) -> String {
    let p: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    // CI runs windows-latest (.github/workflows/ci.yaml:55) and the repo
    // has no .gitattributes, so a checkout there may be CRLF. Every
    // comparison in this file is byte-exact, so normalise on read.
    s.replace("\r\n", "\n")
}

/// Body of the first fenced block opened with exactly ```` ```{info} ````.
/// The info-string match is exact, so `rust` does not match `rust,no_run`.
fn fenced_block(md: &str, info: &str) -> Option<String> {
    let open = format!("```{info}");
    let mut lines = md.lines();
    lines.by_ref().find(|l| l.trim_end() == open)?;
    let mut body = String::new();
    for line in lines {
        if line.trim_end() == "```" {
            return Some(body);
        }
        body.push_str(line);
        body.push('\n');
    }
    None
}

/// The edge-group table as `(group, source)` pairs, in document order.
/// Located by its header row rather than a line number.
fn edge_group_rows(readme: &str) -> Vec<(String, String)> {
    let header = "| Group | Source |";
    let start = readme
        .find(header)
        .unwrap_or_else(|| panic!("README.md must contain an edge-group table headed `{header}`"));
    readme[start..]
        .lines()
        .skip(2) // header row + `|---|---|` separator
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            let mut cells = l.split('|').skip(1);
            let group = cells
                .next()
                .unwrap_or_else(|| panic!("malformed table row: {l:?}"))
                .trim();
            let source = cells
                .next()
                .unwrap_or_else(|| panic!("table row missing a Source cell: {l:?}"))
                .trim();
            (group.trim_matches('`').to_string(), source.to_string())
        })
        .collect()
}

/// Every fence's info string, in document order.
fn fence_info_strings(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open = false;
    for line in md.lines() {
        let Some(info) = line.trim_end().strip_prefix("```") else {
            continue;
        };
        if open {
            open = false; // this is a closing fence
        } else {
            open = true;
            out.push(info.to_string());
        }
    }
    out
}

/// AC2's real guard. `lib_rs_compiles_readme_as_doctests` only proves
/// the anchor is present — it says nothing about whether the fence it
/// points at is still *live*.
///
/// Retagging the library quickstart ```` ```rust,ignore ```` is the
/// first forbidden shortcut the spec names, and it is completely
/// silent: the quickstart stops being compiled and run, every other
/// test in this file still passes, and CI stays green. Verified by
/// doing it — `cargo test --doc` went from `1 passed; 1 ignored` to
/// `0 passed; 2 ignored` with all six sibling tests green.
///
/// So pin the policy directly: exactly two `rust`-family fences, in
/// this order, with these exact info strings. `no_run` on the axum
/// fence is required (it binds a port); anything weaker than a bare
/// `rust` on the quickstart means AC2 is not actually being enforced.
#[test]
fn readme_rust_fences_are_exactly_the_two_expected() {
    let readme = read("README.md");
    let rust_fences: Vec<String> = fence_info_strings(&readme)
        .into_iter()
        .filter(|i| i == "rust" || i.starts_with("rust,") || i.starts_with("rust "))
        .collect();
    assert_eq!(
        rust_fences,
        vec!["rust".to_string(), "rust,no_run".to_string()],
        "README.md must carry exactly two Rust fences: the library quickstart as a \
         bare ```rust (compiled AND run by `cargo test --doc` — that is AC2), then \
         the axum example as ```rust,no_run (compiled only; it binds a port). \
         Adding `ignore`/`compile_fail`, adding a third Rust fence, or reordering \
         them all silently weaken the doctest gate."
    );
}

/// AC4. Ordered whole-vector equality, deliberately: `EDGE_GROUPS` is
/// documented as a *stable order* matching `Graph::groups[i]`
/// (build/registry.rs:18-21), so a reordered table is a real defect and
/// a set/`contains` assertion would wave it through.
#[test]
fn readme_edge_group_table_matches_edge_groups_exactly() {
    let readme = read("README.md");
    let header = "| Group | Source |";
    let start = readme
        .find(header)
        .unwrap_or_else(|| panic!("README.md must contain an edge-group table headed `{header}`"));

    let names: Vec<String> = readme[start..]
        .lines()
        .skip(2) // header row + `|---|---|` separator
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            let cell = l
                .split('|')
                .nth(1)
                .unwrap_or_else(|| panic!("malformed table row: {l:?}"))
                .trim();
            cell.trim_matches('`').to_string()
        })
        .collect();

    let expected: Vec<String> = rustyroute::EDGE_GROUPS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(
        names, expected,
        "README.md's edge-group table must match `rustyroute::EDGE_GROUPS` \
         exactly and in order. Regenerate from the constant (build/groups.rs \
         PASS_GROUPS + MENAI_NAME) rather than editing the README by hand."
    );
    assert_eq!(names.len(), 13, "there are exactly 13 edge groups");
}

/// AC5. The example is the source of truth; the README copy is what OSS
/// readers actually paste. They must be byte-identical.
#[test]
fn readme_axum_fence_matches_example_file() {
    let readme = read("README.md");
    let example = read("examples/axum_server.rs");
    let fence = fenced_block(&readme, "rust,no_run")
        .expect("README.md must contain a ```rust,no_run fence holding the axum example");
    assert_eq!(
        fence, example,
        "the README's ```rust,no_run fence has drifted from \
         examples/axum_server.rs. Re-sync it from the file — see the \
         splice snippet in the ENG-4683 plan, Task 4 Step 2."
    );
}

/// AC6. Those four numbers are the only reason group 13 exists.
///
/// Asserts the whole bbox phrase, not the four literals separately: a
/// bare `readme.contains("-4.20")` sweep would still pass if the README
/// swapped the axes (`lat ∈ [-4.20, -4.00], lng ∈ [53.13, 53.30]`),
/// which is exactly the mistake worth catching — the longitudes and
/// latitudes here are not interchangeable.
#[test]
fn readme_documents_menai_bbox() {
    let readme = read("README.md");
    let groups_rs = read("build/groups.rs");

    let bounds = [
        ("MENAI_LNG_MIN", "-4.20"),
        ("MENAI_LNG_MAX", "-4.00"),
        ("MENAI_LAT_MIN", "53.13"),
        ("MENAI_LAT_MAX", "53.30"),
    ];
    for (konst, literal) in bounds {
        assert!(
            groups_rs.contains(&format!("{konst}: f64 = {literal};")),
            "build/groups.rs no longer defines `{konst} = {literal}` — the README \
             bbox and this test must both be updated to the new value"
        );
    }

    let [lng_min, lng_max, lat_min, lat_max] = bounds.map(|(_, literal)| literal);
    let phrase = format!("`lng ∈ [{lng_min}, {lng_max}]`, `lat ∈ [{lat_min}, {lat_max}]`");
    assert!(
        readme.contains(&phrase),
        "README.md must state the menaiStrait bbox as `{phrase}` — with each bound \
         on its own axis. build/groups.rs:35-38 is the source of truth."
    );
}

/// AC4, second column. The group table also claims which upstream
/// `pass` tag feeds each of the first twelve groups; those claims are
/// checkable against `build/groups.rs`'s `PASS_GROUPS` and would
/// otherwise be the one part of the table nothing verifies.
#[test]
fn readme_edge_group_table_pass_tags_match_pass_groups() {
    let readme = read("README.md");
    let groups_rs = read("build/groups.rs");

    // Parse `("suez", "suezCanal"),` pairs out of the PASS_GROUPS array.
    let start = groups_rs
        .find("pub const PASS_GROUPS")
        .expect("build/groups.rs must define PASS_GROUPS");
    let body = &groups_rs[start
        ..start
            + groups_rs[start..]
                .find("];")
                .expect("PASS_GROUPS array must be terminated")];
    let pairs: Vec<(String, String)> = body
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let inner = l.strip_prefix('(')?.split_once("),")?.0;
            let (tag, public) = inner.split_once(',')?;
            Some((
                tag.trim().trim_matches('"').to_string(),
                public.trim().trim_matches('"').to_string(),
            ))
        })
        .collect();
    assert_eq!(
        pairs.len(),
        12,
        "expected 12 PASS_GROUPS entries, parsed {pairs:?}"
    );

    // Row-by-row, not a whole-file `contains` sweep: a bare `contains`
    // finds every tag *somewhere* and so would pass happily if two rows
    // had their Source cells swapped — and the column-1 test above only
    // reads group names, so nothing else would catch it either.
    let rows = edge_group_rows(&readme);
    for (tag, public) in pairs {
        let source = rows
            .iter()
            .find(|(group, _)| *group == public)
            .map(|(_, source)| source.as_str())
            .unwrap_or_else(|| panic!("README.md has no edge-group row for `{public}`"));
        // The full phrase, not just the backticked tag: these twelve
        // groups are `pass`-tag derived and `menaiStrait` is the only
        // bbox one, so a cell reading "bbox around `suez`" would be a
        // real misattribution that a bare tag match would wave through.
        assert!(
            source.contains(&format!("`pass` tag `{tag}`")),
            "README.md's `{public}` row must credit upstream `pass` tag `{tag}` \
             (build/groups.rs PASS_GROUPS), but its Source cell reads: {source:?}"
        );
    }
}

/// AC8. The ticket fixes both the set of sections and their order.
/// Asserted as a whole vector so an inserted, renamed, dropped, or
/// reordered heading all fail loudly.
#[test]
fn readme_sections_appear_in_ticket_order() {
    let readme = read("README.md");
    let headings: Vec<&str> = readme
        .lines()
        .filter(|l| l.starts_with("# ") || l.starts_with("## "))
        .collect();
    let expected = [
        "# rustyroute",
        "## Quickstart (library)",
        "## Quickstart (HTTP server with axum)",
        "## CLI",
        "## How the data is built",
        "## Data and features",
        "## Edge groups",
        "## Performance",
        "## License and attribution",
        "## Contributing",
        "## Security",
    ];
    assert_eq!(
        headings, expected,
        "README.md's sections must match the ENG-4683 inventory, in order"
    );
}

/// Guard the guard: without the anchor, `cargo test --doc` compiles
/// none of the README's fences and AC2 silently lapses.
#[test]
fn lib_rs_compiles_readme_as_doctests() {
    let lib = read("src/lib.rs");
    assert!(
        lib.contains("#[cfg(doctest)]") && lib.contains("include_str!(\"../README.md\")"),
        "src/lib.rs must keep the `#[cfg(doctest)] #[doc = include_str!(\"../README.md\")]` \
         anchor — without it `cargo test --doc` compiles none of the README's fences \
         and the quickstarts can rot (ENG-4683 AC2)."
    );
}
