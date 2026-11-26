---
title: GitHub Actions bump through our own PR - Plan
type: chore
date: 2026-11-13
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-plan-bootstrap
execution: code
---

# GitHub Actions bump through our own PR - Plan

## Goal Capsule

- **Objective:** `main` carries the GitHub Actions versions Renovate proposed in rustyroute#23, its CI is green, and Renovate closes #23 on its own.
- **Means:** Apply #23's diff in this ticket's PR and fix whatever that bump breaks (KTD1, KTD2).
- **Authority:** The ENG-7686 review criteria (R1 to R5) win over everything else here; where this plan and a criterion disagree, the criterion governs.
- **Execution profile:** One draft PR, `chore(deps): ENG-7686 ...`, on branch `worktree-ENG-7686`, stacked on PR #38 (ENG-7685, branch `worktree-ENG-7685`). The base is `main` once #38 merges. The lfg shipping step owns commit, push and PR creation.
- **Stop conditions:** Stop and report if a failing check turns out to need a change outside the five workflow files and their string-asserting tests, or if `cargo-deny` or `cargo-audit` stay red with #38 in the base.
- **Who finishes:** The lfg run carries it through CI. A person merges, after #38.

---

## Product Contract

### Summary

Land the GitHub Actions version bump from Renovate's rustyroute#23 (checkout v7, setup-python v7, cache v6, codecov-action v7, CodSpeed action v5, SHA-pinned throughout) through this ticket's own PR. Update the three test files that assert the old floating action refs so the PR head is green.

### Problem Frame

Renovate's #23 is `BLOCKED` with nine failing checks, so the bump cannot merge as it stands. The ticket asks whether the bump or `main` causes the failures and wants the cause fixed in a PR of our own, leaving #23 for Renovate to close once `main` carries the bump.

Two separate causes explain the nine failures. Six `test` jobs and `coverage` fail because string-level tests assert the old floating refs, and cargo stops at the first failing test binary (`audit_workflow`), so more tests may be hidden behind it. `cargo-deny` and `cargo-audit` fail on RUSTSEC-2026-0459 (`libsqlite3-sys` 0.30.1 through `rusqlite` 0.32.1), which `main`'s own `audit` runs already fail on, so the bump does not cause it.

### Requirements

- R1. The five workflow files `.github/workflows/audit.yaml`, `ci.yaml`, `codspeed.yaml`, `release-plz.yaml` and `release.yaml` carry the same action bump as rustyroute#23's diff.
- R2. rustyroute#23 and its branch `renovate/github-actions` are not pushed to, retitled or closed by hand.
- R3. The diff touches only those five files plus the fixes the failing checks require (here, the string-asserting tests).
- R4. The PR title reads `chore(deps): ENG-7686 ...`.
- R5. `cargo-deny`, `cargo-audit`, every `test` matrix job and `coverage` pass on the PR head.
- R6. After the change the tests still fail when an action's major version drifts, so the supply-chain and release gates keep their guard.

### Key Decisions

- **The bump lands through our PR, and #23 is left for Renovate to close.** Governs R1, R2. (session-settled: user-directed - chosen over pushing fixes to `renovate/github-actions` or closing #23 by hand: Renovate closes #23 itself once `main` carries the bump.)
- **The diff stays inside the five workflow files plus the fixes the failing checks require.** Governs R3. (session-settled: user-directed - chosen over bundling unrelated workflow or dependency changes: keeps this a pure deps bump.)
- **The PR title carries the ticket id.** Governs R4. (session-settled: user-directed - chosen over a title without the id or the id only in the branch name: `.github/workflows/clickup.yaml` reads `ENG-[0-9]+` from the PR title only.)
- **The failing checks are fixed, not waved through.** Governs R5. (session-settled: user-directed - chosen over merging with them red or calling them flaky: a red check blocks the merge.)

### Scope Boundaries

Non-goals, considered and not built:

