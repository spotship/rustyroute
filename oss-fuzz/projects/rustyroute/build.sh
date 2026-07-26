#!/bin/bash -eu
# ENG-4691: OSS-Fuzz build script for rustyroute. Runs inside the
# gcr.io/oss-fuzz-base/base-builder-rust image (nightly + cargo-fuzz + clang
# preinstalled). Builds every fuzz target and copies the binaries — plus the
# committed seed corpus — into $OUT for the OSS-Fuzz runners.

cd "$SRC/rustyroute"

# Pin the target triple rather than relying on cargo-fuzz's default. That
# default is the *host* triple, which is not reliably gnu — it resolved to musl
# on GitHub's runners, and ASan (project.yaml `sanitizers: address`) is
# incompatible with a statically linked libc. See the same pin in
# .github/workflows/fuzz.yaml. Deriving the output dir from the same variable
# keeps the build and the copy below from ever disagreeing.
FUZZ_TARGET_TRIPLE="x86_64-unknown-linux-gnu"

# cargo-fuzz auto-locates the fuzz/ package from the crate root.
cargo fuzz build -O --target "$FUZZ_TARGET_TRIPLE"

FUZZ_TARGET_OUTPUT_DIR="fuzz/target/$FUZZ_TARGET_TRIPLE/release"
for target in load_archive route_inputs; do
  cp "$FUZZ_TARGET_OUTPUT_DIR/$target" "$OUT/"
done

# Ship the committed seed corpus so OSS-Fuzz starts with coverage. OSS-Fuzz
# only ingests seeds from $OUT/<target>_seed_corpus.zip (loose files in $OUT
# are ignored), so package the load_archive seed(s) into that zip. Only
# load_archive has a committed seed; route_inputs relies on coverage-guided
# discovery.
if compgen -G "fuzz/corpus/load_archive/*" > /dev/null; then
  zip -j "$OUT/load_archive_seed_corpus.zip" fuzz/corpus/load_archive/*
fi
