/// Модуль анализа метрик.
///
/// Содержит единственную публичную функцию `analyze()`, которая принимает
/// результат опроса двух нод и конфигурацию, и возвращает структурированный
/// анализ с флагами нарушений.
///
/// Этот модуль намеренно не делает никаких сетевых запросов — только математика
/// и сравнения. Благодаря этому его легко тестировать без реального Solana-узла.
use crate::config::Config;
use crate::metrics::ProbeResult;

/// Результат анализа одного цикла опроса.
///
/// Содержит как сырые вычисленные значения (для логирования),
/// так и булевы флаги (для принятия решения об алерте).
#[derive(Debug, Clone)]
pub struct Analysis {
    /// Разница слотов: `target_slot - reference_slot`.
    /// Отрицательное значение означает, что target-нода отстаёт от эталона.
    /// Например, -12 значит: наша нода позади эталонной на 12 слотов.
    pub slot_delta: i64,

    /// RTT запроса к target-ноде в миллисекундах.
    pub target_rtt_ms: u64,

    /// RTT запроса к reference-ноде в миллисекундах (для контекста).
    pub reference_rtt_ms: u64,

    /// `true` если отставание target-ноды превышает порог `slot_lag_threshold`.
    /// Условие: `slot_delta < -(config.slot_lag_threshold as i64)`
    pub is_slot_lagging: bool,

    /// `true` если RTT target-ноды превышает порог `rtt_threshold_ms`.
    pub is_rtt_high: bool,

    /// `true` если нужно отправить алерт (хотя бы одно условие нарушено).
    /// `needs_alert = is_slot_lagging || is_rtt_high`
    pub needs_alert: bool,
}

impl Analysis {
    /// Возвращает человекочитаемое описание проблемы для логирования.
    /// Если проблем нет — возвращает "OK".
    pub fn status_text(&self) -> String {
        if !self.needs_alert {
            return "OK".to_string();
        }
        let mut parts = Vec::new();
        if self.is_slot_lagging {
            parts.push(format!("отставание слотов: {} (порог: нарушен)", self.slot_delta));
        }
        if self.is_rtt_high {
            parts.push(format!("высокий RTT: {}ms", self.target_rtt_ms));
        }
        parts.join(", ")
    }
}

/// Анализирует результат опроса нод и возвращает структуру `Analysis`.
///
/// # Аргументы
/// * `probe` — результат параллельного опроса target и reference нод
/// * `cfg`   — конфигурация с порогами для сравнения
///
/// # Логика
/// - `slot_delta` = target_slot - reference_slot (может быть отрицательным)
/// - Нода отстаёт если она за эталоном больше чем на `slot_lag_threshold` слотов
/// - RTT высокий если он превышает `rtt_threshold_ms`
pub fn analyze(probe: &ProbeResult, cfg: &Config) -> Analysis {
    // Вычисляем дельту слотов.
    // Приводим u64 к i64 чтобы дельта могла быть отрицательной.
    let slot_delta = probe.target.slot as i64 - probe.reference.slot as i64;

    // Нода отстаёт если дельта отрицательна И по модулю превышает порог.
    // Пример: порог=5, дельта=-7 → отстаёт. Дельта=-3 → в норме.
    let is_slot_lagging = slot_delta < -(cfg.slot_lag_threshold as i64);

    // RTT высокий если превышает настроенный порог в миллисекундах.
    let is_rtt_high = probe.target.rtt_ms > cfg.rtt_threshold_ms;

    Analysis {
        slot_delta,
        target_rtt_ms: probe.target.rtt_ms,
        reference_rtt_ms: probe.reference.rtt_ms,
        is_slot_lagging,
        is_rtt_high,
        needs_alert: is_slot_lagging || is_rtt_high,
    }
}

