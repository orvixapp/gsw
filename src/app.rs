use crate::cli::{self, Command, Target, WatchArgs};
use crate::metrics::{self, Peak, ProcSample};
use crate::process::{self, LaunchedProcess};
use crate::sqlite_store::Store;
use crate::{procfs, summary, terminal};
use std::env;
use std::thread;

pub fn run() -> Result<(), String> {
    match cli::parse(env::args().skip(1).collect())? {
        Command::Help => {
            cli::print_help();
            Ok(())
        }
        Command::Watch(args) => watch(args),
        Command::Summary(args) => {
            let store = Store::open(&args.db_path)?;
            let rows = store.hourly_summary()?;
            summary::print_hourly(&rows)
        }
    }
}

fn watch(args: WatchArgs) -> Result<(), String> {
    let container_target = if let Target::Container(container) = &args.target {
        Some(container.clone())
    } else {
        None
    };
    if let Some(container) = container_target {
        return watch_container(args, container);
    }

    let mut launched = None;
    let pid = match args.target {
        Target::Pid(pid) => pid,
        Target::Name(name) => process::find_single_process(&name)?.pid,
        Target::Container(_) => unreachable!("container handled above"),
        Target::Launch(command) => {
            let child = LaunchedProcess::spawn(&command)?;
            let pid = child.pid();
            launched = Some(child);
            pid
        }
    };

    let store = Store::open(&args.db_path)?;
    let cpu_count = procfs::cpu_count().unwrap_or(1).max(1);
    let mut peak = Peak::default();
    let mut prev = procfs::read_sample(pid)?;

    loop {
        thread::sleep(args.interval);

        if let Some(child) = launched.as_mut() {
            if let Some(code) = child.has_exited()? {
                return Err(format!("el proceso monitoreado termino con codigo {code}"));
            }
        }

        let current = procfs::read_sample(pid)?;
        let metrics = metrics::calculate(&prev, &current, cpu_count);

        metrics::update_peak(&mut peak, &metrics);
        store.insert_sample(&metrics)?;
        store.prune(args.retention_hours, args.max_samples, metrics.epoch)?;
        terminal::print_live(pid, cpu_count, &args.db_path, &metrics, &peak);

        prev = current;
        if args.once {
            break;
        }
    }

    Ok(())
}

fn watch_container(args: WatchArgs, container: String) -> Result<(), String> {
    let store = Store::open(&args.db_path)?;
    let cpu_count = procfs::cpu_count().unwrap_or(1).max(1);
    let mut peak = Peak::default();
    let mut active_pid = None;
    let mut prev: Option<ProcSample> = None;

    loop {
        let Some(pid) = process::docker_container_pid(&container)? else {
            active_pid = None;
            prev = None;
            terminal::print_waiting_container(&container, &args.db_path);
            thread::sleep(args.interval);
            continue;
        };

        if active_pid != Some(pid) || prev.is_none() {
            active_pid = Some(pid);
            prev = Some(procfs::read_sample(pid)?);
            terminal::print_attaching_container(&container, pid, &args.db_path);
            thread::sleep(args.interval);
            continue;
        }

        let current = match procfs::read_sample(pid) {
            Ok(sample) => sample,
            Err(_) => {
                active_pid = None;
                prev = None;
                terminal::print_waiting_container(&container, &args.db_path);
                thread::sleep(args.interval);
                continue;
            }
        };
        let metrics = metrics::calculate(
            prev.as_ref().expect("prev checked above"),
            &current,
            cpu_count,
        );

        metrics::update_peak(&mut peak, &metrics);
        store.insert_sample(&metrics)?;
        store.prune(args.retention_hours, args.max_samples, metrics.epoch)?;
        terminal::print_live(pid, cpu_count, &args.db_path, &metrics, &peak);

        prev = Some(current);
        if args.once {
            break;
        }

        thread::sleep(args.interval);
    }

    Ok(())
}
