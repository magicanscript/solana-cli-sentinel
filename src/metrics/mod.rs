/// Metrics collection module: probing Solana nodes and measuring RTT.
///
/// Main functions:
/// - `probe_node(url)` — probes a single node: fetches the current slot and measures RTT
/// - `probe_both(cfg)` — probes target and reference in parallel via `tokio::try_join!`
///
/// RTT is measured as the time from sending the `getSlot` RPC request to receiving
/// the response. This is not pure network RTT — it includes JSON serialisation and
/// server-side processing — but it is a reliable indicator of node availability and
/// responsiveness.
use std::time::Instant;

use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use tracing::debug;

use crate::config::Config;
use crate::error::SentinelError;
use crate::utils;

/// Metrics for a single probe of a single node.
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    /// Current slot of the node at the time of the probe.
    pub slot: u64,

    /// Node response time in milliseconds (RTT).
    pub rtt_ms: u64,

    /// Node URL (used for logging and alerts).
    pub node_url: String,
}

/// Result of a parallel probe of both nodes in one cycle.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    /// Metrics for the monitored (target) node.
    pub target: NodeMetrics,

    /// Metrics for the reference node.
    pub reference: NodeMetrics,
}

/// Probes a single Solana node: fetches the current slot and measures response time.
///
/// Creates a new `RpcClient` on each call — the client is lightweight and stateless,
/// so reusing it between calls provides no meaningful benefit.
///
/// Returns `SentinelError::Rpc` if the node is unreachable, times out,
/// or returns an invalid response to `getSlot`.
pub async fn probe_node(url: &str) -> Result<NodeMetrics, SentinelError> {
    let client = RpcClient::new(url.to_string());

    // Start the timer immediately before the request
    let start = Instant::now();
    let slot = client
        .get_slot()
        .await
        .map_err(|e| SentinelError::Rpc(format!("{url}: {e}")))?;
    let rtt_ms = start.elapsed().as_millis() as u64;

    debug!(url, slot, rtt_ms, "node probed");

    Ok(NodeMetrics {
        slot,
        rtt_ms,
        node_url: url.to_string(),
    })
}

/// Probes both nodes from the configuration in parallel with automatic retry.
///
/// Each node is probed independently with exponential backoff (up to 3 attempts).
/// `tokio::try_join!` runs both retry loops concurrently — total wall-clock time
/// ≈ max(rtt_target, rtt_reference), not their sum.
pub async fn probe_both(cfg: &Config) -> Result<ProbeResult, SentinelError> {
    let target_url = cfg.target_rpc_url.clone();
    let reference_url = cfg.reference_rpc_url.clone();

    let (target, reference) = tokio::try_join!(
        utils::retry_async("target rpc", 3, || probe_node(&target_url)),
        utils::retry_async("reference rpc", 3, || probe_node(&reference_url)),
    )?;

    Ok(ProbeResult { target, reference })
}
