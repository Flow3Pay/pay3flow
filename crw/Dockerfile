# syntax=docker/dockerfile:1

# ---- Build stage -----------------------------------------------------------
FROM rust:1.97-bookworm AS builder
WORKDIR /build

# protoc for the tonic-prost-build codegen in crw-server/build.rs; build-essential
# + cmake + perl for aws-lc-sys (rustls/aws-lc-rs), which compiles C code at build time.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        protobuf-compiler \
        build-essential \
        cmake \
        perl \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY proto ./proto

RUN cargo build --release -p crw-server

# ---- Runtime stage ---------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r crw && useradd -r -g crw -d /app crw

WORKDIR /app

COPY --from=builder /build/target/release/crw-server /usr/local/bin/crw-server
COPY config.toml /app/config.toml

ENV RUST_LOG=info
ENV CRW_CONFIG=/app/config.toml

USER crw

EXPOSE 3030 3031

HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD wget -q --spider http://localhost:3030/health || exit 1

CMD ["crw-server"]