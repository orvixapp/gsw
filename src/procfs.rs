use crate::metrics::ProcSample;
use crate::time;
use std::fs;

pub fn read_sample(pid: i32) -> Result<ProcSample, String> {
    let epoch = time::epoch_seconds()?;
    let (local_ts, local_hour) = time::local_time_strings(epoch);
    let (cpu_total_ticks, cpu_idle_ticks) = read_cpu_ticks()?;
    let proc_ticks = read_proc_ticks(pid)?;
    let (rss_kb, vm_size_kb, threads) = read_proc_status(pid)?;
    let (read_bytes, write_bytes) = read_proc_io(pid).unwrap_or((None, None));
    let (mem_total_kb, mem_available_kb) = read_meminfo()?;
    let (load1, load5, load15) = read_loadavg()?;
    let uptime_secs = read_uptime_secs()?;

    Ok(ProcSample {
        epoch,
        local_ts,
        local_hour,
        cpu_total_ticks,
        cpu_idle_ticks,
        proc_ticks,
        rss_kb,
        vm_size_kb,
        threads,
        read_bytes,
        write_bytes,
        mem_total_kb,
        mem_available_kb,
        load1,
        load5,
        load15,
        uptime_secs,
    })
}

pub fn cpu_count() -> Result<u64, String> {
    let stat = fs::read_to_string("/proc/stat")
        .map_err(|err| format!("no pude leer /proc/stat: {err}"))?;
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
        .map_err(|err| format!("no pude leer /proc/stat: {err}"))?;
    let line = stat
        .lines()
        .next()
        .ok_or("no pude leer la primera linea de /proc/stat")?;

    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .map(|part| {
            part.parse::<u64>()
                .map_err(|_| "formato invalido en /proc/stat".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let total = fields.iter().sum();
    let idle = fields.get(3).copied().unwrap_or(0) + fields.get(4).copied().unwrap_or(0);

    Ok((total, idle))
}

fn read_proc_ticks(pid: i32) -> Result<u64, String> {
    let path = format!("/proc/{pid}/stat");
    let stat = fs::read_to_string(&path).map_err(|err| format!("no pude leer {path}: {err}"))?;
    let close_paren = stat
        .rfind(')')
        .ok_or_else(|| format!("formato invalido en {path}"))?;
    let fields: Vec<&str> = stat[close_paren + 1..].split_whitespace().collect();
    let utime = parse_stat_field(&fields, 11, "utime")?;
    let stime = parse_stat_field(&fields, 12, "stime")?;
    Ok(utime + stime)
}

fn parse_stat_field(fields: &[&str], index: usize, name: &str) -> Result<u64, String> {
    fields
        .get(index)
        .ok_or_else(|| format!("no encontre {name} en /proc/<pid>/stat"))?
        .parse::<u64>()
        .map_err(|_| format!("{name} invalido en /proc/<pid>/stat"))
}

fn read_proc_status(pid: i32) -> Result<(u64, u64, u64), String> {
    let path = format!("/proc/{pid}/status");
    let status = fs::read_to_string(&path).map_err(|err| format!("no pude leer {path}: {err}"))?;
    let mut rss_kb = 0;
    let mut vm_size_kb = 0;
    let mut threads = 0;

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            rss_kb = parse_status_kb(value)?;
        } else if let Some(value) = line.strip_prefix("VmSize:") {
            vm_size_kb = parse_status_kb(value)?;
        } else if let Some(value) = line.strip_prefix("Threads:") {
            threads = value
                .trim()
                .parse::<u64>()
                .map_err(|_| "Threads invalido en /proc/<pid>/status".to_string())?;
        }
    }

    Ok((rss_kb, vm_size_kb, threads))
}

fn parse_status_kb(value: &str) -> Result<u64, String> {
    value
        .split_whitespace()
        .next()
        .ok_or("valor kB vacio en /proc/<pid>/status")?
        .parse::<u64>()
        .map_err(|_| "valor kB invalido en /proc/<pid>/status".to_string())
}

fn read_proc_io(pid: i32) -> Result<(Option<u64>, Option<u64>), String> {
    let path = format!("/proc/{pid}/io");
    let io = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let mut read_bytes = None;
    let mut write_bytes = None;

    for line in io.lines() {
        if let Some(value) = line.strip_prefix("read_bytes:") {
            read_bytes = value.trim().parse::<u64>().ok();
        } else if let Some(value) = line.strip_prefix("write_bytes:") {
            write_bytes = value.trim().parse::<u64>().ok();
        }
    }

    Ok((read_bytes, write_bytes))
}

fn read_meminfo() -> Result<(u64, u64), String> {
    let meminfo = fs::read_to_string("/proc/meminfo")
        .map_err(|err| format!("no pude leer /proc/meminfo: {err}"))?;
    let mut total = None;
    let mut available = None;

    for line in meminfo.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            total = Some(parse_status_kb(value)?);
        } else if let Some(value) = line.strip_prefix("MemAvailable:") {
            available = Some(parse_status_kb(value)?);
        }
    }

    Ok((
        total.ok_or("MemTotal no encontrado en /proc/meminfo")?,
        available.ok_or("MemAvailable no encontrado en /proc/meminfo")?,
    ))
}

fn read_loadavg() -> Result<(f64, f64, f64), String> {
    let loadavg = fs::read_to_string("/proc/loadavg")
        .map_err(|err| format!("no pude leer /proc/loadavg: {err}"))?;
    let mut parts = loadavg.split_whitespace();
    let load1 = parse_f64(parts.next(), "load1")?;
    let load5 = parse_f64(parts.next(), "load5")?;
    let load15 = parse_f64(parts.next(), "load15")?;
    Ok((load1, load5, load15))
}

fn read_uptime_secs() -> Result<u64, String> {
    let uptime = fs::read_to_string("/proc/uptime")
        .map_err(|err| format!("no pude leer /proc/uptime: {err}"))?;
    let first = uptime
        .split_whitespace()
        .next()
        .ok_or("uptime vacio en /proc/uptime")?;
    first
        .parse::<f64>()
        .map(|secs| secs as u64)
        .map_err(|_| "uptime invalido en /proc/uptime".to_string())
}

fn parse_f64(value: Option<&str>, name: &str) -> Result<f64, String> {
    value
        .ok_or_else(|| format!("{name} no encontrado"))?
        .parse::<f64>()
        .map_err(|_| format!("{name} invalido"))
}
