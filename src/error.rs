/// Central error type for the entire application.
///
/// Uses `thiserror` which auto-generates `Display` and `std::error::Error`
/// implementations via the `#[derive(Error)]` attribute.
///
/// Each variant maps to one error source:
/// - `Config`   — errors reading configuration from env variables
/// - `Rpc`      — errors talking to the Solana RPC (network, timeout, bad response)
/// - `Http`     — HTTP-level errors (reqwest) for Mistral API and Telegram
/// - `Llm`      — unexpected LLM response structure (missing JSON fields)
/// - `Telegram` — Telegram Bot API returned `ok: false`
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SentinelError {
    /// A required env variable is missing or has an invalid format.
    /// `{0}` contains a human-readable description of what is wrong.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Error querying a Solana RPC node.
    /// `{0}` contains the client message (address, error code, etc.).
    #[error("RPC error: {0}")]
    Rpc(String),

    /// HTTP-level error: network failures, timeouts, TLS issues.
    /// `#[from]` lets Rust auto-convert `reqwest::Error` into `SentinelError::Http` via `?`.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// The LLM responded but the JSON structure was unexpected
    /// (e.g. the `choices[0].message.content` field is absent).
    #[error("Unexpected LLM response: {0}")]
    Llm(String),

    /// Telegram API returned `ok: false` with an error description.
    #[error("Telegram error: {0}")]
    Telegram(String),
}
