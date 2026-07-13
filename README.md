# gsw

`gsw` is a low-overhead Linux monitoring agent for processes, Docker containers,
and Go services. It is designed for small servers where a full Prometheus and
Grafana stack would consume too much memory.

It opens no listening ports. Metrics come from `/proc`, cgroup v2, and optional
loopback-only Go pprof endpoints. Samples are displayed in a terminal dashboard
and stored in a local SQLite database.

## Features

- Monitor multiple named services in one process.
- Track services by PID, process name, or stable Docker container name.
- Reattach automatically when a container is recreated.
- Read aggregate container CPU, memory, I/O, and task counts from cgroup v2.
- Track process CPU, RSS, virtual memory, threads, file descriptors, and I/O rates.
- Store per-service history in SQLite with WAL and retention limits.
- Sample Go goroutine counts from optional pprof endpoints.
- Detect sustained goroutine growth using a bounded in-memory trend window.
- Run interactively or as a headless systemd agent.
- Keep all CLI, diagnostics, configuration, and terminal output in English.

## Requirements

- Linux.
- SQLite runtime library.
- Docker CLI and access to `/var/run/docker.sock` for container targets.
- cgroup v2 for aggregate container metrics. Process metrics remain available as
  a fallback when cgroup v2 cannot be read.

Install the SQLite runtime:

```bash
# Arch / CachyOS
sudo pacman -S sqlite

# Ubuntu / Debian
sudo apt install libsqlite3-0
```

## Build

```bash
cargo build --release
sudo install -m 0755 target/release/gsw /usr/local/bin/gsw
```

## Interactive usage

Monitor several containers:

```bash
gsw watch \
  --service api=container:orvix-api \
  --service telephony=container:orvix-telephony \
  --service cache=container:llavero \
  --interval 5 \
  --db server-metrics.db
```

Legacy single-target commands remain supported:

```bash
gsw watch --pid 12345
gsw watch --name api-server --label api
gsw watch --container api-server --label api
gsw watch -- ./server --port 8080
```

`CPU 100%` means one full logical CPU, following `top` semantics. A multi-core
service can exceed 100%.

## Configuration

Copy the example and edit service names:

```bash
sudo install -d -m 0755 /etc/gsw /var/lib/gsw
sudo install -m 0644 config/gsw.example.toml /etc/gsw/config.toml
gsw watch --config /etc/gsw/config.toml
```

Example:

```toml
database = "/var/lib/gsw/metrics.db"
interval_seconds = 5
pprof_interval_seconds = 30
retention_hours = 168
max_samples = 150000

[[services]]
name = "api"
target = "container:orvix-api"

[[services]]
name = "telephony"
target = "container:orvix-telephony"
```

Supported target formats are `pid:123`, `name:server`, and
`container:container-name`.

## Headless agent

The `agent` command runs the same collector without redrawing a dashboard:

```bash
gsw agent --config /etc/gsw/config.toml
```

Install the supplied systemd unit:

```bash
sudo install -m 0644 packaging/systemd/gsw.service /etc/systemd/system/gsw.service
sudo systemctl daemon-reload
sudo systemctl enable --now gsw
sudo systemctl status gsw
```

The unit applies a 32 MiB memory high-water mark and a 96 MiB hard limit to keep
the monitoring agent from becoming a resource problem itself.

## Go runtime metrics

`/proc` and cgroups cannot see goroutines. A Go service must expose pprof on a
private endpoint. Never attach pprof to a public application port.

For Docker, publish a dedicated diagnostics port on loopback only:

```yaml
ports:
  - "127.0.0.1:6060:6060"
```

Then add the URL to the service configuration:

```toml
[[services]]
name = "api"
target = "container:orvix-api"
pprof_url = "http://127.0.0.1:6060"
```

The pprof request has a 750 ms timeout and a 128 KiB response limit. It runs on
the slower `pprof_interval_seconds` schedule rather than every process sample.
Full heap and goroutine profiles are not continuously stored.

## Historical summary

```bash
gsw summary --db /var/lib/gsw/metrics.db
gsw summary --service telephony --db /var/lib/gsw/metrics.db
```

Rows are grouped by service and local hour. The report includes CPU averages and
peaks, RSS averages and peaks, goroutine trends, and peak file descriptor usage.

## Retention and disk use

The default configuration retains seven days of detailed samples and at most
150,000 rows per service. Count-based pruning is partitioned by service, so
adding more services does not silently shorten another service's history.

SQLite runs in WAL mode. Do not manually delete the `-wal` or `-shm` files while
the agent is running.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for module boundaries,
collection flow, and storage decisions.

## Security

`gsw` does not serve HTTP or accept remote input. Docker socket access is highly
privileged; only trusted users should run container monitoring. Pprof endpoints
must remain on loopback or another private transport.
