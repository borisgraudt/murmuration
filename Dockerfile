# syntax=docker/dockerfile:1
#
# Multi-stage build for the `mur` mesh node.
#   docker build -t murmuration .
#   docker run --rm -p 8080:8080 -p 8000:8000 murmuration
# The published image lives at ghcr.io/borisgraudt/murmuration (see
# .github/workflows/packages.yml).

FROM rust:1.90-slim AS builder
WORKDIR /src

# System deps for the node's crypto/TLS stack.
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Workspace manifests + members (core depends on the murmuration-routing crate).
COPY Cargo.toml Cargo.lock* ./
COPY core/Cargo.toml core/Cargo.toml
COPY crates/murmuration-routing/Cargo.toml crates/murmuration-routing/Cargo.toml
COPY core/src core/src
COPY core/benches core/benches
COPY crates/murmuration-routing/src crates/murmuration-routing/src
COPY crates/murmuration-routing/README.md crates/murmuration-routing/README.md
COPY crates/murmuration-routing/LICENSE crates/murmuration-routing/LICENSE

RUN cargo build -p murmuration --release --bin mur --bin core

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /src/target/release/mur /usr/local/bin/mur
COPY --from=builder /src/target/release/core /usr/local/bin/core

ENV RUST_LOG=info
# P2P (tcp) + gateway (tcp) + discovery (udp).
EXPOSE 8080/tcp 8000/tcp 9998/udp

ENTRYPOINT ["mur"]
CMD ["start", "8080", "--gateway", "8000"]
