/// Реестр CLI-команд приложения.
///
/// Текущие команды:
/// - `status` — разовый опрос нод, вывод в stdout (реализуется в Фазе 2)
/// - `watch`  — демон: бесконечный polling loop (реализуется в Фазе 4)
///
/// Новые команды добавляются как варианты в `Commands` и ветки в `execute`.
use anyhow::Result;
use clap::Subcommand;

use crate::config::Config;

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
        // TODO Фаза 2: реализовать commands::status::run(cfg)
        Commands::Status => {
            println!("Команда 'status' будет реализована в Фазе 2.");
            println!("Конфигурация загружена: {}", cfg.summary());
        }
        // TODO Фаза 4: реализовать commands::watch::run(cfg)
        Commands::Watch => {
            println!("Команда 'watch' будет реализована в Фазе 4.");
            println!("Конфигурация загружена: {}", cfg.summary());
        }
    }
    Ok(())
}
