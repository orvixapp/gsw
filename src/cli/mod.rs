use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

pub const DEFAULT_DB_PATH: &str = "server-metrics.db";
pub const DEFAULT_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_PPROF_INTERVAL_SECS: u64 = 30;
pub const DEFAULT_RETENTION_HOURS: u64 = 168;
pub const DEFAULT_MAX_SAMPLES: u64 = 150_000;

#[derive(Debug, PartialEq)]
pub enum Command {
    Watch(WatchArgs),
    Agent(WatchArgs),
    Summary(SummaryArgs),
    Help,
}

#[derive(Debug, PartialEq)]
pub struct WatchArgs {
    pub services: Vec<ServiceSpec>,
    pub interval: Duration,
    pub pprof_interval: Duration,
    pub db_path: PathBuf,
    pub once: bool,
    pub retention_hours: u64,
    pub max_samples: u64,
    pub dashboard: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceSpec {
    pub name: String,
    pub target: Target,
    pub pprof_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Pid(i32),
    Name(String),
    Container(String),
    Launch(Vec<String>),
}

impl Target {
    pub fn description(&self) -> String {
        match self {
            Self::Pid(pid) => format!("pid:{pid}"),
            Self::Name(name) => format!("name:{name}"),
            Self::Container(name) => format!("container:{name}"),
            Self::Launch(command) => format!("launch:{}", command.join(" ")),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SummaryArgs {
    pub db_path: PathBuf,
    pub service: Option<String>,
}

pub fn parse(raw: Vec<String>) -> Result<Command, String> {
    match raw.first().map(String::as_str) {
        None | Some("-h") | Some("--help") | Some("help") => Ok(Command::Help),
        Some("watch") => parse_watch(&raw[1..]).map(Command::Watch),
        Some("agent") => parse_agent(&raw[1..]).map(Command::Agent),
        Some("summary") => parse_summary(&raw[1..]).map(Command::Summary),
        Some(legacy @ "--pid") | Some(legacy @ "--name") => {
            let mut args = vec![legacy.to_string()];
            args.extend_from_slice(&raw[1..]);
            parse_watch(&args).map(Command::Watch)
        }
        Some(other) => Err(format!("unknown command: {other}")),
    }
}

fn parse_watch(raw: &[String]) -> Result<WatchArgs, String> {
    if raw.iter().any(|value| value == "--config") {
        return parse_config_watch(raw);
    }
    let mut pid = None;
    let mut name = None;
    let mut container = None;
    let mut launch = None;
    let mut label = None;
    let mut services = Vec::new();
    let mut pprof_urls = HashMap::new();
    let mut interval = Duration::from_secs(DEFAULT_INTERVAL_SECS);
    let mut pprof_interval = Duration::from_secs(DEFAULT_PPROF_INTERVAL_SECS);
    let mut db_path = PathBuf::from(DEFAULT_DB_PATH);
    let mut once = false;
    let mut retention_hours = DEFAULT_RETENTION_HOURS;
    let mut max_samples = DEFAULT_MAX_SAMPLES;

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--service" => {
                i += 1;
                services.push(parse_service(
                    raw.get(i).ok_or("--service requires NAME=TYPE:VALUE")?,
                )?);
            }
            "--pprof" => {
                i += 1;
                let value = raw.get(i).ok_or("--pprof requires NAME=URL")?;
                let (service, url) = value.split_once('=').ok_or("--pprof requires NAME=URL")?;
                validate_service_name(service)?;
                if !url.starts_with("http://") {
                    return Err("--pprof currently supports local http:// URLs only".to_string());
                }
                if pprof_urls
                    .insert(service.to_string(), url.to_string())
                    .is_some()
                {
                    return Err(format!("duplicate --pprof configuration for '{service}'"));
                }
            }
            "--pid" => {
                i += 1;
                let value = raw.get(i).ok_or("--pid requires a value")?;
                pid = Some(value.parse::<i32>().map_err(|_| "--pid must be numeric")?);
            }
            "--name" => {
                i += 1;
                name = Some(raw.get(i).ok_or("--name requires a value")?.clone());
            }
            "--container" => {
                i += 1;
                container = Some(raw.get(i).ok_or("--container requires a name")?.clone());
            }
            "--label" => {
                i += 1;
                let value = raw.get(i).ok_or("--label requires a value")?;
                validate_service_name(value)?;
                label = Some(value.clone());
            }
            "--interval" => {
                i += 1;
                interval = Duration::from_secs(parse_positive_u64(
                    raw.get(i).ok_or("--interval requires seconds")?,
                    "--interval",
                )?);
            }
            "--pprof-interval" => {
                i += 1;
                pprof_interval = Duration::from_secs(parse_positive_u64(
                    raw.get(i).ok_or("--pprof-interval requires seconds")?,
                    "--pprof-interval",
                )?);
            }
            "--db" => {
                i += 1;
                db_path = PathBuf::from(raw.get(i).ok_or("--db requires a path")?);
            }
            "--once" => once = true,
            "--retention-hours" => {
                i += 1;
                retention_hours = parse_u64(
                    raw.get(i).ok_or("--retention-hours requires hours")?,
                    "--retention-hours",
                )?;
            }
            "--max-samples" => {
                i += 1;
                max_samples = parse_u64(
                    raw.get(i).ok_or("--max-samples requires a count")?,
                    "--max-samples",
                )?;
            }
            "--" => {
                let command = raw[i + 1..].to_vec();
                if command.is_empty() {
                    return Err("a command is required after --".to_string());
                }
                launch = Some(command);
                break;
            }
            "-h" | "--help" => return Ok(help_watch()),
            unknown => return Err(format!("unknown watch argument: {unknown}")),
        }
        i += 1;
    }

