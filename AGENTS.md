# AGENTS.md

## Cursor Cloud specific instructions

### Project overview

cowcat-rs is a Rust PoW (Proof-of-Work) reverse proxy shield. It gates incoming HTTP traffic with a cryptographic PoW challenge before forwarding verified requests to a configurable upstream. See `README.md` for full architecture and configuration details.

### Development commands

Standard commands are documented in `README.md` § "Development & build commands". Key ones:

- **Build:** `cargo build` (debug) or `cargo build --release`
- **Lint:** `cargo clippy` and `cargo fmt --check`
- **Test:** `cargo test` (no automated tests exist yet; the command still verifies compilation)
- **Run:** `cargo run -- --config config.toml`

### Running the application locally

1. Copy `config.toml.example` to `config.toml` (already has sensible defaults).
2. The proxy needs an upstream HTTP server at the address in `[proxy].target` (default `http://127.0.0.1:1234`). A minimal upstream for testing: `python3 -m http.server 1234`.
3. Start the proxy: `RUST_LOG=info cargo run -- --config config.toml` — it listens on `0.0.0.0:8080`.
4. To bypass the PoW gate for quick testing, set env `COWCAT_POW_DIFFICULTY=0` or use a path covered by an allow rule (e.g. `/.well-known/acme-challenge/`).

### Non-obvious notes

- `cargo fmt --check` currently reports formatting diffs in the existing code. This is a pre-existing condition, not introduced by new changes.
- The PoW challenge page solves very quickly in Chrome (difficulty=3 with WASM workers ~30-70ms), so you may not visually see the challenge page before redirect.
- Static assets in `static/assets/` are embedded at compile time via `rust-embed`. Regenerating them requires `bunx esbuild` (JS) and `wasm32-unknown-unknown` target (WASM), but pre-built assets are checked in and sufficient for normal development.
- No external services (databases, caches, queues) are required; everything is in-memory.
