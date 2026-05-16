<!--
PR title MUST follow Conventional Commits:

    type(scope): short imperative summary
    type: short imperative summary

Allowed types: feat, fix, perf, refactor, style, test, ci, docs,
revert, build, chore. Use the present tense.

Spot Ship internal contributors should also prefix the summary with the
ClickUp ticket id, e.g. `feat(graph): ENG-1234 implement Dijkstra`.
External contributors are not expected to include a ticket id.
-->

## Summary

<!-- What changed and why. 1–3 short paragraphs. -->

## Verification

```sh
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
cargo package --allow-dirty --list
```

<!-- Paste the relevant output, or summarize the result. If a command
     was skipped, explain why. -->

## Checklist

- [ ] PR title follows the Conventional Commits format above.
- [ ] All commits are signed off (DCO — `git commit -s`).
- [ ] No routing code, vendored data, build script, or CI workflow is
      added unless this PR explicitly owns that scope.
- [ ] `LICENSE` and `NOTICE` attribution preserved when those files
      were touched.
- [ ] Community / config files (`*.json`, `*.yml`, `*.toml`, `*.md`)
      parse cleanly with the validators listed in `CONTRIBUTING.md`.