    let legacy_count = pid.is_some() as u8
        + name.is_some() as u8
        + container.is_some() as u8
        + launch.is_some() as u8;
    if !services.is_empty() && legacy_count > 0 {
        return Err(
            "--service cannot be combined with --pid, --name, --container, or --".to_string(),
        );
    }

    if services.is_empty() {
        if legacy_count != 1 {
            return Err(
                "watch requires --service or exactly one legacy target: --pid, --name, --container, or -- <command>"
                    .to_string(),
            );
        }
        let target = if let Some(pid) = pid {
            Target::Pid(pid)
        } else if let Some(name) = name {
            Target::Name(name)
        } else if let Some(container) = container {
            Target::Container(container)
        } else {
            Target::Launch(launch.expect("legacy target count checked"))
        };
        services.push(ServiceSpec {
            name: label.unwrap_or_else(|| default_service_name(&target)),
            target,
            pprof_url: None,
        });
    } else if label.is_some() {
        return Err("--label is only valid with a legacy single target".to_string());
    }

    let mut names = HashSet::new();
    for service in &mut services {
        if !names.insert(service.name.clone()) {
            return Err(format!("duplicate service name: '{}'", service.name));
        }
        service.pprof_url = pprof_urls.remove(&service.name);
    }
    if let Some((unknown, _)) = pprof_urls.into_iter().next() {
        return Err(format!("--pprof references unknown service '{unknown}'"));
    }

    Ok(WatchArgs {
        services,
        interval,
        pprof_interval,
        db_path,
        once,
        retention_hours,
        max_samples,
        dashboard: true,
    })
}

fn parse_agent(raw: &[String]) -> Result<WatchArgs, String> {
    if !raw.iter().any(|value| value == "--config") {
        return Err("agent requires --config PATH".to_string());
    }
    let mut args = parse_config_watch(raw)?;
    args.dashboard = false;
    Ok(args)
}

fn parse_service(value: &str) -> Result<ServiceSpec, String> {
    let (name, target) = value
        .split_once('=')
        .ok_or("--service requires NAME=TYPE:VALUE")?;
    validate_service_name(name)?;
    let target = parse_target_spec(target)?;
    Ok(ServiceSpec {
        name: name.to_string(),
        target,
        pprof_url: None,
    })
}

pub(crate) fn parse_target_spec(target: &str) -> Result<Target, String> {
    let (kind, target_value) = target
        .split_once(':')
        .ok_or("service target requires TYPE:VALUE")?;
    if target_value.is_empty() {
        return Err("--service target value cannot be empty".to_string());
    }
    Ok(match kind {
        "pid" => Target::Pid(
            target_value
                .parse::<i32>()
                .map_err(|_| "service pid must be numeric")?,
        ),
        "name" => Target::Name(target_value.to_string()),
        "container" => Target::Container(target_value.to_string()),
        other => return Err(format!("unsupported service target type: {other}")),
    })
}

pub(crate) fn validate_service_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(format!(
            "invalid service name '{name}'; use letters, numbers, '.', '_' or '-'"
        ));
    }
    Ok(())
}

