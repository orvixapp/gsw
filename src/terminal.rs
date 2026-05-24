use crate::metrics::{Metrics, Peak};
use std::io::{self, Write};
use std::path::Path;

pub fn print_live(pid: i32, cpu_count: u64, db_path: &Path, metrics: &Metrics, peak: &Peak) {
    print!("\x1B[2J\x1B[H");
    println!(
        "gsw  |  PID {pid}  |  CPU logica: {cpu_count}  |  db: {}",
        db_path.display()
    );
    println!(
        "Hora local       : {}  |  uptime: {}  |  load: {:.2} {:.2} {:.2}",
        metrics.local_ts,
        format_uptime(metrics.uptime_secs),
        metrics.load1,
        metrics.load5,
        metrics.load15
    );
    println!();
    println!(
        "Proceso CPU      : {:>8.2}%  {}  {:>5.1}% de la instancia",
        metrics.cpu_percent,
        bar(metrics.cpu_capacity_percent, 30),
        metrics.cpu_capacity_percent
    );
    println!(
        "Sistema CPU      : {:>8.2}%  {}",
        metrics.system_cpu_percent,
        bar(metrics.system_cpu_percent, 30)
    );
    println!();
    println!(
        "Proceso RAM      : {:>8.2} MB  {}  {:>5.1}% de RAM",
        metrics.rss_mb,
        bar(metrics.rss_system_percent, 30),
        metrics.rss_system_percent
    );
    println!(
        "Sistema RAM      : {:>8.2}/{:.2} MB usados  {}  libre {:.2} MB",
        metrics.mem_used_mb,
        metrics.mem_total_mb,
        bar(system_mem_percent(metrics), 30),
        metrics.mem_available_mb
    );
    println!("Mem virtual proc : {:>8.2} MB", metrics.vm_size_mb);
    println!("Threads          : {:>8}", metrics.threads);
    println!(
        "Disco leido      : {:>8}",
        display_optional_mb(metrics.read_mb)
    );
    println!(
        "Disco escrito    : {:>8}",
        display_optional_mb(metrics.write_mb)
    );
    println!();
    println!(
        "Pico CPU proc    : {:>8.2}%   {}",
        peak.cpu_percent,
        empty_dash(&peak.cpu_ts)
    );
    println!(
        "Pico RAM proc    : {:>8.2} MB  {}",
        peak.rss_mb,
        empty_dash(&peak.rss_ts)
    );
    println!(
        "Pico CPU sistema : {:>8.2}%   {}",
        peak.system_cpu_percent,
        empty_dash(&peak.system_cpu_ts)
    );
    println!(
        "Pico RAM sistema : {:>8.2} MB  {}",
        peak.mem_used_mb,
        empty_dash(&peak.mem_used_ts)
    );
    println!();
    println!(
        "Ctrl+C para salir. Luego: gsw summary --db {}",
        db_path.display()
    );
    let _ = io::stdout().flush();
}

pub fn print_waiting_container(container: &str, db_path: &Path) {
    print!("\x1B[2J\x1B[H");
    println!(
        "gsw  |  Docker container: {container}  |  db: {}",
        db_path.display()
    );
    println!();
    println!("Esperando a que el contenedor exista y este corriendo...");
    println!("Durante un deploy con docker stop/rm/run esto es normal.");
    println!();
    println!("Ctrl+C para salir.");
    let _ = io::stdout().flush();
}

pub fn print_attaching_container(container: &str, pid: i32, db_path: &Path) {
    print!("\x1B[2J\x1B[H");
    println!(
        "gsw  |  Docker container: {container}  |  PID host: {pid}  |  db: {}",
        db_path.display()
    );
    println!();
    println!("Contenedor detectado. Tomando muestra inicial...");
    println!();
    println!("Ctrl+C para salir.");
    let _ = io::stdout().flush();
}

fn display_optional_mb(value: Option<f64>) -> String {
    value
        .map(|number| format!("{number:.2} MB"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn empty_dash(value: &str) -> &str {
    if value.is_empty() { "-" } else { value }
}

fn bar(percent: f64, width: usize) -> String {
    let clamped = percent.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "#".repeat(filled), ".".repeat(empty))
}

fn system_mem_percent(metrics: &Metrics) -> f64 {
    if metrics.mem_total_mb <= 0.0 {
        0.0
    } else {
        metrics.mem_used_mb / metrics.mem_total_mb * 100.0
    }
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
