//! ENG-4679: verify the crate-level unsafe-code lint is `deny`, not
//! `forbid`. The mmap-based `Graph::load` needs a single targeted
//! `#[allow(unsafe_code)]` on the `memmap2::Mmap::map(&file)` call,
//! which `forbid` would reject.
//!
//! ENG-4684 extends this file to lock the *whole* lint posture, not just
//! the unsafe-code half:
//!
//!   - the `#![deny(...)]` group in `src/lib.rs` names `missing_docs`,
//!     `missing_debug_implementations` and
//!     `rustdoc::broken_intra_doc_links` alongside `unsafe_code`;
//!   - `src/lib.rs` warns both `clippy::pedantic` and `clippy::nursery`;
//!   - `Cargo.toml` carries a `[lints.clippy]` table warning `pedantic`
//!     package-wide, because `#![...]` in `src/lib.rs` reaches only the
//!     library crate and would leave `build.rs`, `tests/*`, `benches/`
//!     and `src/bin/` gated by a single line of CI YAML.
//!
//! Everything here is a source-text assertion, deliberately: the lints
//! are *configuration*, so a compile-time check cannot tell "the gate is
//! on" from "the gate is off and nothing happens to violate it". If one
//! of these declarations is deleted, CI stays green and the regression is
//! silent — which is exactly the drift this file exists to catch. This
//! file may only ever be changed in the direction of asserting MORE.

