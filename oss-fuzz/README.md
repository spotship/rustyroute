# OSS-Fuzz submission for rustyroute (ENG-4691)

This directory stages the files that OSS-Fuzz needs for continuous fuzzing of
`rustyroute`. They live here so they are versioned alongside the fuzz targets
they build, but the **actual submission is a pull request to the external
[`google/oss-fuzz`](https://github.com/google/oss-fuzz) repository** — it
cannot be merged from this repo.

```
oss-fuzz/projects/rustyroute/
  project.yaml   # engine/sanitizer config + maintainer contacts
  Dockerfile     # clones this repo into the OSS-Fuzz base-builder-rust image
  build.sh       # cargo fuzz build -O; copies target binaries + seed to $OUT
```

## Prerequisites

- `github.com/spotship/rustyroute` must be **public** (OSS-Fuzz only fuzzes
  public projects). The Dockerfile clones over HTTPS.
- OSS-Fuzz requires **two maintainer email addresses** associated with the
  project. Satisfied: `project.yaml` `auto_ccs` lists `jimbo@spot-ship.com`
  and `jimbo@freedman.io`.

## Submission steps

Steps 1–2 are already done; the submission itself (steps 3–6) is a deliberate
human follow-up and has **not** been started.

1. ~~Confirm the co-maintainer email and add it to `auto_ccs`.~~ Done — both
   maintainer addresses are in `project.yaml`.
2. ~~Verify the repo is public.~~ Done — `spotship/rustyroute` is public.
3. Fork `google/oss-fuzz`. Copy this `projects/rustyroute/` directory to
   `projects/rustyroute/` in the fork (drop the `oss-fuzz/` prefix — in
   google/oss-fuzz the path is `projects/rustyroute/`).
4. Validate locally against the OSS-Fuzz tooling:
   ```sh
   python infra/helper.py build_image rustyroute
   python infra/helper.py build_fuzzers rustyroute
   python infra/helper.py check_build rustyroute
   ```
5. Open the pull request to `google/oss-fuzz`. Approval typically takes
   **1–3 weeks**; an OSS-Fuzz maintainer must merge it.
6. Once the PR is open, paste its link into ClickUp ticket **ENG-4691** as the
   tracking link.

## Notes

- The CI quick-pass (`.github/workflows/fuzz.yaml`) runs `load_archive` for 60s
  per PR. OSS-Fuzz runs the deep, continuous variant of both targets.
- `build.sh` builds both `load_archive` and `route_inputs`; keep the target
  list in sync with `fuzz/Cargo.toml`.
- The committed seed is delivered to OSS-Fuzz as
  `$OUT/load_archive_seed_corpus.zip` — OSS-Fuzz only ingests seed corpora
  from `<target>_seed_corpus.zip`, not from loose files copied into `$OUT`.
  `base-builder-rust` provides `zip`.
