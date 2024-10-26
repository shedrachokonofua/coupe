# Build stage
FROM rust:1.70-slim-buster AS builder

WORKDIR /usr/src/app

COPY . .

RUN cargo build --release

# Runtime stage
FROM debian:buster-slim

COPY --from=builder /usr/src/app/target/release/consumer-function-waker /usr/local/bin/app

CMD ["app"]