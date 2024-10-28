# Build stage
FROM rust:1.82-slim-bullseye AS builder

WORKDIR /usr/src/app

# Copy the entire workspace
COPY . .

# Build dependencies
RUN cargo build --release --workspace

# Build for release
RUN cargo build --release --bin trigger

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
  openssl \
  ca-certificates \
  poppler-utils \
  && rm -rf /var/lib/apt/lists/*


WORKDIR /usr/local/bin

# Copy the built binary from the builder stage
COPY --from=builder /usr/src/app/target/release/trigger .

# Expose the port
EXPOSE 80

# Set the startup command
CMD ["./trigger"]