- A `deny.toml` ignore for RUSTSEC-2026-0459. A real upgrade clears it (KTD4), `AGENTS.md` treats `deny.toml` as a tested policy file, and the format rule for ignores asks reviewers to refuse them without a reason.
- Reading or fixing why `release-plz` fails on `main`. Its last three runs (2a1fb98, 2f8eeff, 81ccd1e) fail with "failed to release package", it is not among #23's nine failures, and the log does not show a cause connected to action versions. See Open Questions.
- Rewording the history comments in `codspeed.yaml` and `audit.yaml` that name `action@v4` and `audit-check@v2`. They describe past behaviour and no test reads them.
- A shared test helper module for the pinned-ref check. Three call sites in three separate test crates do not justify `tests/common`.
- Any change to `Cargo.toml` or the dependency set. ENG-7685 owns those.

### Dependencies / Prerequisites

- PR #38 (ENG-7685) is in the base. It raises `rusqlite` to 0.41.1, which resolves `libsqlite3-sys` 0.39.1, fixed per the advisory (`>=0.39.0`). The worktree's gitignored `Cargo.lock` already shows these versions. Without #38 in the base, R5 cannot hold for `cargo-deny` and `cargo-audit`.

### Outstanding Questions

#### Deferred to implementation

- Which other test files break on the bump is only known from a full `--no-fail-fast` run (U3). The three named in U2 are the ones found by grep; the run decides whether there are more.

#### For the ticket owner, non-blocking

- The acceptance criterion "default-branch CI on `main` is green after the merge" can fail on `release-plz`, which is red on `main` before and independent of this change. Does the criterion mean `ci` and `audit` only? Until answered, the PR claims only `ci` and `audit`.

---

## Planning Contract

### Key Technical Decisions

- KTD1. **Apply #23's diff as written rather than re-deriving pins.** The ticket names that diff as the reference. Read it with `gh pr diff 23`; never check out or touch `renovate/github-actions`. The five workflow files are untouched by ENG-7685, so the diff applies cleanly. There is no lockfile for the Actions bump, so the ticket's "regenerate any lockfile" instruction has nothing to do here. (session-settled: user-directed - chosen over a hand-written bump with different pins: the ticket says the diff is the reference.)
- KTD2. **Update the tests to accept a SHA pin with a version comment; do not keep floating tags.** `renovate.json` extends `helpers:pinGitHubActionDigests`, so SHA pins are the repo's chosen convention and Renovate would re-pin any floating tag we kept. The tests keep guarding the version through the trailing comment (`# v2`, `# v5`, `# v0.5`). Alternative considered and rejected: leave `cargo-deny-action@v2` floating in `audit.yaml` so the test passes unchanged. That departs from #23's diff, contradicts R1, and the next Renovate run would re-open the pin.
- KTD3. **The assertion accepts either form of one action's ref.** A `uses:` line for the action passes when its ref is the expected tag (`@v2`) or a 40-hex SHA followed by `# <expected tag>`. It fails for any other tag, `@main`, or a SHA whose comment names a different version. Accepting the tag form keeps the tests passing if someone un-pins deliberately; the version contract is the same either way. Each test file carries its own few lines of checking.
- KTD4. **No `Cargo.toml` or `deny.toml` change for the advisory.** RUSTSEC-2026-0459 comes from `main`'s `rusqlite = "0.32"` resolving `libsqlite3-sys` 0.30.1. CI regenerates the lockfile on every run, so the stacked base's `rusqlite = "0.41.1"` is enough to clear it. Alternative considered and rejected: a dated `deny.toml` ignore, which silences a real, fixable advisory.
- KTD5. **`rustsec/audit-check@v2` stays floating.** #23 does not pin it, and `audit_workflow_uses_audit_check_v2` already passes on it. Do not extend the bump past #23's diff.

### Assumptions

- The nine failures on #23 reduce to the two causes found (test string assertions, RUSTSEC advisory). Unverified until a full test run and the PR's CI.
- `codecov/codecov-action` v7 still accepts the inputs in `ci.yaml` (`files`, `fail_ci_if_error`, `token`). With `fail_ci_if_error: false` a broken upload would not fail CI, so check the coverage job's upload step in the CI log rather than trusting a green check.
- The `dtolnay/rust-toolchain` SHA pins resolve toolchains as before. #23's `clippy`, `fmt`, `docs`, `features` and `wasm` jobs passed with them, which supports this.

---

## Implementation Units

### U1. Apply Renovate's bump to the five workflows

