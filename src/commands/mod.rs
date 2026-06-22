use anyhow::Result;
use clap::Subcommand;

use crate::config::Config;

pub mod status;
pub mod watch;

#[derive(Subcommand)]
pub enum Commands {
    /// Один опрос обеих нод: выводит слот, RTT и статус в stdout.
    /// Завершается с exit code 1 если обнаружена проблема.
    Status,

    /// Запускает демон: опрашивает ноды каждые N секунд,
    /// при проблеме генерирует алерт через LLM и отправляет в Telegram.
    Watch,
}

/// Диспетчер команд. Получает разобранную команду и конфигурацию,
/// вызывает соответствующий обработчик.
pub async fn execute(cmd: Commands, cfg: Config) -> Result<()> {
    match cmd {
        Commands::Status => status::run(cfg).await,
        Commands::Watch => watch::run(cfg).await,
    }
}
