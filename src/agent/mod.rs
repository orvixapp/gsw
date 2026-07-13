//! Sampling orchestration and service supervision.

use crate::analysis::trends::GoroutineTrend;
use crate::cli::{ServiceSpec, Target, WatchArgs};
use crate::collectors::{cgroup, go_runtime, procfs};
use crate::domain::metrics::{self, HostSample, ProcessSample};
use crate::domain::service::{ServiceState, ServiceStatus};
use crate::platform::process::{self, LaunchedProcess};
use crate::presentation::terminal;
use crate::storage::sqlite::Store;
use std::thread;

pub fn watch(args: WatchArgs) -> Result<(), String> {
    let store = Store::open(&args.db_path)?;
    let cpu_count = procfs::cpu_count().unwrap_or(1).max(1);
    let mut runtimes = args
        .services
        .into_iter()
        .map(ServiceRuntime::new)
        .collect::<Result<Vec<_>, _>>()?;
    let mut previous_host = procfs::read_host_sample()?;

    for runtime in &mut runtimes {
        runtime.prepare(&previous_host);
    }

    loop {
        thread::sleep(args.interval);
        let current_host = procfs::read_host_sample()?;

        for runtime in &mut runtimes {
            runtime.sample(
                &previous_host,
                &current_host,
                cpu_count,
                args.pprof_interval.as_secs(),
            );
            if let (Some(pid), Some(metrics)) = (runtime.state.pid, &runtime.state.metrics) {
                store.insert_sample(&runtime.state.name, pid, metrics)?;
            }
        }

        store.prune(args.retention_hours, args.max_samples, current_host.epoch)?;
        if args.dashboard {
            terminal::print_dashboard(
                cpu_count,
                &args.db_path,
                &current_host,
                &runtimes
                    .iter()
                    .map(|runtime| runtime.state.clone())
                    .collect::<Vec<_>>(),
            );
        }
        previous_host = current_host;

        if args.once {
            break;
        }
    }

    Ok(())
}

struct ServiceRuntime {
    spec: ServiceSpec,
    state: ServiceState,
    launched: Option<LaunchedProcess>,
    previous_process: Option<ProcessSample>,
    last_goroutines: Option<u64>,
    next_pprof_epoch: u64,
    goroutine_trend: GoroutineTrend,
}

impl ServiceRuntime {
    fn new(spec: ServiceSpec) -> Result<Self, String> {
        let target = spec.target.description();
        let mut launched = None;
        let mut state = ServiceState::new(spec.name.clone(), target);
        if let Target::Launch(command) = &spec.target {
            let child = LaunchedProcess::spawn(command)?;
            state.pid = Some(child.pid());
            launched = Some(child);
        }
        Ok(Self {
            spec,
            state,
            launched,
            previous_process: None,
            last_goroutines: None,
            next_pprof_epoch: 0,
            goroutine_trend: GoroutineTrend::default(),
        })
    }

    fn prepare(&mut self, host: &HostSample) {
        if self.state.pid.is_none() {
            self.resolve_pid();
        }
        let Some(pid) = self.state.pid else {
            return;
        };
        match self.read_target_sample(pid) {
            Ok(sample) => {
                self.previous_process = Some(sample);
                self.state.status = ServiceStatus::Starting;
                self.state.last_error = None;
                self.next_pprof_epoch = host.epoch;
            }
            Err(err) => self.mark_unavailable(err),
        }
    }

