FROM rust:latest AS builder
ARG TARGETARCH

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static

RUN case "${TARGETARCH}" in \
        amd64) RUST_TARGET="x86_64-unknown-linux-musl" ;; \
        arm64) RUST_TARGET="aarch64-unknown-linux-musl" ;; \
        *) echo "Unsupported TARGETARCH: ${TARGETARCH}" && exit 1 ;; \
    esac \
    && rustup target add "${RUST_TARGET}" \
    && apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/* \
    && cargo build --release --target "${RUST_TARGET}" \
    && mkdir -p /app/build \
    && cp "/app/target/${RUST_TARGET}/release/cowcat-rs" /app/build/cowcat-rs

FROM alpine:latest

RUN apk add --no-cache ca-certificates

WORKDIR /app

COPY --from=builder /app/build/cowcat-rs /usr/local/bin/cowcat-rs
COPY config.toml.example /app/config.toml.example

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/cowcat-rs"]
CMD ["--config", "/app/config.toml"]
