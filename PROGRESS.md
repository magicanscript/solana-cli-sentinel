# Отчёты о выполнении фаз

## Фаза 1 — Фундамент ✅
**Дата:** 2026-06-19

### Что сделано
- `Cargo.toml` — добавлены зависимости: `thiserror`, `tracing`, `tracing-subscriber`, `chrono`, `humantime`
- `src/error.rs` — `SentinelError` с вариантами: `Config`, `Rpc`, `Http`, `Llm`, `Telegram`
- `src/config/mod.rs` — `Config::from_env()` читает 10 параметров из env, возвращает `Result`, маскирует ключи в `summary()`
- `src/analysis/mod.rs` — чистая функция `analyze()`, структура `Analysis`, 7 unit-тестов
- `src/metrics/mod.rs` — заглушка со структурами `NodeMetrics`, `ProbeResult`
- `src/main.rs` / `src/commands/mod.rs` — обновлены под новую структуру модулей

### Результат сборки
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.04s
```
Ошибок нет. Предупреждения — ожидаемые (unused fields до реализации следующих фаз).

---

## Фаза 2 — Сбор метрик (metrics, status) ✅
**Дата:** 2026-06-22

### Что сделано
- `src/metrics/mod.rs` — реализованы `probe_node(url)` и `probe_both(cfg)`:
  - `probe_node` создаёт `RpcClient`, вызывает `get_slot()`, замеряет RTT через `Instant`
  - `probe_both` запускает оба запроса параллельно через `tokio::try_join!`
  - Ошибки оборачиваются в `SentinelError::Rpc` с URL ноды для диагностики
  - Добавлен `tracing::debug!` на каждый успешный опрос
- `src/commands/status.rs` — новый файл, реализация команды `status`:
  - Вызывает `probe_both` + `analyze`, выводит slot/RTT обеих нод
  - При `needs_alert=true` — статус в stderr, exit code 1
  - При OK — статус в stdout, exit code 0
- `src/commands/mod.rs` — подключён модуль `status`, убрана заглушка

### Результат сборки
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.38s
```
8 unit-тестов пройдено. Предупреждения — ожидаемые (поля для Фаз 3–4).

---

## Фаза 3 — Нотификации (llm, notify) ✅
**Дата:** 2026-06-22

### Что сделано
- `src/llm/mod.rs` — `LlmClient::generate_alert_text()`:
  - POST к Mistral Chat Completions API (`mistral-small-latest`)
  - Промпт на русском: URL ноды, дельта слотов, RTT, пороги → алерт ≤ 200 символов
  - Парсинг `choices[0].message.content`; ошибки → `SentinelError::Llm`
  - 4 unit-теста на построение промпта
- `src/notify/mod.rs` — `TelegramClient::send_message()`:
  - POST к Telegram Bot API (`sendMessage`, `parse_mode: HTML`)
  - Проверка `ok: true`; ошибки → `SentinelError::Telegram`
- `src/main.rs` — зарегистрированы модули `llm` и `notify`
- Интеграционные тесты с `#[ignore]` (запуск: `cargo test <name> -- --ignored --nocapture`)

### Результат живых тестов
- **Mistral**: сгенерировал алерт: `"Срочно: отставание слотов -15 (порог 5), RTT 800мс (порог 500мс) на ноде http://my-node:8899. Проверить сеть, нагрузку, синхронизацию."`
- **Telegram**: сообщение доставлено в чат `7874219393` (@heavenR1der / @magicanSol_bot)

---

## Фаза 4 — Демон (watch, Ctrl+C) ✅
**Дата:** 2026-06-22

### Что сделано
- `src/commands/watch.rs` — реализация команды `watch`:
  - Бесконечный цикл: первый тик выполняется немедленно, затем `tokio::select!` между `sleep(poll_interval)` и `ctrl_c()`
  - Graceful shutdown: Ctrl+C (SIGINT) прерывает сон и завершает процесс с логом
  - Cooldown: если с последнего алерта прошло меньше `alert_cooldown` — новый алерт подавляется с `warn!`
  - LLM-фолбэк: при ошибке Mistral API отправляется базовый текст алерта без LLM
  - Ошибки опроса и Telegram логируются через `tracing::error!`, демон продолжает работу
- `src/commands/mod.rs` — подключён модуль `watch`, заглушка удалена

### Результат сборки
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.64s
```
12 unit-тестов пройдено (2 интеграционных — `#[ignore]`). 1 предупреждение (unused field `reference_rtt_ms` — для Фазы 5).

---

## Фаза 5 — Hardening (tracing, retry, .env.example) ✅
**Дата:** 2026-06-22

### Что сделано
- `src/utils/mod.rs` — `retry_async<F, Fut, T, E>`:
  - Универсальный async-retry с экспоненциальным backoff: 1с → 2с → 4с
  - До `max_attempts` попыток; при исчерпании — возвращает последнюю ошибку
  - `warn!` на каждый неудачный промежуточный attempt с меткой операции
- `src/metrics/mod.rs` — `probe_both` использует `retry_async("target rpc", 3, ...)` и `retry_async("reference rpc", 3, ...)` через `tokio::try_join!`; каждая нода ретраится независимо
- `src/llm/mod.rs` — HTTP-вызов к Mistral обёрнут в `retry_async("mistral api", 3, ...)`; `reqwest::Client` клонируется дёшево (Arc-based) для `async move`-замыканий
- `src/notify/mod.rs` — HTTP-вызов к Telegram обёрнут в `retry_async("telegram api", 3, ...)`; аналогичный паттерн
- `src/main.rs` — улучшена инициализация tracing:
  - `EnvFilter::try_from_default_env()` с фолбэком на `"info"` (раньше была паника при отсутствии `RUST_LOG`)
  - `with_target(false)` — убирает путь модуля (`solana_cli_sentinel::metrics`) из каждой строки лога
- `.env.example` — документированный шаблон всех 10 переменных окружения с примерами и ссылками

### Результат сборки
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.57s
```
12 unit-тестов пройдено (2 интеграционных — `#[ignore]`). 1 предупреждение (unused field `reference_rtt_ms` в `Analysis` — поле зарезервировано для будущих метрик).