/// Collapse every run of ASCII whitespace to a single space so the
/// assertions below are insensitive to how `rustfmt` (or a human) chooses
/// to break a multi-line inner attribute. `#![deny(\n    unsafe_code,`
/// and `#![deny(unsafe_code,` must satisfy the same check — the intent is
/// "the deny group names this lint", not "the file contains this exact
/// byte sequence".
fn squeeze_whitespace(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_ws = false;
    for c in src.chars() {
        if c.is_whitespace() {
            if !in_ws {
                out.push(' ');
            }
            in_ws = true;
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

fn lib_rs() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn cargo_toml() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn lib_rs_uses_deny_not_forbid_unsafe_code() {
    let src = lib_rs();
    // Match the inner-attribute prefix and leave the closing paren
    // open so additional lints in the same attribute group don't
    // break the check — e.g. `#![deny(unsafe_code, warnings)]` or
    // reformatted variants still satisfy the intent. Whitespace is
    // squeezed first so the multi-line ENG-4684 layout
    // (`#![deny(\n    unsafe_code,`) also matches.
    let squeezed = squeeze_whitespace(&src);
    assert!(
        squeezed.contains("#![deny( unsafe_code") || squeezed.contains("#![deny(unsafe_code"),
        "src/lib.rs must declare a deny(unsafe_code) inner attribute (got: {})",
        src.lines().take(15).collect::<Vec<_>>().join("\\n")
    );
    // Reject any forbid(unsafe_code) form similarly — `forbid` blocks
    // the targeted `#[allow(unsafe_code)]` we need for the mmap call.
    assert!(
        !src.contains("forbid(unsafe_code"),
        "src/lib.rs must NOT declare forbid(unsafe_code) (would block targeted allow on mmap)"
    );
}

/// ENG-4684 AC6: the deny group must name every lint the ticket
/// specifies, not just `unsafe_code`. Asserted on the whitespace-squeezed
/// text of the single `#![deny(...)]` attribute so a reflow cannot break
/// it, and so a lint moved out of `deny` into `warn` (or dropped
/// entirely) fails loudly.
#[test]
fn lib_rs_deny_group_names_the_full_lint_set() {
    let src = lib_rs();
    let squeezed = squeeze_whitespace(&src);
    let start = squeezed
        .find("#![deny(")
        .expect("src/lib.rs must declare a #![deny(...)] inner attribute");
    let end = squeezed[start..]
        .find(')')
        .map(|i| start + i)
        .expect("the #![deny(...)] attribute must be closed");
    let deny_group = &squeezed[start..end];
    for lint in [
        "unsafe_code",
        "missing_docs",
        "missing_debug_implementations",
        "rust_2018_idioms",
        "rust_2024_compatibility",
        "rustdoc::broken_intra_doc_links",
    ] {
        assert!(
            deny_group.contains(lint),
            "src/lib.rs's #![deny(...)] group must name `{lint}` — without it the \
             corresponding gate is off and nothing goes red when it is violated. \
             Group as read: {deny_group:?}"
        );
    }
}

/// ENG-4684 AC5: the library opts into both clippy groups. `pedantic`
/// is *also* set package-wide in `Cargo.toml` (see
/// `cargo_toml_warns_pedantic_package_wide`), but `nursery` is
/// deliberately library-only — the AC's clippy command carries
/// `-W clippy::pedantic` and no nursery flag, so `src/lib.rs` is the only
/// thing that makes `cargo clippy --lib -- -D warnings` a nursery gate.
#[test]
fn lib_rs_warns_pedantic_and_nursery() {
    let squeezed = squeeze_whitespace(&lib_rs());
    assert!(
        squeezed.contains("#![warn(clippy::pedantic"),
        "src/lib.rs must declare `#![warn(clippy::pedantic, ...)]` — it is what makes \
         `cargo clippy --lib --all-features -- -D warnings` a pedantic gate (AC5)."
    );
    assert!(
        squeezed.contains("clippy::nursery"),
        "src/lib.rs must warn `clippy::nursery` — nursery is deliberately NOT extended \
         to the other targets, so this attribute is the only place it is enabled."
    );
}

/// ENG-4684: no blanket escape hatch. A `#![allow(clippy::pedantic)]`,
/// `#![allow(clippy::nursery)]` or `#![allow(warnings)]` anywhere in
/// `src/lib.rs` would silently undo the two attributes above while
/// leaving them visible in the file.
#[test]
fn lib_rs_has_no_blanket_allow() {
    let squeezed = squeeze_whitespace(&lib_rs());
    for banned in [
        "allow(clippy::pedantic",
        "allow(clippy::nursery",
        "allow(warnings",
    ] {
        assert!(
            !squeezed.contains(banned),
            "src/lib.rs must not contain `{banned})` — a blanket allow re-disables the \
             whole group the ticket just enabled. Allows must name individual lints."
        );
    }
}

/// ENG-4684: `#![...]` attributes in `src/lib.rs` apply to the library
/// crate ONLY. `build.rs`, every `tests/*.rs`, `benches/route.rs` and
/// `src/bin/rustyroute.rs` are separate crates, and the ~120 pedantic
/// findings they carry were fixed or justified-allowed under this ticket.
/// Without the `Cargo.toml [lints.clippy]` table the only thing keeping
/// them clean is the `-W clippy::pedantic` flag on `ci.yaml:47`, which
/// `.pre-commit-config.yaml`'s clippy hook does not pass — so a
/// contributor's local run would not see a regression.
#[test]
fn cargo_toml_warns_pedantic_package_wide() {
    let toml = cargo_toml();
    let squeezed = squeeze_whitespace(&toml);
    assert!(
        squeezed.contains("[lints.clippy]"),
        "Cargo.toml must declare a `[lints.clippy]` table so the pedantic baseline \
         applies to every target in the package, not just the library."
    );
    let start = squeezed
        .find("[lints.clippy]")
        .expect("checked immediately above");
    let table = &squeezed[start..];
    assert!(
        table.contains("pedantic"),
        "Cargo.toml's `[lints.clippy]` table must set `pedantic` — that is the whole \
         reason the table exists (ENG-4684). Table as read: {table:?}"
    );
    // `= "allow"` in either the table or the lib attributes would be a
    // silent un-gating; only `warn` or `deny` are acceptable levels here.
    let pedantic_line = toml
        .lines()
        .find(|l| l.trim_start().starts_with("pedantic"))
        .expect("Cargo.toml must carry a `pedantic = ...` entry under [lints.clippy]");
    assert!(
        pedantic_line.contains("\"warn\"") || pedantic_line.contains("\"deny\""),
        "Cargo.toml's `pedantic` lint level must be `warn` or `deny`, not allow/forbid — \
         CI escalates warnings with `-D warnings`. Offending line: {pedantic_line:?}"
    );
}
