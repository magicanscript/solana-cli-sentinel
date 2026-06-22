/// Команда `watch` — демон непрерывного мониторинга Solana-ноды.
///
/// Алгоритм:
/// 1. Инициализирует LLM- и Telegram-клиенты один раз при старте
/// 2. Немедленно выполняет первый опрос (tick)
/// 3. Ждёт `poll_interval` через `tokio::select!`, который прерывается по Ctrl+C
/// 4. При каждом тике: опрос → анализ → если нужен алерт и cooldown истёк → LLM → Telegram
///
/// Cooldown предотвращает спам: если алерт уже отправлялся менее чем `alert_cooldown` назад,
/// новый алерт подавляется с предупреждением в лог.
///
/// При ошибке опроса или отправки — логирует и продолжает цикл (демон не падает).
use std::time::Instant;

use anyhow::Result;
use tracing::{error, info, warn};

use crate::analysis;
use crate::config::Config;
use crate::llm::LlmClient;
use crate::metrics;
use crate::notify::TelegramClient;

/// Запускает бесконечный цикл мониторинга.
///
/// Завершается только при получении Ctrl+C (SIGINT).
/// Все ошибки внутри цикла обрабатываются локально — демон не паникует.
pub async fn run(cfg: Config) -> Result<()> {
    info!("демон запущен: {}", cfg.summary());

    let llm = LlmClient::new();
    let tg = TelegramClient::new();
    let mut last_alert_at: Option<Instant> = None;
    let poll_interval = cfg.poll_interval;

    loop {
        tick(&cfg, &llm, &tg, &mut last_alert_at).await;

        // Ждём следующий тик или Ctrl+C.
        // tokio::select! гарантирует, что сигнал обработается даже во время сна.
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                match result {
                    Ok(()) => info!("получен Ctrl+C, завершаю работу..."),
                    Err(e) => error!("ошибка обработки сигнала: {e}"),
                }
                break;
            }
            _ = tokio::time::sleep(poll_interval) => {}
        }
    }

    info!("демон остановлен");
    Ok(())
}

/// Один цикл опроса: probe → analyze → (опционально) alert.
///
/// Ошибки логируются и не прерывают демон.
async fn tick(
    cfg: &Config,
    llm: &LlmClient,
    tg: &TelegramClient,
    last_alert_at: &mut Option<Instant>,
) {
    // Параллельный опрос обеих нод
    let probe = match metrics::probe_both(cfg).await {
        Ok(p) => p,
        Err(e) => {
            error!("ошибка опроса нод: {e}");
            return;
        }
    };

    let analysis = analysis::analyze(&probe, cfg);

    info!(
        "тик: delta={:+} target_rtt={}ms статус={}",
        analysis.slot_delta,
        analysis.target_rtt_ms,
        analysis.status_text(),
    );

    if !analysis.needs_alert {
        return;
    }

    // Проверяем cooldown: не слать алерт повторно слишком часто
    if let Some(last) = *last_alert_at {
        let elapsed = last.elapsed();
        if elapsed < cfg.alert_cooldown {
            let remaining = cfg.alert_cooldown.saturating_sub(elapsed);
            warn!(
                "алерт подавлен cooldown: до следующего {}с",
                remaining.as_secs()
            );
            return;
        }
    }

    // Генерируем текст через LLM; при ошибке — фолбэк на базовый текст
    let alert_text = match llm
        .generate_alert_text(&cfg.target_rpc_url, &analysis, cfg)
        .await
    {
        Ok(text) => text,
        Err(e) => {
            error!("ошибка LLM: {e} — использую базовый текст алерта");
            format!("ALERT {}: {}", cfg.target_rpc_url, analysis.status_text())
        }
    };

    // Отправляем в Telegram
    if let Err(e) = tg.send_message(&alert_text, cfg).await {
        error!("ошибка отправки в Telegram: {e}");
        return;
    }

    *last_alert_at = Some(Instant::now());
    info!("алерт отправлен: {alert_text}");
}
