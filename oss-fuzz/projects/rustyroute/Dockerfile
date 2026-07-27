FROM gcr.io/oss-fuzz-base/base-builder-rust
# Absolute $SRC paths: build.sh does `cd "$SRC/rustyroute"`, so spell the same
# location here instead of leaning on the base image's default working dir
# being $SRC.
RUN git clone --depth 1 https://github.com/spotship/rustyroute $SRC/rustyroute
WORKDIR $SRC/rustyroute
COPY build.sh $SRC/
