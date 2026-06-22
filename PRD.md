# PRD: Solana CLI Sentinel

## Контекст

Проект — лёгкий демон-мониторинг для Solana-нод, написанный на Rust. Текущая кодовая база содержит только скелет: два stub-subcommand (`Block`, `Api`) без реализации и минимальный `Config`. Задача — реализовать полноценный sentinel-сервис: регулярный опрос нод, сравнительный анализ, AI-генерацию алертов и доставку через Telegram.

---

## 1. Цель продукта

Дать системному администратору Solana-ноды **пассивный мониторинг без UI и без дашборда** — демон запущен, работает в фоне, и только при проблеме присылает лаконичный технический алерт в Telegram. LLM используется не для украшения, а для генерации читаемого человеком текста из сырых чисел.

---

## 2. Пользовательские сценарии

| Сценарий | Описание |
|----------|----------|
| **Запуск демона** | `solana-cli-sentinel watch` запускает бесконечный polling loop, читает конфиг из `.env` |
| **Разовая проверка** | `solana-cli-sentinel status` делает один проб и печатает человекочитаемый отчёт в stdout |
| **Алерт по отставанию** | Нода отстала на > N слотов — в Telegram приходит текст, сгенерированный LLM |
| **Алерт по RTT** | Нода отвечает медленнее порога — аналогичный алерт |
| **Cooldown** | Повторные алерты подавляются на настраиваемый период, чтобы не спамить |
| **Graceful shutdown** | Ctrl+C останавливает демон без паники |

---

## 3. Функциональные требования

### 3.1 Сбор метрик
- Параллельный опрос двух нод: **target** (мониторимая) и **reference** (эталонная, по умолчанию mainnet-beta)
- На каждый проб фиксируется: текущий slot, RTT в миллисекундах, timestamp
- Используется `solana_rpc_client::nonblocking::rpc_client::RpcClient` (async)
- Оба узла опрашиваются конкурентно через `tokio::try_join!`

### 3.2 Анализ
- `slot_delta = target_slot - reference_slot` (отрицательное = target отстаёт)
- `is_slot_lagging = slot_delta < -(slot_lag_threshold)`
- `is_rtt_high = target_rtt_ms > rtt_threshold_ms`
- `needs_alert = is_slot_lagging || is_rtt_high`
- Анализ — чистая функция без I/O, полностью покрыта unit-тестами

### 3.3 LLM-агент
- При `needs_alert = true` и истёкшем cooldown — вызов Anthropic Messages API
- Промпт передаёт сырые числа: URL ноды, slot_delta, RTT, пороги
- Промпт требует: краткий технический алерт ≤ 200 символов, plain text, без markdown
- Парсинг ответа: `content[0].text` из JSON

### 3.4 Уведомления
- Готовый текст отправляется через Telegram Bot API (`sendMessage`)
- `parse_mode: "HTML"` — устойчиво к спецсимволам в URL нод
- При ошибке отправки — лог `tracing::error!`, без паники демона

### 3.5 Команды CLI
| Команда | Поведение |
|---------|-----------|
| `watch` | Бесконечный loop: probe → analyze → maybe_alert → sleep(interval) |
| `status` | Один проб, человекочитаемый вывод в stdout, exit code 1 если `needs_alert` |

---

## 4. Нефункциональные требования

- **Язык**: Rust, edition 2024
- **Async runtime**: Tokio (full features)
- **Логирование**: `tracing` + `tracing-subscriber` (env-filter по `RUST_LOG`)
- **Конфигурация**: только через env-переменные / `.env` файл (dotenvy)
- **Без постоянного хранилища**: cooldown хранится in-memory; перезапуск = новый алерт допустим
- **Одиночный бинарник**: `cargo build --release` → готовый исполняемый файл

---

## 5. Конфигурация (переменные окружения)

| Переменная | По умолчанию | Обязательная |
|-----------|-------------|:---:|
| `SOLANA_TARGET_RPC_URL` | — | ✓ |
| `SOLANA_REFERENCE_RPC_URL` | `https://api.mainnet-beta.solana.com` | |
| `SENTINEL_POLL_INTERVAL_SECS` | `10` | |
| `SENTINEL_SLOT_LAG_THRESHOLD` | `5` | |
| `SENTINEL_RTT_THRESHOLD_MS` | `500` | |
| `SENTINEL_ALERT_COOLDOWN_SECS` | `300` | |
| `MISTRAL_API_KEY` | — | ✓ |
| `MISTRAL_MODEL` | `mistral-small-latest` | |
| `TELEGRAM_BOT_TOKEN` | — | ✓ |
| `TELEGRAM_CHAT_ID` | — | ✓ |

