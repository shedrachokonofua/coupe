# Build stage
FROM rust:1.77-slim-buster AS builder

WORKDIR /usr/src/app

# Copy the entire workspace
COPY . .

# Build dependencies
RUN cargo build --release --workspace

# Build for release
RUN cargo build --release --bin trigger

# Runtime stage
FROM debian:buster-slim

RUN apt-get update && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

# Copy the built binary from the builder stage
COPY --from=builder /usr/src/app/target/release/trigger .

# Set the startup command
CMD ["./trigger"]