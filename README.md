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

- Linux with systemd (user session)
- Rust 1.70+

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
| `sdtab list` | List all managed timers and services |
| `sdtab status <name>` | Show detailed status with next 5 run times |
| `sdtab edit <name>` | Edit unit file with $EDITOR |
| `sdtab logs <name> [-f] [-n N]` | View logs (journalctl) |
| `sdtab restart <name>` | Restart a service |
| `sdtab enable <name>` | Enable a timer or service |
| `sdtab disable <name>` | Disable (keep files) |
| `sdtab remove <name>` | Remove completely |
| `sdtab export [-o <file>]` | Export config as TOML |
| `sdtab apply <file> [--prune] [--dry-run]` | Apply config from TOML |

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

Use `sdtab apply Sdtabfile.toml` to recreate all units from this file. Add `--prune` to remove units not in the file.

## How It Works

sdtab generates standard systemd unit files under `~/.config/systemd/user/` with a `sdtab-` prefix. No custom daemon or database — everything is plain systemd.

```
~/.config/systemd/user/
├── sdtab-backup.service    # [Service] definition
├── sdtab-backup.timer      # [Timer] with OnCalendar
├── sdtab-web.service       # Long-running service
```

Metadata is stored as comments in the service file (`# sdtab:type=`, `# sdtab:cron=`, etc.), so sdtab can reconstruct the original configuration without an external database.

## AI Agent Ready

sdtab is designed to work with AI coding agents (Claude Code, Cline, Devin, Cursor, etc.) out of the box.

**The problem**: systemd is notoriously hard for AI agents to operate. Creating a timer requires writing two files in the correct format, placing them in the right directory, running `daemon-reload`, then `enable --now`. One mistake and nothing works.

**The solution**: `sdtab add "0 9 * * *" "./backup.sh"` — one command, done.

### What's included

- **`CLAUDE.md`** — project instructions that AI agents read automatically. Contains the full command reference, architecture, and design decisions.
- **Skill file** — a pre-built prompt (`sdtab.md`) that teaches Claude Code how to manage timers and services through sdtab.
- **`--dry-run`** — lets agents preview generated unit files before committing.
- **`--json`** — machine-readable output for programmatic use.

```bash
# Agent can list units and parse the output
sdtab list --json
```

```json
[
  {"name":"backup","type":"timer","schedule":"0 3 * * *","command":"./backup.sh","status":"Mon 2026-03-02 03:00:00 JST"},
  {"name":"web","type":"service","schedule":"@service","command":"node index.js","status":"active"}
]
```

### Comparison

| Feature | raw systemd | crontab | sdtab |
|---------|------------|---------|-------|
| Timer + service in one command | No (2 files + 3 commands) | N/A (no services) | Yes |
| AI agent friendly | No | Partial | Yes |
| Export/import config | No | No | Yes (TOML) |
| Resource limits | Manual | No | `--memory-max`, `--cpu-quota` |
| Machine-readable output | `systemctl show` (verbose) | No | `--json` |

## License

MIT
