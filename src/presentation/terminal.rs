use crate::domain::metrics::{HostSample, Metrics};
use crate::domain::service::{ServiceState, ServiceStatus};
use std::io::{self, IsTerminal, Write};
use std::path::Path;

pub fn print_dashboard(
    cpu_count: u64,
    db_path: &Path,
    host: &HostSample,
    services: &[ServiceState],
) {
    let interactive = io::stdout().is_terminal();
    if interactive {
        print!("\x1B[2J\x1B[H");
    }
    let first_metrics = services.iter().find_map(|service| service.metrics.as_ref());
    let host_cpu = first_metrics
        .map(|metrics| format!("{:.1}%", metrics.system_cpu_percent))
        .unwrap_or_else(|| "warming".to_string());
    let total_mb = host.mem_total_kb as f64 / 1024.0;
    let available_mb = host.mem_available_kb as f64 / 1024.0;
    let used_mb = (total_mb - available_mb).max(0.0);
    let service_cpu: f64 = services
        .iter()
        .filter_map(|service| service.metrics.as_ref())
        .map(|metrics| metrics.cpu_percent)
        .sum();
    let service_rss: f64 = services
        .iter()
        .filter_map(|service| service.metrics.as_ref())
        .map(|metrics| metrics.rss_mb)
        .sum();

    println!(
        "gsw  {}  |  {} logical CPUs  |  {} services  |  db {}",
        host.local_ts,
        cpu_count,
        services.len(),
        db_path.display()
    );
    println!(
        "HOST  CPU {:>7}  MEM {:>7.0}/{:<7.0} MiB  AVAILABLE {:>7.0} MiB  LOAD {:.2} {:.2} {:.2}  UPTIME {}",
        host_cpu,
        used_mb,
        total_mb,
        available_mb,
        host.load1,
        host.load5,
        host.load15,
        format_uptime(host.uptime_secs)
    );
    println!(
        "TRACKED TOTAL  CPU {:>7.1}%  RSS {:>8.1} MiB",
        service_cpu, service_rss
    );
    println!();
    println!(
        "SERVICE            STATUS       PID     CPU       RSS    FDS   THR    GOR    GOR/M     READ/s    WRITE/s  SIGNAL"
    );
    println!("{}", "─".repeat(119));
    for service in services {
        print_service(service, interactive);
    }

    let errors = services
        .iter()
        .filter_map(|service| {
            service
                .last_error
                .as_ref()
                .map(|error| (&service.name, error))
        })
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        println!();
        println!("DIAGNOSTICS");
        for (name, error) in errors {
            println!("  {name}: {error}");
        }
    }
    println!();
    println!("Ctrl+C to exit  |  gsw summary --db {}", db_path.display());
    let _ = io::stdout().flush();
}

fn print_service(service: &ServiceState, color: bool) {
    let metrics = service.metrics.as_ref();
    let signal = if service.leak_warning { "GROWING" } else { "-" };
    println!(
        "{:<18} {:<10} {:>7} {:>7} {:>9} {:>6} {:>5} {:>6} {:>8} {:>10} {:>10}  {}",
        truncate(&service.name, 18),
        colored_status(service.status, color),
        service
            .pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "-".into()),
        display_percent(metrics.map(|metrics| metrics.cpu_percent)),
        display_mb(metrics.map(|metrics| metrics.rss_mb)),
        display_u64(metrics.map(|metrics| metrics.file_descriptors)),
        display_u64(metrics.map(|metrics| metrics.threads)),
        display_optional_u64(metrics.and_then(|metrics| metrics.goroutines)),
        display_rate(metrics.and_then(|metrics| metrics.goroutine_growth_per_min)),
        display_io_rate(metrics.and_then(|metrics| metrics.read_mb_per_sec)),
        display_io_rate(metrics.and_then(|metrics| metrics.write_mb_per_sec)),
        if service.leak_warning && color {
            "\x1b[31mGROWING\x1b[0m"
        } else {
            signal
        }
    );
}

fn colored_status(status: ServiceStatus, color: bool) -> String {
    let value = status.as_str();
    if !color {
        return value.to_string();
    }
    let code = match status {
        ServiceStatus::Running => 32,
        ServiceStatus::Starting => 36,
        ServiceStatus::Waiting => 33,
        ServiceStatus::Error => 31,
    };
    format!("\x1b[{code}m{value}\x1b[0m")
}

fn display_percent(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "-".into())
}

fn display_mb(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}M"))
        .unwrap_or_else(|| "-".into())
}

fn display_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".into())
}

fn display_optional_u64(value: Option<u64>) -> String {
    display_u64(value)
}

fn display_rate(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:+.1}"))
        .unwrap_or_else(|| "-".into())
}

fn display_io_rate(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.2}M"))
        .unwrap_or_else(|| "-".into())
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[allow(dead_code)]
fn _system_mem_percent(metrics: &Metrics) -> f64 {
    if metrics.mem_total_mb <= 0.0 {
        0.0
    } else {
        metrics.mem_used_mb / metrics.mem_total_mb * 100.0
    }
}
