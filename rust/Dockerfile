FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY sql ./sql
COPY web ./web
COPY schema.json states.json events.json ./
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir /data && chown 10001:10001 /data
COPY --from=build /build/target/release/retosean /usr/local/bin/retosean
USER 10001:10001
WORKDIR /data
ENV RUST_BIND=0.0.0.0:8080 RUST_MEDIA_DIR=/data/media RUST_LOG=retosean=info
EXPOSE 8080
ENTRYPOINT ["retosean"]
CMD ["serve"]
