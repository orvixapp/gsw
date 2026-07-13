# Architecture

`gsw` is a single-node, low-overhead monitoring agent. It intentionally avoids
an async runtime, a network listener, and a separate database service.

## Modules

- `cli`: command-line compatibility and multi-service target parsing.
- `config`: strict TOML configuration for unattended collection.
- `domain`: service state and metric models.
- `collectors`: Linux procfs and optional Go runtime collection.
- `platform`: process discovery, Docker PID resolution, and local time.
- `analysis`: bounded in-memory trend detection.
- `storage`: SQLite schema migration, retention, and summaries.
- `agent`: the single sampling loop and target supervision.
- `presentation`: terminal dashboard and historical reports.

## Collection model

The host is sampled once per interval. Process samples are then collected for
each configured service and calculated against the same host CPU delta. This
keeps cross-service comparisons aligned and avoids duplicate reads of host
memory, CPU, load, and uptime.

Go runtime collection is optional and slower than process collection. The
goroutine endpoint is sampled on its own interval with a 750 ms timeout and a
128 KiB response limit. Full goroutine and heap profiles are deliberately not
captured on every interval; future alert-triggered profiles belong outside the
main sample table.

## Storage model

SQLite uses WAL mode and stores one row per service sample. Every row carries a
stable service name and the active PID so container recreation remains visible
in history. Retention by count is applied independently per service.

The database is local operational history, not a distributed telemetry store.
Long-term fleet monitoring should export rollups to an external system rather
than turning this agent into a server.
