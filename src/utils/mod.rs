/// Утилиты общего назначения.
use std::future::Future;
use std::time::Duration;

use tracing::warn;

/// Повторяет асинхронную операцию при ошибке с экспоненциальным backoff.
///
/// Стратегия: до `max_attempts` попыток; задержки между ними: 1с, 2с, 4с, ...
/// При исчерпании всех попыток возвращает последнюю ошибку без паники.
///
/// # Аргументы
/// * `label`        — метка операции для строк предупреждения в лог
/// * `max_attempts` — общее число попыток (включая первую)
/// * `op`           — фабрика Future: вызывается заново на каждой попытке
///
/// # Пример
/// ```ignore
/// let slot = retry_async("get_slot", 3, || rpc.get_slot()).await?;
/// ```
pub async fn retry_async<F, Fut, T, E>(
    label: &str,
    max_attempts: u32,
    mut op: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut delay = Duration::from_secs(1);
    for attempt in 1..=max_attempts {
        match op().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if attempt < max_attempts {
                    warn!(
                        "{label}: попытка {attempt}/{max_attempts} неудачна ({e}), повтор через {}с",
                        delay.as_secs()
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                } else {
                    return Err(e);
                }
            }
        }
    }
    unreachable!()
}
