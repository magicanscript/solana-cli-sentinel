/// Модуль сбора метрик (заглушка для Фазы 1).
///
/// Содержит только структуры данных — они нужны модулю `analysis` уже сейчас.
/// Реальная логика опроса нод (probe_node, probe_both) будет реализована в Фазе 2.

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
