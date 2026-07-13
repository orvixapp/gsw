#[derive(Debug)]
pub struct HourSummary {
    pub service_name: String,
    pub local_hour: String,
    pub samples: i64,
    pub cpu_avg: f64,
    pub cpu_peak: f64,
    pub rss_avg: f64,
    pub rss_peak: f64,
    pub goroutines_avg: Option<f64>,
    pub goroutines_peak: Option<i64>,
    pub file_descriptors_peak: i64,
}

pub fn print_hourly(rows: &[HourSummary]) -> Result<(), String> {
    if rows.is_empty() {
        return Err("the SQLite database contains no matching samples".to_string());
    }
    println!(
        "SERVICE            LOCAL HOUR       SAMPLES  CPU AVG  CPU PEAK  RSS AVG  RSS PEAK  GOR AVG  GOR PEAK  FD PEAK"
    );
    println!("{}", "─".repeat(112));
    for row in rows {
        println!(
            "{:<18} {:<16} {:>7} {:>8.2}% {:>9.2}% {:>8.2} {:>9.2} {:>8} {:>9} {:>8}",
            truncate(&row.service_name, 18),
            row.local_hour,
            row.samples,
            row.cpu_avg,
            row.cpu_peak,
            row.rss_avg,
            row.rss_peak,
            display_optional_f64(row.goroutines_avg),
            display_optional_i64(row.goroutines_peak),
            row.file_descriptors_peak,
        );
    }
    Ok(())
}

fn display_optional_f64(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "-".into())
}

fn display_optional_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".into())
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}
