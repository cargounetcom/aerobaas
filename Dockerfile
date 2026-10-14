FROM rust:1.82-bookworm AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock* ./
COPY crates/ crates/
COPY erlang/ erlang/

RUN cargo build --release --bin aero

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/aero /app/aero
COPY --from=builder /app/erlang /app/erlang

ENV AERO_ENV=production
ENV RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/aero", "serve", "--host", "0.0.0.0", "--port", "8080"]
