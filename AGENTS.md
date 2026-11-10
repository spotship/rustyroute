# AGENTS.md

Read this before touching rustyroute. It only lists things that would
otherwise cost you a wrong turn — see [`README.md`](README.md) for what
the library does and [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full
local-validation command list.

## Repo shape, not estate shape

- **Default branch is `main`, not `dev`.** Most spotship repos
  branch and release off `dev`; this one doesn't have a `dev`
  branch at all. Branch from and PR against `main`.
- **`task init` is the only task.** It runs `cargo fetch` and
  `pre-commit install`. There is no `task lint` or `task test`, so don't
  reach for them out of estate habit: lint with `cargo fmt` and
  `cargo clippy`, test with `cargo test`. The full list is in
  `CONTRIBUTING.md`'s "Local validation" section.
- **No `package.json`, no semantic-release.** Releases are cut by
  `release-plz` (conventional-commit → version bump + `CHANGELOG.md` →
  crates.io publish) plus `cargo-dist` for cross-platform binaries on
  the resulting tag. Don't look for a Node-based release step.

## Build and test

- **`Cargo.lock` is gitignored on purpose** (library crate convention)
  — don't commit one if `cargo` generates it locally. CI resolves
  dependencies afresh on every run (the `audit.yaml` jobs run
  `cargo generate-lockfile` before scanning), so your local resolved
  versions need not match CI's.
- **A plain `cargo build` does not build the `rustyroute` binary.** The
  `[[bin]]` target has `required-features = ["cli"]` specifically so
  library-only consumers never compile `clap`. Pass `--features cli`
  (or `cargo install --path . --features cli`) to get the binary.
- **The `cli` feature silently turns on all five `data-{N}km`
  features**, not just the resolution you asked for on the command
  line. That's deliberate: `Graph::load`'s `OUT_DIR` fallback points at
  a build-time temp directory that no longer exists after `cargo
  install`, so an installed binary can only serve `--resolution` values
  whose static bytes were baked in — which means all five. Don't "fix"
  this by trimming the feature list in `Cargo.toml`.
- **The vendored `.gpkg` files under `vendor/eurostat-marnet/` are the
  only data source `build.rs` reads.** There is no network fetch at
  build time; a fresh clone builds fully offline. Don't add a
  download step, and don't touch those files directly — see their own
  `vendor/eurostat-marnet/README.md` for the re-vendoring procedure,
  which is checksum-locked by `tests/vendored_data.rs`.
- **`tests/downstream_consumer/` is its own Cargo package, not a
  workspace member** (path-dependency on the parent crate, used to
  smoke-test the published-crate surface). Root-level `cargo fmt
  --check` / `cargo clippy` do **not** cover it — that's why
  `.pre-commit-config.yaml` runs a second, separate
  `cargo-fmt-check-downstream` hook. If you touch that crate, lint it
  with `--manifest-path tests/downstream_consumer/Cargo.toml`
  explicitly.
- **The pre-commit git hook only runs `cargo fmt --check` plus file
  hygiene — not clippy.** Clippy is wired as a manual stage
  (`pre-commit run --hook-stage manual`) because it's too slow for
  every commit. Passing the git hook does not mean clippy is clean,
  and `cargo doc` with warnings denied is a separate CI gate on top:
  run the full `CONTRIBUTING.md` "Local validation" list before
  pushing.
- **The library must keep compiling for `wasm32-unknown-unknown`**
  under `--no-default-features` (CI's `wasm` job). `memmap2` and the
  mmap path in `Graph::load` are already gated to
  `cfg(not(target_arch = "wasm32"))` for this reason — if you add a
  new dependency or native-only code path to the non-build-script
  parts of the crate, gate it the same way or the wasm smoke build
  breaks.
- **A first `cargo build`/`cargo test` is slow** (builds bundled
  SQLite via `rusqlite` for the build script, then compiles the graph
  for every enabled resolution). That's normal, not a hang.

## Files whose exact wording is tested, not just their structure

The `ci`, `audit`, `codspeed` and `release` workflows, `deny.toml`,
`dist-workspace.toml`, `release-plz.toml`, `.pre-commit-config.yaml`
and `CHANGELOG.md` are string-asserted by `tests/*_workflow.rs`,
`tests/*_config.rs` and `tests/changelog.rs`. **Don't hand-edit any of
those files — even a comment reword or key reorder — without running
`cargo test` after.** A failure here is these tests doing their job
(catching a silent weakening of a gate), not a flaky test to work
around.

- `Cargo.toml`'s `rust-version` and `ci.yaml`'s MSRV matrix entry
  (`1.93.0`) are a matched pair maintained by hand, per the comment at
  `ci.yaml`'s `test-matrix` job — bump both together.
- `tests/changelog.rs` requires the newest `## [X.Y.Z]` heading in
  `CHANGELOG.md` to equal `Cargo.toml`'s `version`. `release-plz`
  keeps these in sync normally; if you ever bump the version by hand,
  update the changelog heading too or this test fails.
- `deny.toml`'s licence allowlist deliberately **excludes GPL-3.0 /
  AGPL-3.0 / LGPL-3.0**, even though EUPL-1.2 Article 5 technically
  permits relicensing onto them — pulling one of those in silently
  removes rustyroute's "stay EUPL-1.2, relicense at your option"
  promise to downstream. If a dependency bump drags one in, that's a
  licensing decision for the project owner, not something to resolve
  by widening the allowlist yourself.
- `openssl-sys` is denied outright (`bans.deny` in `deny.toml`) — this
  crate standardises on `rustls`. Don't add a TLS dependency that
  pulls in openssl without raising that as its own decision first.

## Commits

- The DCO GitHub App checks every commit in a PR for a sign-off, so
  commit with `git commit -s`. PR-title and `[skip-perf]`
  rules are in `CONTRIBUTING.md`.