fn parse_config_watch(raw: &[String]) -> Result<WatchArgs, String> {
    let mut path = None;
    let mut once = false;
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--config" => {
                i += 1;
                if path.is_some() {
                    return Err("--config can only be specified once".to_string());
                }
                path = Some(PathBuf::from(raw.get(i).ok_or("--config requires a path")?));
            }
            "--once" => once = true,
            "-h" | "--help" => return Ok(help_watch()),
            other => {
                return Err(format!(
                    "{other} cannot be combined with --config; put the setting in the TOML file"
                ));
            }
        }
        i += 1;
    }
    crate::config::load(&path.ok_or("--config requires a path")?, once)
}

fn default_service_name(target: &Target) -> String {
    match target {
        Target::Pid(pid) => format!("pid-{pid}"),
        Target::Name(name) | Target::Container(name) => name.clone(),
        Target::Launch(command) => command
            .first()
            .and_then(|value| std::path::Path::new(value).file_name())
            .and_then(|value| value.to_str())
            .unwrap_or("process")
            .to_string(),
    }
}

fn parse_positive_u64(value: &str, argument: &str) -> Result<u64, String> {
    let number = parse_u64(value, argument)?;
    if number == 0 {
        return Err(format!("{argument} must be greater than zero"));
    }
    Ok(number)
}

fn parse_u64(value: &str, argument: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("{argument} must be numeric"))
}

fn help_watch() -> WatchArgs {
    print_help();
    std::process::exit(0);
}

fn parse_summary(raw: &[String]) -> Result<SummaryArgs, String> {
    let mut db_path = PathBuf::from(DEFAULT_DB_PATH);
    let mut service = None;
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--db" => {
                i += 1;
                db_path = PathBuf::from(raw.get(i).ok_or("--db requires a path")?);
            }
            "--service" => {
                i += 1;
                let value = raw.get(i).ok_or("--service requires a name")?;
                validate_service_name(value)?;
                service = Some(value.clone());
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            unknown => return Err(format!("unknown summary argument: {unknown}")),
        }
        i += 1;
    }
    Ok(SummaryArgs { db_path, service })
}

pub fn print_help() {
    println!(
        "gsw - lightweight Linux service monitor

USAGE:
  gsw agent --config /etc/gsw/config.toml
  gsw watch --config /etc/gsw/config.toml
  gsw watch --service api=container:api --service worker=container:worker [OPTIONS]
  gsw watch --pid <PID> [--label api] [OPTIONS]
  gsw watch --name <TEXT> [--label api] [OPTIONS]
  gsw watch --container <NAME> [--label api] [OPTIONS]
  gsw watch [OPTIONS] -- ./server --flag value
  gsw summary [--service NAME] [--db server-metrics.db]

WATCH OPTIONS:
  --config PATH               Load all watch settings from a TOML file
  --service NAME=TYPE:VALUE   Repeatable target; TYPE is pid, name, or container
  --pprof NAME=URL            Optional Go pprof endpoint for a named service
  --interval SECONDS          Process and host sampling interval (default: 5)
  --pprof-interval SECONDS    Go runtime sampling interval (default: 30)
  --db PATH                   SQLite database path
  --retention-hours HOURS     Detailed sample retention (default: 168)
  --max-samples COUNT         Maximum samples retained per service
  --once                      Collect one calculated sample and exit

gsw opens no listening ports. A pprof URL should be bound to loopback or a
private Unix-to-HTTP bridge. CPU follows top semantics: 100% is one full core."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_services_and_pprof() {
        let command = parse(vec![
            "watch".into(),
            "--service".into(),
            "api=container:orvix-api".into(),
            "--service".into(),
            "telephony=name:orvix-telephony".into(),
            "--pprof".into(),
            "api=http://127.0.0.1:6060".into(),
        ])
        .unwrap();
        let Command::Watch(args) = command else {
            panic!("expected watch command");
        };
        assert_eq!(args.services.len(), 2);
        assert_eq!(args.services[0].name, "api");
        assert_eq!(
            args.services[0].pprof_url.as_deref(),
            Some("http://127.0.0.1:6060")
        );
    }

    #[test]
    fn preserves_legacy_pid_usage() {
        let command = parse(vec!["watch".into(), "--pid".into(), "42".into()]).unwrap();
        let Command::Watch(args) = command else {
            panic!("expected watch command");
        };
        assert_eq!(args.services[0].name, "pid-42");
        assert_eq!(args.services[0].target, Target::Pid(42));
    }

    #[test]
    fn rejects_duplicate_service_names() {
        let error = parse(vec![
            "watch".into(),
            "--service".into(),
            "api=pid:1".into(),
            "--service".into(),
            "api=pid:2".into(),
        ])
        .unwrap_err();
        assert!(error.contains("duplicate service name"));
    }
}
