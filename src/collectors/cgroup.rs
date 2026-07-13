use crate::collectors::procfs;
use crate::domain::metrics::ProcessSample;
use std::fs;
use std::path::{Path, PathBuf};

pub fn read_container_sample(pid: i32) -> Result<ProcessSample, String> {
    let process = procfs::read_process_sample(pid)?;
    let directory = cgroup_directory(pid)?;
    let cpu_time_micros = read_keyed_u64(directory.join("cpu.stat"), "usage_usec")?;
    let memory_bytes = read_single_u64(directory.join("memory.current"))?;
    let tasks = read_single_u64(directory.join("pids.current")).unwrap_or(process.threads);
    let (read_bytes, write_bytes) = read_io_stat(directory.join("io.stat"))
        .unwrap_or((process.read_bytes, process.write_bytes));
    Ok(ProcessSample {
        cpu_time_micros: Some(cpu_time_micros),
        proc_ticks: process.proc_ticks,
        rss_kb: memory_bytes / 1024,
        vm_size_kb: process.vm_size_kb,
        threads: tasks,
        file_descriptors: process.file_descriptors,
        read_bytes,
        write_bytes,
    })
}

fn cgroup_directory(pid: i32) -> Result<PathBuf, String> {
    let contents = fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .map_err(|err| format!("failed to read cgroup for process {pid}: {err}"))?;
    let relative = contents
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .ok_or("process is not attached to a cgroup v2 hierarchy")?;
    if relative.split('/').any(|component| component == "..") {
        return Err("invalid cgroup path".to_string());
    }
    Ok(Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/')))
}

fn read_single_u64(path: PathBuf) -> Result<u64, String> {
    fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?
        .trim()
        .parse()
        .map_err(|_| format!("invalid value in {}", path.display()))
}

fn read_keyed_u64(path: PathBuf, key: &str) -> Result<u64, String> {
    let contents = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    parse_keyed_u64(&contents, key).ok_or_else(|| format!("missing {key} in {}", path.display()))
}

fn read_io_stat(path: PathBuf) -> Result<(Option<u64>, Option<u64>), String> {
    let contents = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let mut read_bytes = 0_u64;
    let mut write_bytes = 0_u64;
    let mut found = false;
    for line in contents.lines() {
        for field in line.split_whitespace().skip(1) {
            if let Some(value) = field.strip_prefix("rbytes=") {
                read_bytes = read_bytes.saturating_add(value.parse().unwrap_or(0));
                found = true;
            } else if let Some(value) = field.strip_prefix("wbytes=") {
                write_bytes = write_bytes.saturating_add(value.parse().unwrap_or(0));
                found = true;
            }
        }
    }
    if found {
        Ok((Some(read_bytes), Some(write_bytes)))
    } else {
        Ok((None, None))
    }
}

fn parse_keyed_u64(contents: &str, key: &str) -> Option<u64> {
    contents.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        (fields.next()? == key)
            .then(|| fields.next()?.parse().ok())
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keyed_cgroup_stat() {
        assert_eq!(
            parse_keyed_u64("usage_usec 12345\nuser_usec 10000\n", "usage_usec"),
            Some(12345)
        );
    }
}