- **Goal:** The five workflow files match #23's diff.
- **Requirements:** R1, R2, R3; KTD1
- **Dependencies:** none
- **Files:** `.github/workflows/audit.yaml`, `.github/workflows/ci.yaml`, `.github/workflows/codspeed.yaml`, `.github/workflows/release-plz.yaml`, `.github/workflows/release.yaml`
- **Approach:**
  - Get the patch from `gh pr diff 23` and apply it in the worktree.
  - Compare the result with `gh pr diff 23` once more and confirm no other path changed.
  - Leave `rustsec/audit-check@v2` and every `with:` input as they are (KTD5).
- **Execution note:** Configuration-only change; the proof is U2's tests plus the PR's CI, not new unit coverage.
- **Patterns to follow:** The pin style #23 uses: `uses: <action>@<40-hex sha> # <version>`.
- **Test scenarios:** Test expectation: none -- pure workflow configuration; U2 owns the string assertions and U3 the runs.
- **Verification:** `git diff --stat` lists exactly the five files; the diff equals #23's; `clickup.yaml` and every other file are untouched.

### U2. Update the string assertions the bump invalidates

- **Goal:** The tests accept the SHA-pinned refs and still reject a drifted version (R6).
- **Requirements:** R3, R5, R6; KTD2, KTD3, KTD5
- **Dependencies:** U1
- **Files:** `tests/audit_workflow.rs`, `tests/codspeed_workflow.rs`, `tests/release_plz_config.rs`; any further file U3's full run shows failing (test files only).
- **Approach:**
  - `audit_workflow_uses_cargo_deny_action_v2` (`tests/audit_workflow.rs`): replace the `contains("EmbarkStudios/cargo-deny-action@v2")` check with the KTD3 check for tag `v2`; update the message to say SHA pin plus `# v2`.
  - `codspeed_workflow_uses_codspeed_action_v4` (`tests/codspeed_workflow.rs`): expect `CodSpeedHQ/action` at `v5` by the KTD3 check and rename the test to `..._v5`. Keep the OIDC rationale in the message but say v4 or later; the existing `id-token: write` test stays as is.
  - `release_plz_workflow_uses_action_v0_5` (`tests/release_plz_config.rs`): the KTD3 check for `MarcoIeni/release-plz-action` at `v0.5`.
  - Leave `audit_workflow_uses_audit_check_v2` and the two `taiki-e/install-action` and `codecov/codecov-action` presence checks alone; they match without a ref.
  - Header comments that list test names (the AC map in `tests/audit_workflow.rs`) keep pointing at real test names after any rename.
