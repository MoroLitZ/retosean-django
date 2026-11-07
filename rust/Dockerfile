# OPS-4: base images pinned by digest; refresh the digests deliberately.
FROM rust:1-bookworm@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY sql ./sql
COPY web ./web
COPY schema.json states.json states_meta.json events.json ./
RUN cargo build --locked --release

FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
# curl is needed by the healthcheck (OPS-5).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir /data && chown 10001:10001 /data
COPY --from=build /build/target/release/retosean /usr/local/bin/retosean
USER 10001:10001
WORKDIR /data
ENV RUST_BIND=0.0.0.0:8080 RUST_MEDIA_DIR=/data/media RUST_LOG=retosean=info
EXPOSE 8080
# OPS-5: /health only answers 200 when the database responds (src/lib.rs).
HEALTHCHECK --interval=30s --timeout=5s --retries=3 --start-period=10s \
    CMD curl -fsS http://127.0.0.1:8080/health
ENTRYPOINT ["retosean"]
CMD ["serve"]
