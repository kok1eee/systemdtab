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
web     service  @service     node server.js        ● active
  Web Server
```

## Features

- **cron syntax** — familiar `* * * * *` schedule format, automatically converted to systemd OnCalendar
- **Long-running services** — use `@service` to create always-on daemons with restart policies
- **Resource limits** — set memory, CPU, and I/O constraints per unit
- **Export/Import** — `sdtab export` dumps config to TOML, `sdtab apply` restores it on another machine
- **Failure notifications** — Slack webhook alerts when a unit fails (`OnFailure=` mechanism)
- **Colored status** — `● active` (green), `● failed` (red), `○ inactive` (yellow) at a glance; descriptions shown as gray subtitles; auto-disabled when piped
- **Zero dependencies beyond systemd** — no database, no daemon, just unit files

## Install

```bash
# Pre-built binary (no Rust required)
curl -L https://github.com/kok1eee/systemdtab/releases/latest/download/sdtab-x86_64-linux \
  -o ~/.local/bin/sdtab && chmod +x ~/.local/bin/sdtab

# Or via Cargo
cargo install systemdtab

# Or build from source
git clone https://github.com/kok1eee/systemdtab
cd systemdtab
cargo build --release
cp target/release/sdtab ~/.local/bin/
```

### Requirements

- Linux with systemd (user session only — system-wide units are not supported)
- `~/.local/bin` in your `$PATH`

> **Note**: sdtab manages **user-level** units only (`systemctl --user`). It cannot create or manage system-wide services that require root privileges. If `loginctl enable-linger` fails, ask your system administrator to enable it for your user.

## Quick Start

```bash
# Initialize sdtab (enables linger, creates config directory)
sdtab init

# Optional: set up Slack failure notifications
sdtab init --slack-webhook "https://hooks.slack.com/services/T.../B.../xxx"

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
| `sdtab init [--slack-webhook URL] [--slack-mention USER_ID]` | Enable linger, create directories, set up notifications |
| `sdtab add "<schedule>" "<command>" [--dry-run]` | Add a timer |
| `sdtab add "@service" "<command>" [--dry-run]` | Add a long-running service |
| `sdtab list [--json] [--sort time\|name]` | List all managed timers and services (default: sorted by next run time) |
| `sdtab status <name>` | Show detailed status with next 5 run times |
| `sdtab edit <name>` | Edit unit file with $EDITOR (see caveat below) |
| `sdtab logs <name> [-f] [-n N]` | View logs (journalctl) |
| `sdtab restart <name>` | Restart a service |
| `sdtab enable <name>` | Enable a timer or service |
| `sdtab disable <name>` | Disable (keep files) |
| `sdtab remove <name>` | Stop, disable, and remove unit files |
| `sdtab export [-o <file>]` | Export config as TOML |
| `sdtab apply <file> [--prune] [--dry-run]` | Apply config from TOML |

> `sdtab remove` stops and disables the unit before deleting files. `sdtab apply --prune` only removes units with the `sdtab-` prefix — manually created systemd units are never touched.

> **`sdtab edit` caveat**: sdtab stores metadata (original cron expression, command, etc.) as comments at the top of the `.service` file (`# sdtab:cron=`, `# sdtab:command=`). If you delete these comments during editing, `sdtab list` and `sdtab export` will show `?` for the missing fields. The unit itself will still work — only sdtab's bookkeeping is affected.

> `sdtab apply` updates changed units **in-place** (overwrite → daemon-reload) without stopping them first. Only units that actually need a restart are restarted: for services, description-only changes skip restart; for timers, service-side changes (command, env, etc.) take effect on the next trigger without restarting the timer.

> `sdtab init` also installs a [Claude Code](https://docs.anthropic.com/en/docs/claude-code) skill file to `~/.claude/commands/sdtab.md`, enabling `/sdtab` commands in any project.

## Schedule Syntax

Standard cron expressions and convenient shortcuts:

| Expression | Meaning |
|-----------|---------|
| `*/5 * * * *` | Every 5 minutes |
| `0 9 * * *` | Daily at 9:00 |
| `0 9 * * Mon-Fri` | Weekdays at 9:00 |
| `@daily` | Once a day (midnight) |
| `@daily/9` | Daily at 9:00 |
| `@daily/9:30` | Daily at 9:30 |
| `@hourly` | Once an hour |
| `@reboot` | On system boot |
| `@mon/13` | Every Monday at 13:00 |
| `@tue/18` | Every Tuesday at 18:00 |
| `@weekly/Mon,Wed` | Every Monday and Wednesday |
| `@1st/8` | 1st of every month at 8:00 |
| `@20th/8` | 20th of every month at 8:00 |
| `@26th/11:30` | 26th of every month at 11:30 |
| `@service` | Long-running service (not a timer) |

Weekdays use English abbreviations (`@mon`, `@tue`, ..., `@sun`). Dates use English ordinals (`@1st`, `@2nd`, `@20th`, `@26th`). The `/` separator always means "at this time".

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
| `--exec-start-pre <cmd>` | Command to run before ExecStart |
| `--exec-stop-post <cmd>` | Command to run after process stops |
| `--log-level-max <level>` | Max log level to store (e.g. `warning`, `err`) |
| `--random-delay <duration>` | Random delay for timer firing (e.g. `5m`) |
| `--env <KEY=VALUE>` | Environment variable (repeatable) |
| `--no-notify` | Disable failure notification for this unit |
| `--dry-run` | Preview generated unit files without creating them |

## Failure Notifications

Set up Slack notifications for when any unit fails:

```bash
sdtab init --slack-webhook "https://hooks.slack.com/services/T.../B.../xxx"

# With user mention
sdtab init --slack-webhook "https://hooks.slack.com/services/T.../B.../xxx" --slack-mention "U0700J8MN3W"
```

This creates a template unit `sdtab-notify@.service` and adds `OnFailure=sdtab-notify@%n.service` to all subsequently created units. When a timer or service fails, systemd triggers the notification unit, which sends a message to Slack via `curl`.

To opt out of notifications for a specific unit:

```bash
sdtab add "0 9 * * *" "./quiet-task.sh" --no-notify
```

In `Sdtabfile.toml`, use `no_notify = true`:

```toml
[timers.quiet-task]
schedule = "0 9 * * *"
command = "./quiet-task.sh"
workdir = "/home/user"
no_notify = true
```

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
├── sdtab-notify@.service   # Failure notification template (if webhook configured)
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

The cron parser, unit file generation, and TOML serialization are covered by 93 unit tests:

```bash
cargo test
```

## AI Agent Support

`sdtab init` installs a [Claude Code](https://docs.anthropic.com/en/docs/claude-code) skill file to `~/.claude/commands/sdtab.md`. After that, you can manage systemd timers from any project with natural language:

```
You> /sdtab run report.py every day at 9am
```

Claude Code interprets the intent, runs `sdtab add "0 9 * * *" "uv run ./report.py" --dry-run` for confirmation, then creates the timer.

The skill works globally — not just inside the sdtab repository. Once installed, any Claude Code session can create, list, and manage timers and services through `/sdtab`.

Also included:

- **`CLAUDE.md`** — project instructions with full command reference (auto-loaded by AI agents)
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
