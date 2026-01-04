# syntax=docker/dockerfile:1

# ============================================
# 第一阶段：准备 cargo-chef
# ============================================
FROM rust:latest AS chef

RUN cargo install cargo-chef --locked
WORKDIR /app

# ============================================
# 第二阶段：生成依赖配方
# ============================================
FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo chef prepare --recipe-path recipe.json

# ============================================
# 第三阶段：构建依赖和应用
# ============================================
FROM chef AS builder

# 安装 musl 工具链
RUN apt-get update && apt-get install -y \
    musl-tools \
    musl-dev \
    gcc \
    && rm -rf /var/lib/apt/lists/*

# 添加 x86_64 musl target
RUN rustup target add x86_64-unknown-linux-musl

# 复制依赖配方
COPY --from=planner /app/recipe.json recipe.json

# 构建依赖（这一层会被缓存，除非 Cargo.toml 或 Cargo.lock 改变）
# 关键：cook 也要挂 /app/target cache，否则 build 看不到产物
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    CARGO_TARGET_DIR=/app/target \
    cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# 复制源代码
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
COPY config.toml.example ./config.toml.example

# 构建应用
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    CARGO_TARGET_DIR=/app/target \
    cargo build --release --target x86_64-unknown-linux-musl && \
    install -Dm755 /app/target/x86_64-unknown-linux-musl/release/cowcat-rs /app/build/cowcat-rs

# ============================================
# 第四阶段：运行时镜像
# ============================================
FROM alpine:latest

RUN apk add --no-cache ca-certificates

WORKDIR /app

COPY --from=builder /app/build/cowcat-rs /usr/local/bin/cowcat-rs
COPY config.toml.example /app/config.toml.example

# 创建非 root 用户（可选但推荐）
RUN addgroup -g 1000 appuser && \
    adduser -D -u 1000 -G appuser appuser && \
    chown -R appuser:appuser /app

USER appuser

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/cowcat-rs"]
CMD ["--config", "/app/config.toml"]
