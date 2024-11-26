# Needs to be built from the root of the repository

# Source stage: Copy the sentinel source code and the rust packages
FROM debian:bullseye-slim AS source
WORKDIR /usr/src/app
COPY ./platform/sentinel sentinel
COPY ./templates/packages/rust packages

# Build stage: Build the sentinel binary
FROM rust:1.82-slim-bullseye AS builder
WORKDIR /usr/src/app
RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  sed \
  && rm -rf /var/lib/apt/lists/*
COPY --from=source /usr/src/app/sentinel sentinel
COPY --from=source /usr/src/app/packages packages
WORKDIR /usr/src/app/sentinel
RUN sed -i 's|../../templates/packages/rust|../packages|g' Cargo.toml
RUN cargo build --release

# Runtime stage: Copy the sentinel binary and the required libraries
FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y \
  openssl \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/sentinel/target/release/sentinel /usr/local/bin/sentinel

