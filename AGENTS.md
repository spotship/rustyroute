# AGENTS.md

Read this before touching rustyroute. It only lists things that would
otherwise cost you a wrong turn — see [`README.md`](README.md) for what
the library does and [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full
local-validation command list.

## Repo shape, not estate shape

- **Default branch is `main`, not `dev`.** Every other spotship repo
  branches and releases off `dev`; this one doesn't have a `dev`
  branch at all. Branch from and PR against `main`.
- **There is no `Taskfile.yml`.** Don't run `task init` / `task lint` /
  `task test` out of estate habit — they don't exist here. Use plain
  `cargo` commands; the full list is in `CONTRIBUTING.md`'s "Local
  validation" section.
- **No `package.json`, no semantic-release.** Releases are cut by
  `release-plz` (conventional-commit → version bump + `CHANGELOG.md` →
  crates.io publish) plus `cargo-dist` for cross-platform binaries on
  the resulting tag. Don't look for a Node-based release step.
- **This is the only Rust repo in the estate** — Ruff/ESLint/pytest
  conventions from other repos don't apply; lint/format/test here mean
  `cargo clippy` / `cargo fmt` / `cargo test`.
- **`CONTRIBUTING.md`'s "Project status" section (the "repository
  skeleton" paragraph) is stale.** Routing code, vendored graph data,
  `build.rs`, and every CI workflow described below already exist and
  are enforced. Don't take that paragraph as license to skip adding
  routing code, and don't report CI/data as "not yet present."

## Build and test

- **`Cargo.lock` is gitignored on purpose** (library crate convention)
  — don't commit one if `cargo` generates it locally, and don't expect
  your local resolved versions to match what CI resolved on its last
  run; CI runs `cargo generate-lockfile` fresh in each job.
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
  every commit. Passing the git hook does not mean clippy is clean;
  run `cargo clippy --all-targets --all-features -- -D warnings`
  yourself before pushing (CI runs it with `-D warnings` and will
  fail where a clean local `cargo test` would not have warned you).
- **`cargo doc --no-deps --all-features` is also a hard CI gate**
  (`RUSTDOCFLAGS="-D warnings"`), separate from clippy. A doc-comment
  that produces an intra-doc-link warning fails CI even if the code
  itself is warning-free.
- **The library must keep compiling for `wasm32-unknown-unknown`**
  under `--no-default-features` (CI's `wasm` job). `memmap2` and the
  mmap path in `Graph::load` are already gated to
  `cfg(not(target_arch = "wasm32"))` for this reason — if you add a
  new dependency or native-only code path to the non-build-script
  parts of the crate, gate it the same way or the wasm smoke build
  breaks.
- **A first `cargo build`/`cargo test` takes 60–120s** (builds bundled
  SQLite via `rusqlite` for the build script, then compiles the graph
  for every enabled resolution). That's normal, not a hang.

## Files whose exact wording is tested, not just their structure

`tests/ci_workflow.rs`, `tests/audit_workflow.rs`,
`tests/codspeed_workflow.rs`, `tests/release_dist_workflow.rs`,
`tests/release_plz_config.rs`, `tests/pre_commit_config.rs`, and
`tests/changelog.rs` assert on specific strings inside, respectively,
`.github/workflows/{ci,audit,codspeed,release}.yaml`, `deny.toml`,
`dist-workspace.toml`, `release-plz.toml`,
`.pre-commit-config.yaml`, and `CHANGELOG.md`. **Don't hand-edit any of
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

## Commit and release conventions

- Commits must be signed off (`git commit -s`) — `.github/dco.yml`
  enforces the DCO on every commit, not just the PR as a whole.
- PR titles follow `type(scope): summary`; Spot Ship-internal
  contributors put the ClickUp ticket id **inside that summary**
  (`feat(graph): ENG-1234 implement Dijkstra`), not as a trailing
  footer the way some other conventions do it.
- A perf regression that's an intentional trade-off goes through as
  `[skip-perf]` in the PR title plus a `CHANGELOG.md` note — don't
  "fix" a CodSpeed regression by weakening `benches/route.rs`.
