FROM rust:1.82-slim-bullseye AS builder
WORKDIR /usr/src/app
RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  && rm -rf /var/lib/apt/lists/*
COPY ./sentinel sentinel
COPY ./templates/packages/rust packages
RUN sed -i 's|../templates/packages/rust|../packages|g' sentinel/Cargo.toml
WORKDIR /usr/src/app/sentinel
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y \
  openssl \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/sentinel/target/release/sentinel /usr/local/bin/sentinel
