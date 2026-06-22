# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run
cargo run -- <subcommand> [args]
# Examples:
cargo run -- block --slot 123456789
cargo run -- api --url https://api.mainnet-beta.solana.com

# Check (fast compile without linking)
cargo check

# Run tests
cargo test

# Run a single test
cargo test <test_name>
```

## Architecture

This is an early-stage Rust CLI tool for interacting with the Solana blockchain. It uses `clap` with the derive macro for subcommand dispatch.

**Module layout:**
- `src/main.rs` — entry point; parses `.env` via `dotenvy`, defines the top-level `Cli` struct, delegates to `commands::execute`
- `src/commands/mod.rs` — defines the `Commands` enum (subcommands) and the `execute` dispatcher; both `Block` and `Api` commands are stubs (`TODO`)
- `src/config/mod.rs` — `Config::from_env()` reads `SOLANA_RPC_URL` from the environment (defaulting to mainnet-beta)
- `src/utils/mod.rs` — empty placeholder

**Key dependencies:**
- `solana-rpc-client` / `solana-sdk` v2.0 — Solana chain interaction
- `reqwest` — HTTP requests for the `api` subcommand
- `clap` derive — CLI parsing
- `anyhow` — error propagation
- `dotenvy` — loads `.env` at startup

**Environment:** `SOLANA_RPC_URL` in `.env` controls which cluster is targeted. Defaults to `https://api.mainnet-beta.solana.com`.

When adding new subcommands, add a variant to `Commands` in `src/commands/mod.rs` and a match arm in `execute`. Use `Config::from_env()` when RPC access is needed.
