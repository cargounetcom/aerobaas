# AeroBaaS server image (single binary). Built by GitHub Actions; see .github/workflows/ci.yml.
FROM rust:1-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release --locked --bin aero

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 aero
COPY --from=builder /app/target/release/aero /usr/local/bin/aero
USER aero
ENV RUST_LOG=info AERO_SERVER_HOST=0.0.0.0 AERO_SERVER_PORT=8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 CMD curl -fsS http://127.0.0.1:8080/health || exit 1
CMD ["aero", "serve"]
