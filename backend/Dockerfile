FROM rust:1.97-bookworm AS builder
WORKDIR /app
ENV PLAYWRIGHT_DRIVER_CACHE_DIR=/app/playwright-driver-cache

COPY backend/Cargo.toml backend/Cargo.lock backend/
RUN mkdir -p backend/src && echo "fn main(){}" > backend/src/main.rs \
    && cargo build --release --manifest-path backend/Cargo.toml 2>/dev/null \
    && rm -rf backend/src
COPY backend/src backend/src
COPY backend/migrations backend/migrations
COPY backend/build.rs backend/build.rs
COPY backend/providerfile_code.rs backend/providerfile_code.rs
COPY backend/providers backend/providers
RUN touch backend/src/main.rs && cargo build --release --bin pay3flow-backend --manifest-path backend/Cargo.toml \
    && driver_dir="$(find /app/playwright-driver-cache -mindepth 1 -maxdepth 1 -type d -name 'playwright-*' -print -quit)" \
    && test -n "$driver_dir" \
    && cp -a "$driver_dir" /app/playwright-driver

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates chromium \
    && rm -rf /var/lib/apt/lists/*
ENV PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium
ENV PLAYWRIGHT_DRIVER_PATH=/opt/playwright-driver
WORKDIR /app
COPY --from=builder /app/backend/target/release/pay3flow-backend /usr/local/bin/pay3flow-backend
COPY --from=builder /app/playwright-driver /opt/playwright-driver
COPY config.toml /app/config.toml
RUN mkdir -p /app/data
EXPOSE 8080
CMD ["pay3flow-backend"]
