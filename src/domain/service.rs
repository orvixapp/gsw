use crate::domain::metrics::{Metrics, Peak};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Starting,
    Running,
    Waiting,
    Error,
}

impl ServiceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub name: String,
    pub target: String,
    pub pid: Option<i32>,
    pub status: ServiceStatus,
    pub metrics: Option<Metrics>,
    pub peak: Peak,
    pub leak_warning: bool,
    pub last_error: Option<String>,
}

impl ServiceState {
    pub fn new(name: String, target: String) -> Self {
        Self {
            name,
            target,
            pid: None,
            status: ServiceStatus::Starting,
            metrics: None,
            peak: Peak::default(),
            leak_warning: false,
            last_error: None,
        }
    }
}
