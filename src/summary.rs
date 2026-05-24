#[derive(Debug)]
pub struct HourSummary {
    pub local_hour: String,
    pub samples: i64,
    pub cpu_avg: f64,
    pub cpu_peak: f64,
    pub system_cpu_avg: f64,
    pub system_cpu_peak: f64,
    pub rss_peak: f64,
    pub system_mem_peak: f64,
}

pub fn print_hourly(rows: &[HourSummary]) -> Result<(), String> {
    if rows.is_empty() {
        return Err("la base SQLite no tiene muestras validas".to_string());
    }

    println!(
        "Hora local         muestras  CPU proc  Pico proc  CPU sist  Pico sist  RAM proc  RAM sist"
    );
    println!(
        "-----------------------------------------------------------------------------------------"
    );
    for row in rows {
        println!(
            "{:<16} {:>8} {:>8.2}% {:>9.2}% {:>8.2}% {:>9.2}% {:>8.2} {:>8.2}",
            row.local_hour,
            row.samples,
            row.cpu_avg,
            row.cpu_peak,
            row.system_cpu_avg,
            row.system_cpu_peak,
            row.rss_peak,
            row.system_mem_peak
        );
    }

    Ok(())
}
