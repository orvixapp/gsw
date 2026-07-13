use crate::domain::metrics::{HostSample, ProcessSample};
use crate::platform::time;
use std::fs;

pub fn read_host_sample() -> Result<HostSample, String> {
    let epoch = time::epoch_seconds()?;
    let (local_ts, local_hour) = time::local_time_strings(epoch);
    let (cpu_total_ticks, cpu_idle_ticks) = read_cpu_ticks()?;
    let (mem_total_kb, mem_available_kb) = read_meminfo()?;
    let (load1, load5, load15) = read_loadavg()?;
    let uptime_secs = read_uptime_secs()?;
    Ok(HostSample {
        epoch,
        local_ts,
        local_hour,
        cpu_total_ticks,
        cpu_idle_ticks,
        mem_total_kb,
        mem_available_kb,
        load1,
        load5,
        load15,
        uptime_secs,
    })
}

pub fn read_process_sample(pid: i32) -> Result<ProcessSample, String> {
    let proc_ticks = read_proc_ticks(pid)?;
    let (rss_kb, vm_size_kb, threads) = read_proc_status(pid)?;
    let (read_bytes, write_bytes) = read_proc_io(pid).unwrap_or((None, None));
    let file_descriptors = read_file_descriptor_count(pid).unwrap_or(0);
    Ok(ProcessSample {
        cpu_time_micros: None,
        proc_ticks,
        rss_kb,
        vm_size_kb,
        threads,
        file_descriptors,
        read_bytes,
        write_bytes,
    })
}

pub fn cpu_count() -> Result<u64, String> {
    let stat = fs::read_to_string("/proc/stat")
        .map_err(|err| format!("failed to read /proc/stat: {err}"))?;
    Ok(stat
        .lines()
        .filter(|line| {
            let name = line.split_whitespace().next().unwrap_or_default();
            name.len() > 3
                && name.starts_with("cpu")
                && name[3..].chars().all(|ch| ch.is_ascii_digit())
        })
        .count() as u64)
}

fn read_cpu_ticks() -> Result<(u64, u64), String> {
    let stat = fs::read_to_string("/proc/stat")
        .map_err(|err| format!("failed to read /proc/stat: {err}"))?;
    let line = stat
        .lines()
        .find(|line| line.starts_with("cpu "))
        .ok_or("missing aggregate CPU line in /proc/stat")?;
    let values = line
        .split_whitespace()
        .skip(1)
        .map(|value| value.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    let total = values.iter().sum();
    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    Ok((total, idle))
}

fn read_proc_ticks(pid: i32) -> Result<u64, String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|err| format!("failed to read process {pid}: {err}"))?;
    let end = stat
        .rfind(')')
        .ok_or_else(|| format!("invalid /proc/{pid}/stat"))?;
    let fields = stat[end + 1..].split_whitespace().collect::<Vec<_>>();
    let utime = parse_field(&fields, 11, "utime")?;
    let stime = parse_field(&fields, 12, "stime")?;
    Ok(utime.saturating_add(stime))
}

fn read_proc_status(pid: i32) -> Result<(u64, u64, u64), String> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .map_err(|err| format!("failed to read process status {pid}: {err}"))?;
    let mut rss = 0;
    let mut vm_size = 0;
    let mut threads = 0;
    for line in status.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            rss = parse_first_u64(value);
        } else if let Some(value) = line.strip_prefix("VmSize:") {
            vm_size = parse_first_u64(value);
        } else if let Some(value) = line.strip_prefix("Threads:") {
            threads = parse_first_u64(value);
        }
    }
    Ok((rss, vm_size, threads))
}

fn read_proc_io(pid: i32) -> Result<(Option<u64>, Option<u64>), String> {
    let io = fs::read_to_string(format!("/proc/{pid}/io")).map_err(|err| err.to_string())?;
    let mut read_bytes = None;
    let mut write_bytes = None;
    for line in io.lines() {
        if let Some(value) = line.strip_prefix("read_bytes:") {
            read_bytes = Some(parse_first_u64(value));
        } else if let Some(value) = line.strip_prefix("write_bytes:") {
            write_bytes = Some(parse_first_u64(value));
        }
    }
    Ok((read_bytes, write_bytes))
}

fn read_file_descriptor_count(pid: i32) -> Result<u64, String> {
    Ok(fs::read_dir(format!("/proc/{pid}/fd"))
        .map_err(|err| err.to_string())?
        .count() as u64)
}

fn read_meminfo() -> Result<(u64, u64), String> {
    let meminfo = fs::read_to_string("/proc/meminfo")
        .map_err(|err| format!("failed to read /proc/meminfo: {err}"))?;
    let mut total = 0;
    let mut available = 0;
    for line in meminfo.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            total = parse_first_u64(value);
        } else if let Some(value) = line.strip_prefix("MemAvailable:") {
            available = parse_first_u64(value);
        }
    }
    Ok((total, available))
}

fn read_loadavg() -> Result<(f64, f64, f64), String> {
    let load = fs::read_to_string("/proc/loadavg")
        .map_err(|err| format!("failed to read /proc/loadavg: {err}"))?;
    let mut values = load.split_whitespace();
    Ok((
        values.next().unwrap_or("0").parse().unwrap_or(0.0),
        values.next().unwrap_or("0").parse().unwrap_or(0.0),
        values.next().unwrap_or("0").parse().unwrap_or(0.0),
    ))
}

fn read_uptime_secs() -> Result<u64, String> {
    let uptime = fs::read_to_string("/proc/uptime")
        .map_err(|err| format!("failed to read /proc/uptime: {err}"))?;
    Ok(uptime
        .split_whitespace()
        .next()
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(0.0) as u64)
}

fn parse_field(fields: &[&str], index: usize, name: &str) -> Result<u64, String> {
    fields
        .get(index)
        .ok_or_else(|| format!("missing {name} in process stat"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid {name} in process stat"))
}

fn parse_first_u64(value: &str) -> u64 {
    value
        .split_whitespace()
        .next()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0)
}