При отсутствии обязательных переменных — `Config::from_env()` возвращает `Err`, процесс завершается с сообщением об ошибке.

---

## 6. Архитектура модулей

```
src/
  main.rs              — CLI-парсинг, инициализация tracing, Config → dispatch
  error.rs             — SentinelError enum (thiserror)
  config/mod.rs        — Config::from_env() → Result<Config>
  commands/
    mod.rs             — Commands enum { Watch, Status }
    watch.rs           — run(cfg) → daemon loop + Ctrl+C
    status.rs          — run(cfg) → one-shot + exit code
  metrics/mod.rs       — NodeMetrics, ProbeResult, probe_node(), probe_both()
  analysis/mod.rs      — Analysis, analyze(&ProbeResult, &Config) → Analysis
  alert/mod.rs         — AlertEngine { llm, telegram, last_alert_at, cooldown }
  llm/mod.rs           — LlmClient::generate_alert_text()
  notify/mod.rs        — TelegramClient::send_message()
  utils/mod.rs         — вспомогательные функции (time, formatting)
```

### Поток данных
```
.env → Config
         │
         ▼
  probe_both() [tokio::try_join!]
         │
         ▼
  ProbeResult { target, reference }
         │
         ▼
  analyze() ← чистая функция
         │
         ▼
  Analysis.needs_alert?
    ├─ нет → sleep → следующий цикл
    └─ да (+ cooldown?) →
              │
              ▼
        LlmClient → Mistral API → alert text
              │
              ▼
        TelegramClient → sendMessage
```

---

## 7. Новые зависимости Cargo.toml

```toml
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
humantime = "2.1"
```

> `thiserror`, `tracing`, `chrono` уже присутствуют транзитивно через `solana-sdk` — реально новая загрузка только `tracing-subscriber` и `humantime`.

---

## 8. Интеграции

### Mistral Chat Completions API
- Endpoint: `POST https://api.mistral.ai/v1/chat/completions`
- Headers: `Authorization: Bearer <MISTRAL_API_KEY>`, `Content-Type: application/json`
- Body: `{ "model": "...", "max_tokens": 256, "messages": [{"role": "user", "content": "<prompt>"}] }`
- Парсинг: `response["choices"][0]["message"]["content"]` как `serde_json::Value`

### Telegram Bot API
- Endpoint: `POST https://api.telegram.org/bot{token}/sendMessage`
- Body: `{ "chat_id": "...", "text": "...", "parse_mode": "HTML" }`
- Проверка: `response["ok"] == true`, иначе — `response["description"]` в ошибку

---

## 9. Фазы реализации

| Фаза | Модули | Можно проверить |
|------|--------|----------------|
| 1 — Фундамент | `error.rs`, расширенный `config`, `analysis` | `cargo test` на unit-тестах analyze() |
| 2 — Сбор метрик | `metrics`, `commands/status.rs` | `cargo run -- status` вручную |
| 3 — Нотификации | `llm`, `notify` | отдельные ручные тесты |
| 4 — Демон | `alert`, `commands/watch.rs`, Ctrl+C | полный запуск `watch` |
| 5 — Hardening | tracing везде, retry в probe_node, `.env.example` | интеграционный тест end-to-end |

---

## 10. Верификация

1. `cargo build` — компилируется без ошибок
2. `cargo test` — unit-тесты `analysis::analyze()` покрывают: нет алерта / алерт по слотам / алерт по RTT / оба одновременно
3. `cargo run -- status` с реальным `SOLANA_TARGET_RPC_URL` — печатает slot, RTT, статус
4. `cargo run -- watch` с реальными ключами — при искусственно заниженном `SENTINEL_SLOT_LAG_THRESHOLD=0` в Telegram приходит сгенерированный LLM-алерт
5. Повторный алерт подавляется в течение `SENTINEL_ALERT_COOLDOWN_SECS`
6. Ctrl+C завершает процесс без паники

---

## 11. За рамками v1

- Мониторинг нескольких нод одновременно (fan-out)
- Персистентный cooldown (сохранение в файл при перезапуске)
- Метрики голосований валидатора (vote lag)
- Webhook-нотификации помимо Telegram
- Systemd unit-файл для деплоя
