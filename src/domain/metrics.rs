#[derive(Debug, Clone)]
pub struct HostSample {
    pub epoch: u64,
    pub local_ts: String,
    pub local_hour: String,
    pub cpu_total_ticks: u64,
    pub cpu_idle_ticks: u64,
    pub mem_total_kb: u64,
    pub mem_available_kb: u64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessSample {
    pub cpu_time_micros: Option<u64>,
    pub proc_ticks: u64,
    pub rss_kb: u64,
    pub vm_size_kb: u64,
    pub threads: u64,
    pub file_descriptors: u64,
    pub read_bytes: Option<u64>,
    pub write_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    pub epoch: u64,
    pub local_ts: String,
    pub local_hour: String,
    pub cpu_percent: f64,
    pub cpu_capacity_percent: f64,
    pub system_cpu_percent: f64,
    pub rss_mb: f64,
    pub rss_system_percent: f64,
    pub vm_size_mb: f64,
    pub threads: u64,
    pub file_descriptors: u64,
    pub read_mb: Option<f64>,
    pub write_mb: Option<f64>,
    pub read_mb_per_sec: Option<f64>,
    pub write_mb_per_sec: Option<f64>,
    pub goroutines: Option<u64>,
    pub goroutine_growth_per_min: Option<f64>,
    pub mem_total_mb: f64,
    pub mem_used_mb: f64,
    pub mem_available_mb: f64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Peak {
    pub cpu_percent: f64,
    pub cpu_ts: String,
    pub rss_mb: f64,
    pub rss_ts: String,
    pub goroutines: u64,
    pub goroutines_ts: String,
}

pub fn calculate(
    previous_host: &HostSample,
    current_host: &HostSample,
    previous_process: &ProcessSample,
    current_process: &ProcessSample,
    cpu_count: u64,
) -> Metrics {
    let proc_delta = current_process
        .proc_ticks
        .saturating_sub(previous_process.proc_ticks) as f64;
    let total_delta = current_host
        .cpu_total_ticks
        .saturating_sub(previous_host.cpu_total_ticks) as f64;
    let idle_delta = current_host
        .cpu_idle_ticks
        .saturating_sub(previous_host.cpu_idle_ticks) as f64;
    let elapsed_secs = current_host
        .epoch
        .saturating_sub(previous_host.epoch)
        .max(1) as f64;
    let cpu_percent = match (
        previous_process.cpu_time_micros,
        current_process.cpu_time_micros,
    ) {
        (Some(previous), Some(current)) => {
            current.saturating_sub(previous) as f64 / (elapsed_secs * 1_000_000.0) * 100.0
        }
        _ if total_delta > 0.0 => proc_delta / total_delta * cpu_count.max(1) as f64 * 100.0,
        _ => 0.0,
    };
    let cpu_capacity_percent = cpu_percent / cpu_count.max(1) as f64;
    let system_cpu_percent = if total_delta > 0.0 {
        (total_delta - idle_delta).max(0.0) / total_delta * 100.0
    } else {
        0.0
    };
    let mem_total_mb = current_host.mem_total_kb as f64 / 1024.0;
    let mem_available_mb = current_host.mem_available_kb as f64 / 1024.0;
    let mem_used_mb = (mem_total_mb - mem_available_mb).max(0.0);
    let rss_mb = current_process.rss_kb as f64 / 1024.0;
    let rss_system_percent = if mem_total_mb > 0.0 {
        rss_mb / mem_total_mb * 100.0
    } else {
        0.0
    };

    Metrics {
        epoch: current_host.epoch,
        local_ts: current_host.local_ts.clone(),
        local_hour: current_host.local_hour.clone(),
        cpu_percent,
        cpu_capacity_percent,
        system_cpu_percent,
        rss_mb,
        rss_system_percent,
        vm_size_mb: current_process.vm_size_kb as f64 / 1024.0,
        threads: current_process.threads,
        file_descriptors: current_process.file_descriptors,
        read_mb: bytes_to_mb(current_process.read_bytes),
        write_mb: bytes_to_mb(current_process.write_bytes),
        read_mb_per_sec: byte_rate(
            previous_process.read_bytes,
            current_process.read_bytes,
            elapsed_secs,
        ),
        write_mb_per_sec: byte_rate(
            previous_process.write_bytes,
            current_process.write_bytes,
            elapsed_secs,
        ),
        goroutines: None,
        goroutine_growth_per_min: None,
        mem_total_mb,
        mem_used_mb,
        mem_available_mb,
        load1: current_host.load1,
        load5: current_host.load5,
        load15: current_host.load15,
        uptime_secs: current_host.uptime_secs,
    }
}

pub fn update_peak(peak: &mut Peak, metrics: &Metrics) {
    if metrics.cpu_percent > peak.cpu_percent {
        peak.cpu_percent = metrics.cpu_percent;
        peak.cpu_ts = metrics.local_ts.clone();
    }
    if metrics.rss_mb > peak.rss_mb {
        peak.rss_mb = metrics.rss_mb;
        peak.rss_ts = metrics.local_ts.clone();
    }
    if let Some(goroutines) = metrics.goroutines
        && goroutines > peak.goroutines
    {
        peak.goroutines = goroutines;
        peak.goroutines_ts = metrics.local_ts.clone();
    }
}

fn bytes_to_mb(bytes: Option<u64>) -> Option<f64> {
    bytes.map(|value| value as f64 / 1024.0 / 1024.0)
}

fn byte_rate(previous: Option<u64>, current: Option<u64>, elapsed_secs: f64) -> Option<f64> {
    let delta = current?.checked_sub(previous?)?;
    Some(delta as f64 / 1024.0 / 1024.0 / elapsed_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(epoch: u64, total: u64, idle: u64) -> HostSample {
        HostSample {
            epoch,
            local_ts: "2026-01-01 00:00:00".into(),
            local_hour: "2026-01-01 00".into(),
            cpu_total_ticks: total,
            cpu_idle_ticks: idle,
            mem_total_kb: 1024 * 1024,
            mem_available_kb: 512 * 1024,
            load1: 0.1,
            load5: 0.2,
            load15: 0.3,
            uptime_secs: epoch,
        }
    }

    fn process(ticks: u64, read_bytes: u64) -> ProcessSample {
        ProcessSample {
            cpu_time_micros: None,
            proc_ticks: ticks,
            rss_kb: 128 * 1024,
            vm_size_kb: 256 * 1024,
            threads: 8,
            file_descriptors: 32,
            read_bytes: Some(read_bytes),
            write_bytes: Some(0),
        }
    }

    #[test]
    fn calculates_process_cpu_and_io_rate() {
        let metrics = calculate(
            &host(10, 1000, 500),
            &host(12, 1200, 550),
            &process(100, 0),
            &process(150, 2 * 1024 * 1024),
            2,
        );
        assert!((metrics.cpu_percent - 50.0).abs() < 0.001);
        assert_eq!(metrics.read_mb_per_sec, Some(1.0));
        assert_eq!(metrics.file_descriptors, 32);
    }
}
