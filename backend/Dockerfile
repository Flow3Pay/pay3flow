FROM rust:1.97-bookworm AS builder
WORKDIR /app

# protoc for tonic-prost-build codegen (backend/build.rs compiles crw/proto).
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml backend/Cargo.lock backend/build.rs backend/
COPY crw/proto crw/proto
RUN mkdir -p backend/src && echo "fn main(){}" > backend/src/main.rs \
    && cargo build --release --manifest-path backend/Cargo.toml 2>/dev/null; rm -rf backend/src
COPY backend/src backend/src
COPY backend/migrations backend/migrations
RUN touch backend/src/main.rs && cargo build --release --bins --manifest-path backend/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/backend/target/release/pay3flow-backend /usr/local/bin/pay3flow-backend
RUN mkdir -p /app/data
EXPOSE 8080
CMD ["pay3flow-backend"]