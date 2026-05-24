# Contributing

`gsw` is intentionally small. Changes should keep the tool local-first,
Linux-focused, and cheap to run on small servers.

## Development

```bash
cargo fmt
cargo test --locked
cargo build --release
```

## Packaging

```bash
./scripts/package-tar.sh
```

## Design boundaries

- Do not add a web server or open network ports.
- Prefer `/proc` and small standard Linux interfaces over heavy dependencies.
- Keep SQLite retention enabled by default.
- Docker support should follow stable container names, not ephemeral IDs.