    fn sample(
        &mut self,
        previous_host: &HostSample,
        current_host: &HostSample,
        cpu_count: u64,
        pprof_interval_secs: u64,
    ) {
        if self.launched_process_exited() {
            return;
        }

        // Keep using the known PID while it remains readable. Dynamic targets
        // are resolved again only after they disappear, avoiding a Docker CLI
        // subprocess (or a /proc scan) on every sampling interval.
        if self.state.pid.is_none() {
            self.resolve_pid();
        }
        let Some(pid) = self.state.pid else {
            return;
        };
        let current_process = match self.read_target_sample(pid) {
            Ok(sample) => sample,
            Err(err) => {
                self.mark_unavailable(err);
                return;
            }
        };
        let Some(previous_process) = self.previous_process.as_ref() else {
            self.previous_process = Some(current_process);
            self.state.status = ServiceStatus::Starting;
            return;
        };

        let mut current_metrics = metrics::calculate(
            previous_host,
            current_host,
            previous_process,
            &current_process,
            cpu_count,
        );
        self.previous_process = Some(current_process);

        if current_host.epoch >= self.next_pprof_epoch {
            self.sample_go_runtime(current_host.epoch);
            self.next_pprof_epoch = current_host
                .epoch
                .saturating_add(pprof_interval_secs.max(1));
        }
        current_metrics.goroutines = self.last_goroutines;
        current_metrics.goroutine_growth_per_min = self.goroutine_trend.growth_per_minute();
        self.state.leak_warning = self.goroutine_trend.is_suspicious();
        metrics::update_peak(&mut self.state.peak, &current_metrics);
        self.state.metrics = Some(current_metrics);
        self.state.status = ServiceStatus::Running;
        if self
            .state
            .last_error
            .as_deref()
            .is_some_and(|value| !value.starts_with("pprof:"))
        {
            self.state.last_error = None;
        }
    }

    fn resolve_pid(&mut self) {
        let resolved = match &self.spec.target {
            Target::Pid(pid) => Ok(Some(*pid)),
            Target::Name(name) => {
                process::find_single_process(name).map(|process| Some(process.pid))
            }
            Target::Container(name) => process::docker_container_pid(name),
            Target::Launch(_) => Ok(self.state.pid),
        };
        match resolved {
            Ok(Some(pid)) => {
                if self.state.pid != Some(pid) {
                    self.state.pid = Some(pid);
                    self.previous_process = None;
                    self.state.metrics = None;
                    self.state.status = ServiceStatus::Starting;
                    self.last_goroutines = None;
                    self.next_pprof_epoch = 0;
                    self.goroutine_trend = GoroutineTrend::default();
                    self.state.leak_warning = false;
                }
                self.state.last_error = None;
            }
            Ok(None) => self.mark_waiting("target is not running".to_string()),
            Err(err) => self.mark_waiting(err),
        }
    }

    fn sample_go_runtime(&mut self, epoch: u64) {
        let Some(url) = self.spec.pprof_url.as_deref() else {
            return;
        };
        match go_runtime::read_goroutine_count(url) {
            Ok(count) => {
                self.last_goroutines = Some(count);
                self.goroutine_trend.record(epoch, count);
                if self
                    .state
                    .last_error
                    .as_deref()
                    .is_some_and(|value| value.starts_with("pprof:"))
                {
                    self.state.last_error = None;
                }
            }
            Err(err) => self.state.last_error = Some(format!("pprof: {err}")),
        }
    }

    fn read_target_sample(&self, pid: i32) -> Result<ProcessSample, String> {
        if matches!(self.spec.target, Target::Container(_)) {
            cgroup::read_container_sample(pid).or_else(|_| procfs::read_process_sample(pid))
        } else {
            procfs::read_process_sample(pid)
        }
    }

    fn launched_process_exited(&mut self) -> bool {
        let Some(child) = self.launched.as_mut() else {
            return false;
        };
        match child.has_exited() {
            Ok(Some(code)) => {
                self.state.status = ServiceStatus::Error;
                self.state.last_error = Some(format!("launched process exited with code {code}"));
                self.state.pid = None;
                self.previous_process = None;
                true
            }
            Ok(None) => false,
            Err(err) => {
                self.state.status = ServiceStatus::Error;
                self.state.last_error = Some(err);
                true
            }
        }
    }

    fn mark_waiting(&mut self, error: String) {
        self.state.status = ServiceStatus::Waiting;
        self.state.last_error = Some(error);
        self.state.pid = None;
        self.previous_process = None;
    }

    fn mark_unavailable(&mut self, error: String) {
        match self.spec.target {
            Target::Pid(_) | Target::Launch(_) => {
                self.state.status = ServiceStatus::Error;
                self.state.last_error = Some(error);
                self.previous_process = None;
            }
            Target::Name(_) | Target::Container(_) => self.mark_waiting(error),
        }
    }
}
