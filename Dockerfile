# syntax=docker/dockerfile:1

FROM rust:1.85-slim AS builder
WORKDIR /src

COPY Cargo.toml Cargo.toml
COPY core/Cargo.toml core/Cargo.toml
COPY core/src core/src

RUN cargo build -p meshlink_core --release --bin ely --bin core

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /src/target/release/ely /usr/local/bin/ely
COPY --from=builder /src/target/release/core /usr/local/bin/core

ENV RUST_LOG=info
EXPOSE 8080/tcp 9998/udp

ENTRYPOINT ["ely"]
CMD ["start", "8080"]



