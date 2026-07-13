use crate::cli::{
    DEFAULT_DB_PATH, DEFAULT_INTERVAL_SECS, DEFAULT_MAX_SAMPLES, DEFAULT_PPROF_INTERVAL_SECS,
    DEFAULT_RETENTION_HOURS, ServiceSpec, WatchArgs, parse_target_spec, validate_service_name,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    database: Option<PathBuf>,
    interval_seconds: Option<u64>,
    pprof_interval_seconds: Option<u64>,
    retention_hours: Option<u64>,
    max_samples: Option<u64>,
    services: Vec<FileService>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileService {
    name: String,
    target: String,
    pprof_url: Option<String>,
}

pub fn load(path: &Path, once: bool) -> Result<WatchArgs, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read config {}: {err}", path.display()))?;
    let config: FileConfig = toml::from_str(&contents)
        .map_err(|err| format!("invalid config {}: {err}", path.display()))?;
    if config.services.is_empty() {
        return Err("config must define at least one [[services]] entry".to_string());
    }
    let interval_seconds = positive_or_default(
        config.interval_seconds,
        DEFAULT_INTERVAL_SECS,
        "interval_seconds",
    )?;
    let pprof_interval_seconds = positive_or_default(
        config.pprof_interval_seconds,
        DEFAULT_PPROF_INTERVAL_SECS,
        "pprof_interval_seconds",
    )?;
    let mut names = HashSet::new();
    let mut services = Vec::with_capacity(config.services.len());
    for service in config.services {
        validate_service_name(&service.name)?;
        if !names.insert(service.name.clone()) {
            return Err(format!("duplicate service name: '{}'", service.name));
        }
        if let Some(url) = service.pprof_url.as_deref()
            && !url.starts_with("http://")
        {
            return Err(format!(
                "pprof_url for '{}' must use local http://",
                service.name
            ));
        }
        services.push(ServiceSpec {
            name: service.name,
            target: parse_target_spec(&service.target)?,
            pprof_url: service.pprof_url,
        });
    }
    Ok(WatchArgs {
        services,
        interval: Duration::from_secs(interval_seconds),
        pprof_interval: Duration::from_secs(pprof_interval_seconds),
        db_path: config
            .database
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DB_PATH)),
        once,
        retention_hours: config.retention_hours.unwrap_or(DEFAULT_RETENTION_HOURS),
        max_samples: config.max_samples.unwrap_or(DEFAULT_MAX_SAMPLES),
        dashboard: true,
    })
}

fn positive_or_default(value: Option<u64>, default: u64, name: &str) -> Result<u64, String> {
    let value = value.unwrap_or(default);
    if value == 0 {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Target;

    #[test]
    fn loads_multi_service_config() {
        let path = std::env::temp_dir().join(format!("gsw-config-{}.toml", std::process::id()));
        fs::write(
            &path,
            r#"
database = "/tmp/gsw.db"
interval_seconds = 10

[[services]]
name = "api"
target = "container:orvix-api"
pprof_url = "http://127.0.0.1:6060"

[[services]]
name = "telephony"
target = "container:orvix-telephony"
"#,
        )
        .unwrap();
        let config = load(&path, false).unwrap();
        let _ = fs::remove_file(path);
        assert_eq!(config.services.len(), 2);
        assert_eq!(config.interval, Duration::from_secs(10));
        assert_eq!(
            config.services[0].target,
            Target::Container("orvix-api".into())
        );
    }
}
