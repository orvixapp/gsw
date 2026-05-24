use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command};

#[derive(Debug)]
pub struct ProcessCandidate {
    pub pid: i32,
    pub comm: String,
    pub cmdline: String,
}

pub struct LaunchedProcess {
    child: Child,
    finished: bool,
}

impl LaunchedProcess {
    pub fn spawn(command: &[String]) -> Result<Self, String> {
        let program = command
            .first()
            .ok_or("no hay comando para lanzar el servidor")?;
        let mut child = Command::new(program);
        child.args(&command[1..]);

        let child = child
            .spawn()
            .map_err(|err| format!("no pude lanzar {program}: {err}"))?;

        Ok(Self {
            child,
            finished: false,
        })
    }

    pub fn pid(&self) -> i32 {
        self.child.id() as i32
    }

    pub fn has_exited(&mut self) -> Result<Option<i32>, String> {
        let status = self
            .child
            .try_wait()
            .map(|status| status.map(|s| s.code().unwrap_or_default()))
            .map_err(|err| format!("no pude revisar el proceso hijo: {err}"))?;

        if status.is_some() {
            self.finished = true;
        }

        Ok(status)
    }
}

impl Drop for LaunchedProcess {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

pub fn find_single_process(needle: &str) -> Result<ProcessCandidate, String> {
    let needle = needle.to_lowercase();
    let mut candidates = Vec::new();

    for entry in fs::read_dir("/proc").map_err(|err| format!("no pude listar /proc: {err}"))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let filename = entry.file_name();
        let Some(pid_str) = filename.to_str() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<i32>() else {
            continue;
        };

        let comm = read_trimmed(format!("/proc/{pid}/comm")).unwrap_or_default();
        let cmdline = read_cmdline(pid).unwrap_or_default();
        let haystack = format!("{} {}", comm.to_lowercase(), cmdline.to_lowercase());

        if haystack.contains(&needle) {
            candidates.push(ProcessCandidate { pid, comm, cmdline });
        }
    }

    match candidates.len() {
        0 => Err(format!("no encontre procesos que coincidan con '{needle}'")),
        1 => Ok(candidates.remove(0)),
        _ => {
            let mut message =
                format!("encontre varios procesos para '{needle}'. Usa --pid con uno de estos:\n");
            for candidate in candidates.iter().take(12) {
                message.push_str(&format!(
                    "  pid={} comm={} cmdline={}\n",
                    candidate.pid, candidate.comm, candidate.cmdline
                ));
            }
            Err(message)
        }
    }
}

pub fn docker_container_pid(container: &str) -> Result<Option<i32>, String> {
    let output = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Pid}}", container])
        .output()
        .map_err(|err| format!("no pude ejecutar docker inspect: {err}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = stdout.trim();
    let pid = value
        .parse::<i32>()
        .map_err(|_| format!("docker inspect devolvio un PID invalido: {value}"))?;

    if pid > 0 { Ok(Some(pid)) } else { Ok(None) }
}

fn read_trimmed(path: impl AsRef<Path>) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .map_err(|err| err.to_string())
}

fn read_cmdline(pid: i32) -> Result<String, String> {
    let mut bytes = Vec::new();
    File::open(format!("/proc/{pid}/cmdline"))
        .map_err(|err| err.to_string())?
        .read_to_end(&mut bytes)
        .map_err(|err| err.to_string())?;

    Ok(bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part))
        .collect::<Vec<_>>()
        .join(" "))
}
