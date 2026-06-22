/// Модуль сбора метрик: опрос Solana-нод и измерение RTT.
///
/// Основные функции:
/// - `probe_node(url)` — опрашивает одну ноду: запрашивает текущий slot и замеряет RTT
/// - `probe_both(cfg)` — параллельно опрашивает target и reference через `tokio::try_join!`
///
/// RTT определяется как время от отправки RPC-запроса `getSlot` до получения ответа.
/// Это не чистый сетевой RTT — включает JSON-сериализацию и обработку на стороне ноды,
/// но является достаточно точным индикатором доступности и отзывчивости.
use std::time::Instant;

use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use tracing::debug;

use crate::config::Config;
use crate::error::SentinelError;

/// Метрики одного опроса одной ноды.
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    /// Текущий слот ноды на момент опроса.
    pub slot: u64,

    /// Время ответа ноды в миллисекундах (RTT).
    pub rtt_ms: u64,

    /// URL ноды (для логирования и алертов).
    pub node_url: String,
}

/// Результат параллельного опроса двух нод за один цикл.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    /// Метрики наблюдаемой (target) ноды.
    pub target: NodeMetrics,

    /// Метрики эталонной (reference) ноды.
    pub reference: NodeMetrics,
}

/// Опрашивает одну Solana-ноду: запрашивает текущий slot и замеряет время ответа.
///
/// Создаёт новый `RpcClient` на каждый вызов — клиент легковесный и без состояния,
/// поэтому переиспользование между вызовами не даёт ощутимого выигрыша.
///
/// Возвращает `SentinelError::Rpc` если нода недоступна, ответила таймаутом
/// или вернула некорректный ответ на `getSlot`.
pub async fn probe_node(url: &str) -> Result<NodeMetrics, SentinelError> {
    let client = RpcClient::new(url.to_string());

    // Засекаем время непосредственно перед запросом
    let start = Instant::now();
    let slot = client
        .get_slot()
        .await
        .map_err(|e| SentinelError::Rpc(format!("{url}: {e}")))?;
    let rtt_ms = start.elapsed().as_millis() as u64;

    debug!(url, slot, rtt_ms, "нода опрошена");

    Ok(NodeMetrics {
        slot,
        rtt_ms,
        node_url: url.to_string(),
    })
}

/// Параллельно опрашивает обе ноды из конфигурации.
///
/// `tokio::try_join!` запускает оба `probe_node` одновременно и ждёт обоих.
/// Суммарное время ≈ max(rtt_target, rtt_reference), а не их сумма.
///
/// Если хотя бы одна нода вернёт ошибку — функция немедленно завершается
/// с этой ошибкой (вторая задача при этом отменяется).
pub async fn probe_both(cfg: &Config) -> Result<ProbeResult, SentinelError> {
    let (target, reference) = tokio::try_join!(
        probe_node(&cfg.target_rpc_url),
        probe_node(&cfg.reference_rpc_url),
    )?;

    Ok(ProbeResult { target, reference })
}
