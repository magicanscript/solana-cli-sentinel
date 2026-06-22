/// Точка входа в приложение.
///
/// Порядок инициализации:
/// 1. Загрузка `.env` через `dotenvy` (если файл существует — игнорирует ошибку)
/// 2. Инициализация логирования через `tracing_subscriber`
///    — уровень читается из `RUST_LOG` (например: RUST_LOG=debug)
/// 3. Разбор CLI-аргументов через `clap`
/// 4. Чтение конфигурации из env-переменных
/// 5. Передача управления нужной команде
use anyhow::Result;
use clap::Parser;
use dotenvy::dotenv;

mod analysis;
mod commands;
mod config;
mod error;
mod llm;
mod metrics;
mod notify;
mod utils;

#[derive(Parser)]
#[command(name = "solana-cli-sentinel")]
#[command(about = "Демон мониторинга Solana-ноды с AI-алертами в Telegram")]
struct Cli {
    #[command(subcommand)]
    command: commands::Commands,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Загружаем .env если он существует; ok() подавляет ошибку если файл не найден
    dotenv().ok();

    // Инициализируем трейсинг.
    // Уровень читается из RUST_LOG; по умолчанию — info.
    // with_target(false)    — убирает путь модуля из каждой строки лога
    // with_thread_ids(false) — убирает ID потоков (не нужны для однопоточных сценариев)
    // Пример: RUST_LOG=debug cargo run -- status
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    // Читаем конфигурацию один раз здесь и передаём в команду.
    // Если обязательные env-переменные не заданы — process завершится с ошибкой.
    let cfg = config::Config::from_env()?;

    commands::execute(cli.command, cfg).await?;

    Ok(())
}
