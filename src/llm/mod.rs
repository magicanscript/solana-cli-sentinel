/// Модуль LLM-клиента: генерация текста алерта через Mistral API.
///
/// Используется Mistral Chat Completions API:
/// - Endpoint: POST https://api.mistral.ai/v1/chat/completions
/// - Аутентификация: Bearer-токен в заголовке Authorization
/// - Формат ответа: OpenAI-совместимый, текст в choices[0].message.content
///
/// Промпт передаёт LLM сырые числа (URL, дельта слотов, RTT, пороги)
/// и требует краткий технический алерт ≤ 200 символов, plain text, без markdown.
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, error};

use crate::analysis::Analysis;
use crate::config::Config;
use crate::error::SentinelError;

/// HTTP-клиент для Mistral API.
///
/// Держит `reqwest::Client` внутри — он потокобезопасен и переиспользует
/// HTTP-соединения между вызовами (connection pooling).
pub struct LlmClient {
    http: Client,
}

impl LlmClient {
    /// Создаёт новый клиент. Вызывать один раз при старте приложения.
    pub fn new() -> Self {
        Self {
            http: Client::new(),
        }
    }

    /// Генерирует короткий текст алерта по данным анализа.
    ///
    /// # Аргументы
    /// * `target_url` — URL наблюдаемой ноды (для контекста в промпте)
    /// * `analysis`   — результат анализа с дельтой слотов и RTT
    /// * `cfg`        — конфигурация: ключ API, модель, пороги
    ///
    /// # Возвращает
    /// Строку ≤ 200 символов — текст алерта сгенерированный LLM.
    ///
    /// # Ошибки
    /// - `SentinelError::Http`  — сетевая ошибка или таймаут
    /// - `SentinelError::Llm`   — неожиданная структура JSON-ответа
    pub async fn generate_alert_text(
        &self,
        target_url: &str,
        analysis: &Analysis,
        cfg: &Config,
    ) -> Result<String, SentinelError> {
        let prompt = build_prompt(target_url, analysis, cfg);

        debug!(model = cfg.mistral_model, "отправляю запрос к Mistral API");

        let body = json!({
            "model": cfg.mistral_model,
            "max_tokens": 256,
            "messages": [
                { "role": "user", "content": prompt }
            ]
        });

        let response: Value = self
            .http
            .post("https://api.mistral.ai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", cfg.mistral_api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            // reqwest::Error автоматически конвертируется в SentinelError::Http через #[from]
            .map_err(SentinelError::Http)?
            .json()
            .await
            .map_err(SentinelError::Http)?;

        // Извлекаем текст из choices[0].message.content
        let text = response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                error!(response = %response, "неожиданная структура ответа Mistral");
                SentinelError::Llm(format!(
                    "поле choices[0].message.content отсутствует в ответе: {response}"
                ))
            })?
            .trim()
            .to_string();

        debug!(chars = text.len(), "алерт сгенерирован");

        Ok(text)
    }
}

/// Строит промпт для LLM из данных анализа.
///
/// Промпт на русском — LLM должен отвечать на том же языке.
/// Явно перечисляем какие условия нарушены, чтобы алерт был точным.
fn build_prompt(target_url: &str, analysis: &Analysis, cfg: &Config) -> String {
    // Формируем список нарушенных условий
    let mut issues = Vec::new();
    if analysis.is_slot_lagging {
        issues.push(format!(
            "отставание слотов: {} (порог: {})",
            analysis.slot_delta, cfg.slot_lag_threshold
        ));
    }
    if analysis.is_rtt_high {
        issues.push(format!(
            "высокий RTT: {}ms (порог: {}ms)",
            analysis.target_rtt_ms, cfg.rtt_threshold_ms
        ));
    }

    format!(
        "Ты — система мониторинга Solana-ноды. Зафиксирован инцидент.\n\
        Нода: {target_url}\n\
        Проблемы: {issues}\n\n\
        Напиши краткий технический алерт на русском языке.\n\
        Требования: максимум 200 символов, только текст, без markdown, без emoji.",
        issues = issues.join("; ")
    )
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_config() -> Config {
        Config {
            target_rpc_url: "http://localhost:8899".to_string(),
            reference_rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            poll_interval: Duration::from_secs(10),
            slot_lag_threshold: 5,
            rtt_threshold_ms: 500,
            alert_cooldown: Duration::from_secs(300),
            mistral_api_key: "test-key".to_string(),
            mistral_model: "mistral-small-latest".to_string(),
            telegram_bot_token: "test-token".to_string(),
            telegram_chat_id: "test-chat".to_string(),
        }
    }

    fn make_analysis(slot_delta: i64, target_rtt_ms: u64) -> Analysis {
        Analysis {
            slot_delta,
            target_rtt_ms,
            reference_rtt_ms: 50,
            is_slot_lagging: slot_delta < -5,
            is_rtt_high: target_rtt_ms > 500,
            needs_alert: slot_delta < -5 || target_rtt_ms > 500,
        }
    }

    #[test]
    fn test_prompt_contains_url() {
        let cfg = make_config();
        let analysis = make_analysis(-10, 200);
        let prompt = build_prompt("http://my-node:8899", &analysis, &cfg);
        assert!(prompt.contains("http://my-node:8899"));
    }

    #[test]
    fn test_prompt_contains_slot_issue() {
        let cfg = make_config();
        let analysis = make_analysis(-10, 200);
        let prompt = build_prompt("http://my-node:8899", &analysis, &cfg);
        assert!(prompt.contains("отставание слотов"));
        assert!(prompt.contains("-10"));
    }

    #[test]
    fn test_prompt_contains_rtt_issue() {
        let cfg = make_config();
        let analysis = make_analysis(0, 800);
        let prompt = build_prompt("http://my-node:8899", &analysis, &cfg);
        assert!(prompt.contains("высокий RTT"));
        assert!(prompt.contains("800ms"));
    }

    #[test]
    fn test_prompt_contains_both_issues() {
        let cfg = make_config();
        let analysis = make_analysis(-20, 1200);
        let prompt = build_prompt("http://my-node:8899", &analysis, &cfg);
        assert!(prompt.contains("отставание слотов"));
        assert!(prompt.contains("высокий RTT"));
    }

    /// Живой интеграционный тест: реальный вызов Mistral API.
    /// Запуск: cargo test llm_live -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn test_llm_live() {
        dotenvy::dotenv().ok();

        let cfg = Config {
            target_rpc_url: "http://localhost:8899".to_string(),
            reference_rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            poll_interval: Duration::from_secs(10),
            slot_lag_threshold: 5,
            rtt_threshold_ms: 500,
            alert_cooldown: Duration::from_secs(300),
            mistral_api_key: std::env::var("MISTRAL_API_KEY").expect("MISTRAL_API_KEY не задан"),
            mistral_model: std::env::var("MISTRAL_MODEL")
                .unwrap_or_else(|_| "mistral-small-latest".to_string()),
            telegram_bot_token: "placeholder".to_string(),
            telegram_chat_id: "placeholder".to_string(),
        };

        let analysis = make_analysis(-15, 800);
        let client = LlmClient::new();
        let text = client
            .generate_alert_text("http://my-node:8899", &analysis, &cfg)
            .await
            .expect("LLM вернул ошибку");

        println!("Сгенерированный алерт:\n{text}");
        assert!(!text.is_empty(), "алерт не должен быть пустым");
        assert!(text.len() <= 300, "алерт слишком длинный: {} символов", text.len());
    }
}
