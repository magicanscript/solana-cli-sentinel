/// Команда `status` — разовый опрос нод с выводом результата в stdout.
///
/// Алгоритм:
/// 1. Параллельно опрашиваем target и reference ноды (`probe_both`)
/// 2. Анализируем результат (`analyze`)
/// 3. Печатаем человекочитаемый отчёт в stdout
/// 4. Если обнаружена проблема — выходим с кодом 1
///
/// Пример вывода (всё хорошо):
/// ```text
/// Опрашиваю ноды...
///   target    http://192.168.1.10:8899              slot=300123456   rtt=45ms
///   reference https://api.mainnet-beta.solana.com   slot=300123460   rtt=120ms
/// delta=-4   статус=OK
/// ```
///
/// Пример вывода (проблема):
/// ```text
/// Опрашиваю ноды...
///   target    http://192.168.1.10:8899              slot=300000100   rtt=45ms
///   reference https://api.mainnet-beta.solana.com   slot=300000200   rtt=120ms
/// delta=-100   статус=отставание слотов: -100 (порог: нарушен)
/// [exit code 1]
/// ```
use anyhow::Result;

use crate::analysis;
use crate::config::Config;
use crate::metrics;

/// Выполняет разовый опрос нод и печатает результат в stdout.
///
/// Завершает процесс с exit code 1 если `analysis.needs_alert = true`.
/// Это позволяет использовать команду в shell-скриптах:
/// ```sh
/// solana-cli-sentinel status || notify_oncall.sh
/// ```
pub async fn run(cfg: Config) -> Result<()> {
    println!("Опрашиваю ноды...");

    // probe_both выполняет оба запроса параллельно
    let probe = metrics::probe_both(&cfg).await?;
    let analysis = analysis::analyze(&probe, &cfg);

    // Вывод строки для каждой ноды
    println!(
        "  target    {:<45}  slot={:<12}  rtt={}ms",
        probe.target.node_url, probe.target.slot, probe.target.rtt_ms
    );
    println!(
        "  reference {:<45}  slot={:<12}  rtt={}ms",
        probe.reference.node_url, probe.reference.slot, probe.reference.rtt_ms
    );

    // Итоговая строка: дельта слотов и текстовый статус
    let status_line = format!(
        "delta={:+}   target_rtt={}ms   статус={}",
        analysis.slot_delta,
        analysis.target_rtt_ms,
        analysis.status_text(),
    );

    if analysis.needs_alert {
        // Статус проблемы — в stderr, чтобы скрипты могли разделить stdout и stderr
        eprintln!("{status_line}");
        // exit code 1 — стандартный Unix-сигнал "нашёл проблему"
        std::process::exit(1);
    } else {
        println!("{status_line}");
    }

    Ok(())
}
