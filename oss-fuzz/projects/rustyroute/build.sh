#!/bin/bash -eu
# ENG-4691: OSS-Fuzz build script for rustyroute. Runs inside the
# gcr.io/oss-fuzz-base/base-builder-rust image (nightly + cargo-fuzz + clang
# preinstalled). Builds every fuzz target and copies the binaries — plus the
# committed seed corpus — into $OUT for the OSS-Fuzz runners.

cd "$SRC/rustyroute"

# cargo-fuzz auto-locates the fuzz/ package from the crate root.
cargo fuzz build -O

FUZZ_TARGET_OUTPUT_DIR="fuzz/target/x86_64-unknown-linux-gnu/release"
for target in load_archive route_inputs; do
  cp "$FUZZ_TARGET_OUTPUT_DIR/$target" "$OUT/"
done

# Ship the committed seed corpus so OSS-Fuzz starts with coverage.
cp fuzz/corpus/load_archive/*.rkyv "$OUT/" 2>/dev/null || true
