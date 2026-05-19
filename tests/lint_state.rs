//! ENG-4679: verify the crate-level unsafe-code lint is `deny`, not
//! `forbid`. The mmap-based `Graph::load` needs a single targeted
//! `#[allow(unsafe_code)]` on the `memmap2::Mmap::map(&file)` call,
//! which `forbid` would reject.

#[test]
fn lib_rs_uses_deny_not_forbid_unsafe_code() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    // Match the inner-attribute prefix and leave the closing paren
    // open so additional lints in the same attribute group don't
    // break the check — e.g. `#![deny(unsafe_code, warnings)]` or
    // reformatted variants still satisfy the intent.
    assert!(
        src.contains("#![deny(unsafe_code"),
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
