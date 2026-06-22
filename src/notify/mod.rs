/// Модуль уведомлений: отправка сообщений через Telegram Bot API.
///
/// Использует метод sendMessage:
/// - Endpoint: POST https://api.telegram.org/bot{token}/sendMessage
/// - parse_mode: "HTML" — устойчиво к спецсимволам в URL нод (< > & и т.д.)
///
/// При ошибке отправки — логируем через tracing::error! и возвращаем Err.
/// Демон не паникует и продолжает работу (обрабатывается на уровне alert).
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error};

use crate::config::Config;
use crate::error::SentinelError;
use crate::utils;

/// HTTP-клиент для Telegram Bot API.
///
/// Держит `reqwest::Client` для переиспользования соединений.
pub struct TelegramClient {
    http: Client,
}

impl TelegramClient {
    /// Создаёт новый клиент. Вызывать один раз при старте приложения.
    pub fn new() -> Self {
        Self {
            http: Client::new(),
        }
    }

    /// Отправляет текстовое сообщение в Telegram-чат из конфигурации.
    ///
    /// # Аргументы
    /// * `text` — текст сообщения (уже сгенерированный LLM)
    /// * `cfg`  — конфигурация: токен бота, chat_id
    ///
    /// # Ошибки
    /// - `SentinelError::Http`      — сетевая ошибка или таймаут
    /// - `SentinelError::Telegram`  — Telegram API вернул `ok: false`
    pub async fn send_message(&self, text: &str, cfg: &Config) -> Result<(), SentinelError> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            cfg.telegram_bot_token
        );

        let body = json!({
            "chat_id": cfg.telegram_chat_id,
            "text": text,
            // HTML позволяет использовать <b>, <i> и корректно обрабатывает
            // спецсимволы в URL нод без дополнительного экранирования
            "parse_mode": "HTML"
        });

        debug!(chat_id = cfg.telegram_chat_id, "отправляю сообщение в Telegram");

        let http = self.http.clone();

        let response: serde_json::Value = utils::retry_async("telegram api", 3, || {
            let http = http.clone();
            let url = url.clone();
            let b = body.clone();
            async move {
                http.post(url)
                    .json(&b)
                    .send()
                    .await
                    .map_err(SentinelError::Http)?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(SentinelError::Http)
            }
        })
        .await?;

        // Telegram возвращает {"ok": true, ...} при успехе
        // или {"ok": false, "description": "..."} при ошибке
        if response["ok"].as_bool() != Some(true) {
            let description = response["description"]
                .as_str()
                .unwrap_or("неизвестная ошибка")
                .to_string();
            error!(chat_id = cfg.telegram_chat_id, error = description, "ошибка отправки в Telegram");
            return Err(SentinelError::Telegram(description));
        }

        debug!("сообщение успешно отправлено");

        Ok(())
    }
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Живой интеграционный тест: реальная отправка в Telegram.
    /// Запуск: cargo test telegram_live -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn test_telegram_live() {
        dotenvy::dotenv().ok();

        let cfg = Config {
            target_rpc_url: "http://localhost:8899".to_string(),
            reference_rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            poll_interval: Duration::from_secs(10),
            slot_lag_threshold: 5,
            rtt_threshold_ms: 500,
            alert_cooldown: Duration::from_secs(300),
            mistral_api_key: "placeholder".to_string(),
            mistral_model: "mistral-small-latest".to_string(),
            telegram_bot_token: std::env::var("TELEGRAM_BOT_TOKEN")
                .expect("TELEGRAM_BOT_TOKEN не задан"),
            telegram_chat_id: std::env::var("TELEGRAM_CHAT_ID")
                .expect("TELEGRAM_CHAT_ID не задан"),
        };

        let client = TelegramClient::new();
        client
            .send_message("Тест solana-cli-sentinel: Telegram-клиент работает.", &cfg)
            .await
            .expect("Ошибка отправки в Telegram");

        println!("Сообщение отправлено успешно");
    }
}
