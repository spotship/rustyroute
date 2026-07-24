FROM gcr.io/oss-fuzz-base/base-builder-rust
RUN git clone --depth 1 https://github.com/spotship/rustyroute rustyroute
WORKDIR rustyroute
COPY build.sh $SRC/
