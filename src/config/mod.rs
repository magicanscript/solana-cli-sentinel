/// Конфигурация всего приложения.
///
/// Все параметры читаются из переменных окружения (или из файла `.env` через `dotenvy`).
/// Нет CLI-флагов для настроек — только env, чтобы демон было удобно запускать
/// через systemd/docker с переменными окружения.
use std::env;
use std::time::Duration;

use crate::error::SentinelError;

/// Главная структура конфигурации.
/// Создаётся один раз при старте через `Config::from_env()` и затем передаётся
/// по ссылке во все модули.
#[derive(Debug, Clone)]
pub struct Config {
    // --- Solana RPC ---

    /// URL ноды, за которой ведётся наблюдение (обязательное поле).
    /// Пример: "http://192.168.1.10:8899"
    pub target_rpc_url: String,

    /// URL эталонной ноды для сравнения слотов.
    /// По умолчанию — официальная mainnet-beta.
    pub reference_rpc_url: String,

    // --- Параметры polling ---

    /// Интервал между опросами нод.
    /// Читается из `SENTINEL_POLL_INTERVAL_SECS`, по умолчанию 10 секунд.
    pub poll_interval: Duration,

    // --- Пороги для алертов ---

    /// Максимально допустимое отставание target-ноды от reference в слотах.
    /// Если `reference_slot - target_slot > slot_lag_threshold` → алерт.
    pub slot_lag_threshold: u64,

    /// Максимально допустимое время ответа target-ноды в миллисекундах.
    /// Если RTT > rtt_threshold_ms → алерт.
    pub rtt_threshold_ms: u64,

    /// Минимальный интервал между двумя алертами (cooldown).
    /// Предотвращает спам в Telegram при затяжной проблеме.
    pub alert_cooldown: Duration,

    // --- Anthropic LLM ---

    /// Секретный ключ Anthropic API. Обязательное поле.
    /// Читается из `ANTHROPIC_API_KEY`.
    pub anthropic_api_key: String,

    /// ID модели для генерации алертов.
    /// Читается из `ANTHROPIC_MODEL`, по умолчанию "claude-sonnet-4-6".
    pub anthropic_model: String,

    // --- Telegram ---

    /// Токен Telegram-бота (формат: "123456:ABC-DEF..."). Обязательное поле.
    pub telegram_bot_token: String,

    /// ID чата или канала для отправки алертов (например, "-1001234567890"). Обязательное поле.
    pub telegram_chat_id: String,
}

impl Config {
    /// Читает все параметры из переменных окружения.
    ///
    /// Обязательные переменные: `SOLANA_TARGET_RPC_URL`, `ANTHROPIC_API_KEY`,
    /// `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`.
    /// Если хотя бы одна отсутствует — возвращает `Err(SentinelError::Config)`.
    ///
    /// Остальные переменные опциональны и имеют значения по умолчанию.
    pub fn from_env() -> Result<Self, SentinelError> {
        Ok(Self {
            // Обязательные поля: используем вспомогательную функцию require_var(),
            // которая возвращает ошибку с понятным сообщением если переменная не задана.
            target_rpc_url: require_var("SOLANA_TARGET_RPC_URL")?,

            // Опциональные поля: используем unwrap_or_else() для задания дефолта.
            reference_rpc_url: env::var("SOLANA_REFERENCE_RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),

            poll_interval: Duration::from_secs(
                parse_u64_var("SENTINEL_POLL_INTERVAL_SECS", 10)?,
            ),

            slot_lag_threshold: parse_u64_var("SENTINEL_SLOT_LAG_THRESHOLD", 5)?,

            rtt_threshold_ms: parse_u64_var("SENTINEL_RTT_THRESHOLD_MS", 500)?,

            alert_cooldown: Duration::from_secs(
                parse_u64_var("SENTINEL_ALERT_COOLDOWN_SECS", 300)?,
            ),

            anthropic_api_key: require_var("ANTHROPIC_API_KEY")?,

            anthropic_model: env::var("ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-6".to_string()),

            telegram_bot_token: require_var("TELEGRAM_BOT_TOKEN")?,

            telegram_chat_id: require_var("TELEGRAM_CHAT_ID")?,
        })
    }

    /// Возвращает строку с обзором конфигурации для логирования при старте демона.
    /// API-ключи маскируются — в лог попадает только первые 8 символов.
    pub fn summary(&self) -> String {
        format!(
            "target={} reference={} interval={:?} slot_lag_threshold={} rtt_threshold_ms={}ms cooldown={:?} model={}",
            self.target_rpc_url,
            self.reference_rpc_url,
            self.poll_interval,
            self.slot_lag_threshold,
            self.rtt_threshold_ms,
            self.alert_cooldown,
            self.anthropic_model,
        )
    }
}

/// Читает обязательную env-переменную.
/// Возвращает `SentinelError::Config` с именем переменной если она не задана.
fn require_var(name: &str) -> Result<String, SentinelError> {
    env::var(name).map_err(|_| {
        SentinelError::Config(format!(
            "обязательная переменная '{name}' не задана в .env или окружении"
        ))
    })
}

/// Читает числовую env-переменную типа u64.
/// Если переменная не задана — возвращает `default`.
/// Если задана, но не парсится как число — возвращает `SentinelError::Config`.
fn parse_u64_var(name: &str, default: u64) -> Result<u64, SentinelError> {
    match env::var(name) {
        // Переменная не задана — используем дефолт, ошибки нет
        Err(_) => Ok(default),
        // Переменная задана — пробуем распарсить
        Ok(val) => val.parse::<u64>().map_err(|_| {
            SentinelError::Config(format!(
                "переменная '{name}' должна быть числом, получено: '{val}'"
            ))
        }),
    }
}