// ============================================================================
// Unit-тесты
// ============================================================================
//
// Запуск: cargo test
// Запуск конкретного теста: cargo test analysis::tests::test_no_alert

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::NodeMetrics;
    use std::time::Duration;

    /// Создаёт тестовый Config с заданными порогами.
    /// Поля API-ключей заполнены заглушками — в тестах они не используются.
    fn make_config(slot_lag_threshold: u64, rtt_threshold_ms: u64) -> Config {
        Config {
            target_rpc_url: "http://localhost:8899".to_string(),
            reference_rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            poll_interval: Duration::from_secs(10),
            slot_lag_threshold,
            rtt_threshold_ms,
            alert_cooldown: Duration::from_secs(300),
            mistral_api_key: "test-key".to_string(),
            mistral_model: "mistral-small-latest".to_string(),
            telegram_bot_token: "test-token".to_string(),
            telegram_chat_id: "test-chat".to_string(),
        }
    }

    /// Создаёт тестовый ProbeResult с заданными значениями слотов и RTT.
    fn make_probe(target_slot: u64, target_rtt_ms: u64, reference_slot: u64) -> ProbeResult {
        ProbeResult {
            target: NodeMetrics {
                slot: target_slot,
                rtt_ms: target_rtt_ms,
                node_url: "http://localhost:8899".to_string(),
            },
            reference: NodeMetrics {
                slot: reference_slot,
                rtt_ms: 50, // RTT эталонной ноды не влияет на логику алертов
                node_url: "https://api.mainnet-beta.solana.com".to_string(),
            },
        }
    }

    #[test]
    fn test_no_alert_when_everything_is_fine() {
        // Нода не отстаёт (delta = -3, порог = 5), RTT в норме (200ms < 500ms)
        let cfg = make_config(5, 500);
        let probe = make_probe(100_000 - 3, 200, 100_000);
        let analysis = analyze(&probe, &cfg);

        assert_eq!(analysis.slot_delta, -3);
        assert!(!analysis.is_slot_lagging);
        assert!(!analysis.is_rtt_high);
        assert!(!analysis.needs_alert);
    }

    #[test]
    fn test_alert_when_slot_lagging() {
        // Нода отстаёт на 10 слотов при пороге 5 → алерт
        let cfg = make_config(5, 500);
        let probe = make_probe(100_000 - 10, 200, 100_000);
        let analysis = analyze(&probe, &cfg);

        assert_eq!(analysis.slot_delta, -10);
        assert!(analysis.is_slot_lagging);
        assert!(!analysis.is_rtt_high);
        assert!(analysis.needs_alert);
    }

    #[test]
    fn test_alert_when_rtt_high() {
        // RTT = 800ms при пороге 500ms → алерт, слоты в норме
        let cfg = make_config(5, 500);
        let probe = make_probe(100_000, 800, 100_000);
        let analysis = analyze(&probe, &cfg);

        assert!(!analysis.is_slot_lagging);
        assert!(analysis.is_rtt_high);
        assert!(analysis.needs_alert);
    }

    #[test]
    fn test_alert_when_both_conditions_violated() {
        // И отставание слотов И высокий RTT одновременно
        let cfg = make_config(5, 500);
        let probe = make_probe(100_000 - 20, 1200, 100_000);
        let analysis = analyze(&probe, &cfg);

        assert!(analysis.is_slot_lagging);
        assert!(analysis.is_rtt_high);
        assert!(analysis.needs_alert);
    }

    #[test]
    fn test_no_alert_at_exact_threshold() {
        // delta = -5 при пороге 5: NOT lagging (строгое неравенство: < -5)
        let cfg = make_config(5, 500);
        let probe = make_probe(100_000 - 5, 500, 100_000);
        let analysis = analyze(&probe, &cfg);

        // slot_delta = -5, порог = 5: условие slot_delta < -5 → false
        assert!(!analysis.is_slot_lagging);
        // rtt = 500, порог = 500: условие rtt > 500 → false
        assert!(!analysis.is_rtt_high);
        assert!(!analysis.needs_alert);
    }

    #[test]
    fn test_target_ahead_of_reference() {
        // target опережает reference (положительная дельта) — это нормально
        let cfg = make_config(5, 500);
        let probe = make_probe(100_010, 100, 100_000);
        let analysis = analyze(&probe, &cfg);

        assert_eq!(analysis.slot_delta, 10);
        assert!(!analysis.is_slot_lagging);
        assert!(!analysis.needs_alert);
    }

    #[test]
    fn test_status_text_ok() {
        let cfg = make_config(5, 500);
        let probe = make_probe(100_000, 100, 100_000);
        let analysis = analyze(&probe, &cfg);
        assert_eq!(analysis.status_text(), "OK");
    }

    #[test]
    fn test_status_text_lagging() {
        let cfg = make_config(5, 500);
        let probe = make_probe(99_990, 100, 100_000);
        let analysis = analyze(&probe, &cfg);
        assert!(analysis.status_text().contains("отставание слотов"));
    }
}
