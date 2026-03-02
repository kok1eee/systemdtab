# sdtab

[日本語](README_ja.md)

Manage systemd user timers and services like crontab.

```bash
# Add a timer (cron syntax)
sdtab add "0 9 * * *" "uv run ./report.py"

# Add a long-running service
sdtab add "@service" "node server.js" --restart on-failure

# List all managed units
sdtab list
```

```
NAME    TYPE     SCHEDULE     COMMAND               STATUS
report  timer    0 9 * * *    uv run ./report.py    Tue 2026-03-03 09:00:00 JST
web     service  @service     node server.js        active
```

## Features

- **cron syntax** — familiar `* * * * *` schedule format, automatically converted to systemd OnCalendar
- **Long-running services** — use `@service` to create always-on daemons with restart policies
- **Resource limits** — set memory, CPU, and I/O constraints per unit
- **Export/Import** — `sdtab export` dumps config to TOML, `sdtab apply` restores it on another machine
- **Zero dependencies beyond systemd** — no database, no daemon, just unit files

## Install

```bash
cargo install --git https://github.com/kok1eee/systemdtab
```

Or build from source:

```bash
git clone https://github.com/kok1eee/systemdtab
cd systemdtab
cargo build --release
cp target/release/sdtab ~/.local/bin/
```

### Requirements

- Linux with systemd (user session only — system-wide units are not supported)
- Rust 1.70+

> **Note**: sdtab manages **user-level** units only (`systemctl --user`). It cannot create or manage system-wide services that require root privileges. If `loginctl enable-linger` fails, ask your system administrator to enable it for your user.

## Quick Start

```bash
# Initialize sdtab (enables linger, creates config directory)
sdtab init

# Add a daily task at 9:00 AM
sdtab add "0 9 * * *" "./backup.sh" --name backup --memory-max 512M

# Add a service that runs continuously
sdtab add "@service" "node dist/index.js" --name web --restart on-failure --env-file .env

# Check status
sdtab list
sdtab status backup

# View logs
sdtab logs web -f

# Export config to file
sdtab export -o Sdtabfile.toml

# Apply config from file (on another machine)
sdtab apply Sdtabfile.toml --dry-run
sdtab apply Sdtabfile.toml
```

## Commands

| Command | Description |
|---------|-------------|
| `sdtab init` | Enable linger and create directories |
| `sdtab add "<schedule>" "<command>" [--dry-run]` | Add a timer |
| `sdtab add "@service" "<command>" [--dry-run]` | Add a long-running service |
| `sdtab list [--json]` | List all managed timers and services |
| `sdtab status <name>` | Show detailed status with next 5 run times |
| `sdtab edit <name>` | Edit unit file with $EDITOR |
| `sdtab logs <name> [-f] [-n N]` | View logs (journalctl) |
| `sdtab restart <name>` | Restart a service |
| `sdtab enable <name>` | Enable a timer or service |
| `sdtab disable <name>` | Disable (keep files) |
| `sdtab remove <name>` | Stop, disable, and remove unit files |
| `sdtab export [-o <file>]` | Export config as TOML |
| `sdtab apply <file> [--prune] [--dry-run]` | Apply config from TOML |

> `sdtab remove` stops and disables the unit before deleting files. `sdtab apply --prune` only removes units with the `sdtab-` prefix — manually created systemd units are never touched.

## Schedule Syntax

Standard cron expressions and convenient shortcuts:

| Expression | Meaning |
|-----------|---------|
| `*/5 * * * *` | Every 5 minutes |
| `0 9 * * *` | Daily at 9:00 |
| `0 9 * * Mon-Fri` | Weekdays at 9:00 |
| `@daily` | Once a day (midnight) |
| `@hourly` | Once an hour |
| `@reboot` | On system boot |
| `@daily/3` | Every 3 days |
| `@weekly/Mon,Wed` | Every Monday and Wednesday |
| `@service` | Long-running service (not a timer) |

## Add Options

| Option | Description |
|--------|-------------|
| `--name <name>` | Unit name (auto-generated from command if omitted) |
| `--workdir <path>` | Working directory (defaults to current) |
| `--description <text>` | Description |
| `--env-file <path>` | Environment file |
| `--restart <policy>` | `always` / `on-failure` / `no` (services only, default: `always`) |
| `--memory-max <size>` | Memory limit (e.g. `512M`, `1G`) |
| `--cpu-quota <percent>` | CPU limit (e.g. `50%`, `200%`) |
| `--io-weight <N>` | I/O priority: 1-10000 (default: 100) |
| `--timeout-stop <duration>` | Stop timeout (e.g. `30s`) |
| `--random-delay <duration>` | Random delay for timer firing (e.g. `5m`) |
| `--env <KEY=VALUE>` | Environment variable (repeatable) |
| `--dry-run` | Preview generated unit files without creating them |

## Export Format

`sdtab export` produces a TOML file:

```toml
[timers.backup]
schedule = "0 3 * * *"
command = "./backup.sh"
workdir = "/home/user/project"
memory_max = "512M"

[services.web]
command = "node dist/index.js"
workdir = "/home/user/app"
description = "Web Server"
restart = "on-failure"
env_file = "/home/user/.env"
```

Use `sdtab apply Sdtabfile.toml` to recreate all units from this file. Add `--prune` to remove sdtab-managed units not in the file.

## How It Works

sdtab generates standard systemd unit files under `~/.config/systemd/user/` with a `sdtab-` prefix. No custom daemon or database — everything is plain systemd.

```
~/.config/systemd/user/
├── sdtab-backup.service    # [Service] definition
├── sdtab-backup.timer      # [Timer] with OnCalendar
├── sdtab-web.service       # Long-running service
```

Metadata is stored as comments in the service file (`# sdtab:type=`, `# sdtab:cron=`, etc.), so sdtab can reconstruct the original configuration without an external database.

## Comparison with Alternatives

| | sdtab | crontab | [systemd-cron](https://github.com/systemd-cron/systemd-cron) | [fcron](http://fcron.free.fr/) | [jobber](https://github.com/dshearer/jobber) |
|---|---|---|---|---|---|
| One command to create timer | Yes | Yes | No (uses crontab files) | Yes | Yes |
| Long-running services | Yes (`@service`) | No | No (oneshot only) | No | No |
| Resource limits (memory/CPU) | `--memory-max`, `--cpu-quota` | No | Manual (edit unit files) | No | No |
| Export/import config | Yes (TOML) | `crontab -l` (text) | No | No | No |
| Machine-readable output | `--json` | No | No | No | No |
| Backend | systemd native | crond | systemd (generated) | own daemon | own daemon |
| User-level without root | Yes | Yes | System-level | Needs root | Needs root |

## Testing

The cron parser, unit file generation, and TOML serialization are covered by 71 unit tests:

```bash
cargo test
```

## AI Agent Support

sdtab includes files for AI coding agents (Claude Code, Cursor, etc.):

- **`CLAUDE.md`** — project instructions with full command reference
- **`--dry-run`** — preview generated unit files before committing
- **`--json`** — machine-readable output for programmatic use

```bash
sdtab list --json
```

```json
[
  {"name":"backup","type":"timer","schedule":"0 3 * * *","command":"./backup.sh","status":"Mon 2026-03-02 03:00:00 JST"},
  {"name":"web","type":"service","schedule":"@service","command":"node index.js","status":"active"}
]
```

## License

MIT
