# Build stage
FROM rust:1.82-slim-bullseye AS builder
WORKDIR /usr/src/app
RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y \
  openssl \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/consumer-function-waker /usr/local/bin/app

CMD ["app"]