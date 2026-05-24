# Security

`gsw` is a local CLI. It does not expose HTTP, listen on sockets, or accept
remote input.

## Reporting

For security issues, please open a private report through GitHub Security
Advisories if the repository has advisories enabled. Otherwise contact the
maintainers privately before publishing details.

## Scope

Security-sensitive areas:

- Docker PID resolution through `docker inspect`.
- Reading process data from `/proc`.
- Writing metrics into a local SQLite database.

Out of scope:

- Attacks requiring arbitrary shell access to the host.
- Misconfigured Docker permissions on the host.