- **Execution note:** Run the changed test files against U1's workflows first and confirm each failure is the ref assertion before editing it; then confirm the edited assertions go red when the ref or comment is deliberately changed (temporary local edit, reverted).
- **Patterns to follow:** The string-level, no-parser style of the existing tests (the file headers explain the choice) and their failure-message style.
- **Test scenarios:**
  - `audit.yaml` with `EmbarkStudios/cargo-deny-action@<sha> # v2` passes the cargo-deny test.
  - The same test fails when the ref is `@main`, and when the SHA's comment says `# v3`.
  - The same test passes when the ref is the plain tag `@v2` (KTD3's accepted second form).
  - `codspeed.yaml` with `CodSpeedHQ/action@<sha> # v5` passes the renamed codspeed test and fails when the comment says `# v4`.
  - `release-plz.yaml` with `MarcoIeni/release-plz-action@<sha> # v0.5` passes its test and fails with `# v0.6`.
  - `audit_workflow_uses_audit_check_v2` still passes with `rustsec/audit-check@v2` unchanged.
  - Integration: the full `tests/*_workflow.rs` and `tests/release_plz_config.rs` set passes against U1's files, including `ci_workflow.rs`'s `wasm_job_toolchain_installs_wasm32_target`, which reads the `targets:` line next to the now SHA-pinned `rust-toolchain` step.
- **Verification:** Every string-asserting test passes against the bumped workflows, and each edited test still rejects a deliberately wrong version.

### U3. Prove the whole set, locally and in CI

- **Goal:** Nothing else hides behind the first failing test binary, and the advisory is confirmed cleared.
- **Requirements:** R5; KTD4
- **Dependencies:** U1, U2
- **Files:** none changed here. If the run exposes another failing test file, the fix goes in U2's file list.
- **Approach:**
  - Run `cargo test --all-features --no-fail-fast` so every test binary reports, not only the first failing one. The first build compiles bundled SQLite and is slow (`AGENTS.md`).
  - Run `cargo fmt --all --check` and `cargo clippy --all-targets --all-features -- -D warnings`; the pre-commit hook covers fmt only.
  - Regenerate the lockfile (`cargo generate-lockfile`, gitignored) and confirm `libsqlite3-sys` resolves at 0.39.0 or later. `cargo-deny` and `cargo-audit` are not installed here, so CI is the authority for those two.
  - After the push, read the PR's check results for the nine originally failing jobs and the coverage job's codecov upload step.
- **Execution note:** The local run is the gate before commit; CI results come back through lfg's later steps.
- **Test scenarios:** Test expectation: none -- verification only; no new tests.
- **Verification:** The local full run is green; the lockfile shows `libsqlite3-sys` >= 0.39.0; on the PR head `cargo-deny`, `cargo-audit`, all six `test` jobs and `coverage` pass.

---

## Risks & Dependencies

- **Stack ordering.** The `cargo-deny` and `cargo-audit` checks pass only because #38 is in the base. If #38 is closed or reworked without the `rusqlite` bump, R5 fails and this PR needs its own advisory fix (stop condition).
- **Silent codecov regression.** `fail_ci_if_error: false` means a v4-to-v7 behaviour change in the upload would leave CI green with no coverage data. Read the upload step's log once on the PR.
- **More hidden test breakage.** The count of broken tests is a lower bound until U3's `--no-fail-fast` run; the plan sizes U2 for three files and allows more test files, not workflow or source changes.
- **Release automation.** `release-plz.yaml` changes in the bump (checkout v7, release-plz-action SHA) and the workflow is already red on `main`, so a green or red result there says nothing about the bump. It only runs on push to `main`, so the first signal comes after merge.

## Verification Contract

| What | Command or check | Applies to |
|---|---|---|
| String-asserting tests and the rest of the suite | `cargo test --all-features --no-fail-fast` | U2, U3 |
| Format | `cargo fmt --all --check` | U2 |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | U2 |
| Commit gate | pre-commit hooks run on every commit (never skipped) | all |
| Advisory cleared | `cargo generate-lockfile`, then `libsqlite3-sys` >= 0.39.0 in `Cargo.lock` | U3 |
| Diff boundary | `git diff --stat origin/worktree-ENG-7685...HEAD` lists the five workflows, the test files, and this plan | U1, U2 |
| CI | The PR's `cargo-deny`, `cargo-audit`, six `test`, and `coverage` checks pass | U3 |

## Definition of Done

- The five workflow files equal #23's diff and nothing else in `.github/` changed.
- Every edited test passes against the bumped files and fails on a wrong version.
- The full local test run is green.
- The PR is a draft titled `chore(deps): ENG-7686 ...`, #23 untouched.
- On the PR head, `cargo-deny`, `cargo-audit`, the six `test` jobs and `coverage` pass, or the report names exactly which did not and why.
- The `release-plz` question in Outstanding Questions appears in the PR or the closing report.
- No abandoned-attempt code remains in the diff.

## Sources / Research

- Renovate's diff and the nine failing checks: `gh pr diff 23`, `gh pr checks 23` (rustyroute#23).
- `test` failure: `tests/audit_workflow.rs:170-180` (`audit_workflow_uses_cargo_deny_action_v2`); other floating-ref assertions at `tests/codspeed_workflow.rs:46-52` and `tests/release_plz_config.rs:137-142`.
- Advisory: RUSTSEC-2026-0459 in the `cargo-deny` job log on #23; `main`'s `audit` runs on 81ccd1e fail the same way.
- Pin convention: `renovate.json` (`helpers:pinGitHubActionDigests`).
- Tested files and the gitignored lockfile: `AGENTS.md`, "Files whose exact wording is tested" and "Build and test".
- Stack parent: PR #38, `Cargo.toml` diff (`rusqlite` 0.32 to 0.41.1).
