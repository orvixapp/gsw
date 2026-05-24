use std::path::PathBuf;
use std::time::Duration;

pub const DEFAULT_DB_PATH: &str = "server-metrics.db";
pub const DEFAULT_INTERVAL_SECS: u64 = 2;
pub const DEFAULT_RETENTION_HOURS: u64 = 72;
pub const DEFAULT_MAX_SAMPLES: u64 = 150_000;

#[derive(Debug)]
pub enum Command {
    Watch(WatchArgs),
    Summary(SummaryArgs),
    Help,
}

#[derive(Debug)]
pub struct WatchArgs {
    pub target: Target,
    pub interval: Duration,
    pub db_path: PathBuf,
    pub once: bool,
    pub retention_hours: u64,
    pub max_samples: u64,
}

#[derive(Debug)]
pub enum Target {
    Pid(i32),
    Name(String),
    Container(String),
    Launch(Vec<String>),
}

#[derive(Debug)]
pub struct SummaryArgs {
    pub db_path: PathBuf,
}

pub fn parse(raw: Vec<String>) -> Result<Command, String> {
    match raw.first().map(|s| s.as_str()) {
        None | Some("-h") | Some("--help") | Some("help") => Ok(Command::Help),
        Some("watch") => parse_watch(&raw[1..]).map(Command::Watch),
        Some("summary") => parse_summary(&raw[1..]).map(Command::Summary),
        Some(legacy @ "--pid") | Some(legacy @ "--name") => {
            let mut args = vec!["watch".to_string(), legacy.to_string()];
            args.extend_from_slice(&raw[1..]);
            parse(args).map(|command| match command {
                Command::Watch(watch) => Command::Watch(watch),
                other => other,
            })
        }
        Some(other) => Err(format!("comando no reconocido: {other}")),
    }
}

fn parse_watch(raw: &[String]) -> Result<WatchArgs, String> {
    let mut pid = None;
    let mut name = None;
    let mut container = None;
    let mut launch = None;
    let mut interval = Duration::from_secs(DEFAULT_INTERVAL_SECS);
    let mut db_path = PathBuf::from(DEFAULT_DB_PATH);
    let mut once = false;
    let mut retention_hours = DEFAULT_RETENTION_HOURS;
    let mut max_samples = DEFAULT_MAX_SAMPLES;

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--pid" => {
                i += 1;
                let value = raw.get(i).ok_or("--pid necesita un valor")?;
                pid = Some(
                    value
                        .parse::<i32>()
                        .map_err(|_| "--pid debe ser numerico")?,
                );
            }
            "--name" => {
                i += 1;
                name = Some(raw.get(i).ok_or("--name necesita un valor")?.clone());
            }
            "--container" => {
                i += 1;
                container = Some(raw.get(i).ok_or("--container necesita un nombre")?.clone());
            }
            "--interval" => {
                i += 1;
                let value = raw.get(i).ok_or("--interval necesita segundos")?;
                let secs = value
                    .parse::<u64>()
                    .map_err(|_| "--interval debe ser numerico")?;
                if secs == 0 {
                    return Err("--interval debe ser mayor a 0".to_string());
                }
                interval = Duration::from_secs(secs);
            }
            "--db" => {
                i += 1;
                db_path = PathBuf::from(raw.get(i).ok_or("--db necesita una ruta")?);
            }
            "--once" => once = true,
            "--retention-hours" => {
                i += 1;
                let value = raw.get(i).ok_or("--retention-hours necesita horas")?;
                retention_hours = value
                    .parse::<u64>()
                    .map_err(|_| "--retention-hours debe ser numerico")?;
            }
            "--max-samples" => {
                i += 1;
                let value = raw.get(i).ok_or("--max-samples necesita una cantidad")?;
                max_samples = value
                    .parse::<u64>()
                    .map_err(|_| "--max-samples debe ser numerico")?;
            }
            "--" => {
                let command = raw[i + 1..].to_vec();
                if command.is_empty() {
                    return Err("despues de -- debes pasar el binario a lanzar".to_string());
                }
                launch = Some(command);
                break;
            }
            "-h" | "--help" => return Ok(help_watch()),
            unknown => return Err(format!("argumento no reconocido en watch: {unknown}")),
        }
        i += 1;
    }

    let target_count = pid.is_some() as u8
        + name.is_some() as u8
        + container.is_some() as u8
        + launch.is_some() as u8;
    if target_count != 1 {
        return Err(
            "watch necesita exactamente uno: --pid, --name, --container o -- <comando>".to_string(),
        );
    }

    let target = if let Some(pid) = pid {
        Target::Pid(pid)
    } else if let Some(name) = name {
        Target::Name(name)
    } else if let Some(container) = container {
        Target::Container(container)
    } else {
        Target::Launch(launch.expect("launch checked above"))
    };

    Ok(WatchArgs {
        target,
        interval,
        db_path,
        once,
        retention_hours,
        max_samples,
    })
}

fn help_watch() -> WatchArgs {
    print_help();
    std::process::exit(0);
}

fn parse_summary(raw: &[String]) -> Result<SummaryArgs, String> {
    let mut db_path = PathBuf::from(DEFAULT_DB_PATH);
    let mut i = 0;

    while i < raw.len() {
        match raw[i].as_str() {
            "--db" => {
                i += 1;
                db_path = PathBuf::from(raw.get(i).ok_or("--db necesita una ruta")?);
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            unknown => return Err(format!("argumento no reconocido en summary: {unknown}")),
        }
        i += 1;
    }

    Ok(SummaryArgs { db_path })
}

pub fn print_help() {
    println!(
        "gsw

Uso:
  gsw watch --pid <PID> [--interval 2] [--db server-metrics.db]
  gsw watch --name <texto> [--interval 2] [--db server-metrics.db]
  gsw watch --container api-server [--interval 5] [--db server-metrics.db]
  gsw watch [--interval 2] [--db server-metrics.db] -- ./main -flag valor
  gsw summary [--db server-metrics.db]

Notas:
  - No abre puertos, no sirve HTTP y no queda escuchando en la red.
  - -- ./main lanza tu servidor Go como proceso hijo y mide solo ese PID.
  - --container sigue un contenedor Docker por nombre y se reengancha tras deploys.
  - Por defecto conserva 72 horas y maximo 150000 muestras.
  - Usa --retention-hours 24 o --max-samples 50000 para instancias pequenas.
  - Ctrl+C cierra la vista en tiempo real.
  - 100% de CPU significa un core completo; un Go server multi-core puede pasar de 100%.
"
    );
}
