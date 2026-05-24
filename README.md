# gsw

`gsw` is a small Linux CLI for watching the CPU, memory, disk counters, load,
and uptime around one process or Docker container.

It is designed for small servers where a full monitoring stack is too much. It
does not open ports, serve HTTP, or expose a web UI. Metrics are read from
`/proc`, shown in the terminal, and stored in a local SQLite database.

## Features

- Live terminal view for one process.
- Docker container tracking by stable container name.
- Automatic re-attach when a container is recreated.
- SQLite history with retention limits.
- Hourly summary for peak usage analysis.
- No background daemon, no web server, no network listener.

## Requirements

- Linux.
- SQLite runtime library.
- Docker CLI only when using `--container`.

Install SQLite runtime:

```bash
# Arch / CachyOS
sudo pacman -S sqlite

# Ubuntu / Debian
sudo apt install libsqlite3-0
```

## Install From Release

Download the Linux archive from GitHub Releases, then:

```bash
tar -xzf gsw-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
cd gsw-v0.1.0-x86_64-unknown-linux-gnu
sudo install -m 0755 gsw /usr/local/bin/gsw
gsw --help
```

## Build From Source

```bash
cargo build --release
sudo install -m 0755 target/release/gsw /usr/local/bin/gsw
```

Or install with Cargo:

```bash
cargo install --path .
```

## Usage

Watch an existing process:

```bash
gsw watch --pid 12345 --interval 2 --db server-metrics.db
```

Find a process by name:

```bash
gsw watch --name api-server --interval 2
```

Launch a process and watch it:

```bash
gsw watch --db server-metrics.db -- ./server
```

Pass arguments to the launched process:

```bash
gsw watch --interval 1 -- ./server --port 8080
```

For production, it is usually safer to start the application normally and
attach `gsw` by PID or container name. That way stopping `gsw` does not stop the
application.

## Docker

Watch a container by stable name:

```bash
gsw watch --container api-server --interval 5 --retention-hours 24 --max-samples 30000 --db server-metrics.db
```

`gsw` resolves the container's host PID with `docker inspect`. If a deploy stops
and recreates the container with the same name, `gsw` waits during the gap and
attaches to the new PID when the container is running again.

The user running `gsw` must be allowed to run:

```bash
docker inspect -f '{{.State.Pid}}' api-server
```

## Live View

The terminal view shows:

- Process CPU and percent of total host capacity.
- Total system CPU usage.
- Process RSS memory and percent of host memory.
- System used and available memory.
- Load average and uptime.
- Thread count.
- Accumulated disk read/write counters when available.
- Session peaks.

Exit with `Ctrl+C`.

## Retention

By default, `gsw` keeps:

- 72 hours of samples.
- 150000 samples maximum.

For small disks, use a wider interval and stricter retention:

```bash
gsw watch --interval 5 --retention-hours 24 --max-samples 30000 --db server-metrics.db -- ./server
```

With `--interval 5`, 24 hours is about 17280 samples.

## Summary

After collecting data:

```bash
gsw summary --db server-metrics.db
```

`CPU proc` follows the same convention as `top`: `100%` means one full CPU core.
A multi-core application can go above `100%`.

## Data

SQLite creates a `samples` table with:

- `local_ts`: local sample timestamp.
- `local_hour`: hourly bucket.
- `cpu_percent`: process CPU, where `100% = one full core`.
- `system_cpu_percent`: total system CPU.
- `rss_mb`: process physical memory.
- `mem_total_mb`, `mem_used_mb`, `mem_available_mb`: system memory.
- `vm_size_mb`: process virtual memory.
- `threads`: process thread count.
- `load1`, `load5`, `load15`: system load averages.
- `read_mb`, `write_mb`: accumulated disk counters when available.

## Releases

The GitHub Actions workflow builds Linux x86_64 release archives when a version
tag is pushed:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Build the same archive locally:

```bash
./scripts/package-tar.sh
```

## Security

`gsw` is a local CLI. It does not expose HTTP, listen on sockets, or accept
remote input. It reads local Linux process information and writes a local SQLite
database.
