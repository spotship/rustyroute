#![no_main]
//! ENG-4691: fuzz `rustyroute::Graph::from_bytes` on arbitrary bytes.
//!
//! Contract: feeding any byte slice to `from_bytes` must only ever return a
//! typed `LoadError` (or `Ok`) — never panic, OOM, or segfault. `from_bytes`
//! already guards its header length before slicing (`src/loader.rs:533`) and
//! runs rkyv's *checked* `access`, so this target proves that contract holds
//! across the whole input space and guards against regressions.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // `Graph::from_bytes` takes `&'static [u8]`. Promote a copy of the
    // transient fuzz buffer to `'static`, then reclaim it after use so RSS
    // stays flat across libFuzzer's many in-process iterations. A plain
    // `Box::leak` would accumulate one copy per iteration and — with the
    // large committed seed driving `-max_len` up — climb toward
    // `-rss_limit_mb` and trip a false-positive OOM crash.
    let ptr = Box::into_raw(data.to_vec().into_boxed_slice());

    // Inner scope confines the `&'static [u8]` borrow (and the `Graph` that
    // holds it) so both are definitely dead before the free below — no shared
    // reference derived from `ptr` is live across `Box::from_raw`.
    {
        // SAFETY: `ptr` is a freshly created boxed slice we have not freed, so
        // dereferencing it to a shared slice is valid.
        let leaked: &'static [u8] = unsafe { &*ptr };
        let _ = rustyroute::Graph::from_bytes(leaked);
    }

    // SAFETY: a `Graph` holds only `GraphBacking::Static(&'static [u8])`
    // (src/loader.rs:167) — a borrow, not an owner — and both it and `leaked`
    // went out of scope above, so no reference into `ptr` survives.
    // Reconstructing the `Box` to free it is sound.
    drop(unsafe { Box::from_raw(ptr) });
});
