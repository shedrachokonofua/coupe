FROM rust:1.87.0-slim-bookworm AS builder
WORKDIR /usr/src/coupe-sentinel
COPY . .
RUN apt-get update && \
    apt-get install -y ca-certificates pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /usr/app
COPY --from=builder /usr/src/coupe-sentinel/target/release/coupe-sentinel /usr/local/bin/coupe-sentinel
CMD ["coupe-sentinel"]
