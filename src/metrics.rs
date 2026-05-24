#[derive(Debug, Clone)]
pub struct ProcSample {
    pub epoch: u64,
    pub local_ts: String,
    pub local_hour: String,
    pub cpu_total_ticks: u64,
    pub cpu_idle_ticks: u64,
    pub proc_ticks: u64,
    pub rss_kb: u64,
    pub vm_size_kb: u64,
    pub threads: u64,
    pub read_bytes: Option<u64>,
    pub write_bytes: Option<u64>,
    pub mem_total_kb: u64,
    pub mem_available_kb: u64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub uptime_secs: u64,
}

#[derive(Debug)]
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
    pub read_mb: Option<f64>,
    pub write_mb: Option<f64>,
    pub mem_total_mb: f64,
    pub mem_used_mb: f64,
    pub mem_available_mb: f64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub uptime_secs: u64,
}

#[derive(Debug, Default)]
pub struct Peak {
    pub cpu_percent: f64,
    pub cpu_ts: String,
    pub rss_mb: f64,
    pub rss_ts: String,
    pub system_cpu_percent: f64,
    pub system_cpu_ts: String,
    pub mem_used_mb: f64,
    pub mem_used_ts: String,
}

pub fn calculate(prev: &ProcSample, current: &ProcSample, cpu_count: u64) -> Metrics {
    let proc_delta = current.proc_ticks.saturating_sub(prev.proc_ticks) as f64;
    let total_delta = current.cpu_total_ticks.saturating_sub(prev.cpu_total_ticks) as f64;
    let idle_delta = current.cpu_idle_ticks.saturating_sub(prev.cpu_idle_ticks) as f64;
    let cpu_percent = if total_delta > 0.0 {
        proc_delta / total_delta * cpu_count as f64 * 100.0
    } else {
        0.0
    };
    let cpu_capacity_percent = if cpu_count > 0 {
        cpu_percent / cpu_count as f64
    } else {
        0.0
    };
    let system_cpu_percent = if total_delta > 0.0 {
        (total_delta - idle_delta).max(0.0) / total_delta * 100.0
    } else {
        0.0
    };
    let mem_total_mb = current.mem_total_kb as f64 / 1024.0;
    let mem_available_mb = current.mem_available_kb as f64 / 1024.0;
    let mem_used_mb = (mem_total_mb - mem_available_mb).max(0.0);
    let rss_mb = current.rss_kb as f64 / 1024.0;
    let rss_system_percent = if mem_total_mb > 0.0 {
        rss_mb / mem_total_mb * 100.0
    } else {
        0.0
    };

    Metrics {
        epoch: current.epoch,
        local_ts: current.local_ts.clone(),
        local_hour: current.local_hour.clone(),
        cpu_percent,
        cpu_capacity_percent,
        system_cpu_percent,
        rss_mb,
        rss_system_percent,
        vm_size_mb: current.vm_size_kb as f64 / 1024.0,
        threads: current.threads,
        read_mb: current
            .read_bytes
            .map(|bytes| bytes as f64 / 1024.0 / 1024.0),
        write_mb: current
            .write_bytes
            .map(|bytes| bytes as f64 / 1024.0 / 1024.0),
        mem_total_mb,
        mem_used_mb,
        mem_available_mb,
        load1: current.load1,
        load5: current.load5,
        load15: current.load15,
        uptime_secs: current.uptime_secs,
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
    if metrics.system_cpu_percent > peak.system_cpu_percent {
        peak.system_cpu_percent = metrics.system_cpu_percent;
        peak.system_cpu_ts = metrics.local_ts.clone();
    }
    if metrics.mem_used_mb > peak.mem_used_mb {
        peak.mem_used_mb = metrics.mem_used_mb;
        peak.mem_used_ts = metrics.local_ts.clone();
    }
}
