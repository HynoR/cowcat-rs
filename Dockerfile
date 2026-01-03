FROM rust:latest AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static

RUN rustup target add x86_64-unknown-linux-musl \
    && apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/* \
    && cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:latest

RUN apk add --no-cache ca-certificates

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/cowcat-rs /usr/local/bin/cowcat-rs
COPY config.toml.example /app/config.toml.example

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/cowcat-rs"]
CMD ["--config", "/app/config.toml"]
