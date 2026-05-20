# Contributing to rustyroute

Thank you for your interest in contributing! rustyroute is an open
maritime sea-routing library distributed under the EUPL-1.2 licence.
This document covers what to know before opening an issue or pull
request.

## Project status

rustyroute is currently a **repository skeleton**. Routing algorithms,
maritime graph data, build scripts, and CI workflows are intentionally
not yet present. Please do not open PRs that add routing code, vendor
graph data, or introduce CI infrastructure unless the linked issue
explicitly says that is in scope.

## Code of Conduct

This project adopts the [Contributor Covenant 2.1](CODE_OF_CONDUCT.md).
Report unacceptable behavior to `conduct@spot-ship.com`.

## Commit messages and PR titles

We use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

PR titles must follow:

```
type(scope): short imperative summary
```

or, equivalently, without a scope:

```
type: short imperative summary
```

**Allowed types:** `feat`, `fix`, `perf`, `refactor`, `style`, `test`,
`ci`, `docs`, `revert`, `build`, `chore`.

**Suggested scopes** (extend as the project grows): `crate`, `docs`,
`legal`, `community`, `deps`, `infra`, and later `routing`, `graph`,
`data`.

Use the present tense and the imperative mood ("add", not "added").

**Spot Ship internal contributors** should prefix the summary with the
ClickUp ticket id, e.g. `feat(graph): ENG-1234 implement Dijkstra`.
External community contributors are not expected to include a ticket
id; reviewers will add one when the PR is merged.

## Developer Certificate of Origin (DCO)

Every commit must include a sign-off line attesting that you have the
right to contribute the change under the project licence:

```
Signed-off-by: Your Name <you@example.com>
```

Use `git commit -s` to add this automatically. If you forget on one or
more commits, you can amend the most recent with `git commit --amend
--signoff`, or rebase to add sign-offs to all commits in a branch with:

```
git rebase --signoff origin/main
```

See <https://developercertificate.org/> for the full text of the DCO.

## Local validation

First-time setup — install [pre-commit](https://pre-commit.com) itself
(via pipx, pip, or Homebrew), then enable the git hook so format and
file-hygiene checks run on every `git commit`:

```sh
pipx install pre-commit   # or: pip install pre-commit / brew install pre-commit
pre-commit install
```

To run the managed hooks explicitly (e.g. before pushing):

```sh
pre-commit run --all-files
pre-commit run --hook-stage manual  # runs cargo clippy --no-deps -- -D warnings
```

The pre-commit gate runs `cargo fmt --check` plus lightweight file
hygiene (trailing whitespace, EOF newlines, YAML/TOML/JSON syntax,
merge-conflict markers, private keys). Clippy and the rest of the
full local validation suite remain manual. Before opening a pull
request, run:

```sh
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
cargo package --allow-dirty --list
```

If your change only touches non-Rust files (e.g. docs, GitHub config,
licence text), at minimum validate the file format:

- TOML — `python3 -c "import tomllib; tomllib.load(open('Cargo.toml','rb'))"`
- JSON — `python3 -m json.tool < renovate.json > /dev/null`
- YAML — `python3 -c "import yaml; yaml.safe_load(open('.github/dco.yml'))"`

## Pull request workflow

1. Fork the repository (or create a branch if you have write access).
2. Make your change in a focused branch.
3. Sign off every commit (`git commit -s`).
4. Run local validation (see above).
5. Open a pull request and fill in every section of the PR template.
6. Expect a Code Owner review from `@spotship/maintainers`.

## Reporting bugs and requesting features

Use the appropriate [issue form](https://github.com/spotship/rustyroute/issues/new/choose).
Please include enough detail to reproduce or to evaluate the proposal.

## Security

For suspected vulnerabilities, follow [`SECURITY.md`](SECURITY.md). Do
**not** report security issues via public issues.

## Licence

By contributing, you agree that your contributions will be licensed
under [EUPL-1.2](LICENSE). See `NOTICE` for upstream attribution.
