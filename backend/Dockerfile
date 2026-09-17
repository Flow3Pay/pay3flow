FROM rust:1.97-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release 2>/dev/null; rm -rf src
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release --bins

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/pay3flow-backend /usr/local/bin/pay3flow-backend
COPY --from=builder /app/target/release/load_acquirers /usr/local/bin/load_acquirers
COPY --from=builder /app/target/release/fake_solvers /usr/local/bin/fake_solvers
RUN mkdir -p /app/data
EXPOSE 8080
CMD ["pay3flow-backend"]