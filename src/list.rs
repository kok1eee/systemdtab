use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::{init, systemctl};

enum EntryType {
    Timer,
    Service,
}

struct Entry {
    name: String,
    entry_type: EntryType,
    schedule: String,
    command: String,
    status: String,
}

pub fn run() -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    if !dir_path.exists() {
        println!("No timers or services found. Run 'sdtab init' first.");
        return Ok(());
    }

    let mut entries: Vec<Entry> = Vec::new();

    // Find all sdtab-*.service files (both timers and daemon services have .service)
    let read_dir = fs::read_dir(dir_path)?;
    for entry in read_dir {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();

        if !filename.starts_with("sdtab-") || !filename.ends_with(".service") {
            continue;
        }

        let name = filename
            .strip_prefix("sdtab-")
            .unwrap()
            .strip_suffix(".service")
            .unwrap()
            .to_string();

        let service_content = fs::read_to_string(entry.path())?;
        let metadata = parse_service_metadata(&service_content);

        let (entry_type, schedule, status) = match metadata.unit_type {
            EntryType::Service => {
                let service_unit = format!("sdtab-{}.service", name);
                let active_state = systemctl::show_property(&service_unit, "ActiveState")
                    .unwrap_or_else(|_| "unknown".to_string());
                (EntryType::Service, "-".to_string(), active_state)
            }
            EntryType::Timer => {
                let timer_unit = format!("sdtab-{}.timer", name);
                let next_run =
                    systemctl::show_property(&timer_unit, "NextElapseUSecRealtime")
                        .unwrap_or_else(|_| "?".to_string());
                let next_run = format_next_run(&next_run);
                (EntryType::Timer, metadata.cron_expr, next_run)
            }
        };

        entries.push(Entry {
            name,
            entry_type,
            schedule,
            command: metadata.command,
            status,
        });
    }

    if entries.is_empty() {
        println!("No timers or services found.");
        return Ok(());
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));

    // Calculate column widths
    let name_width = entries.iter().map(|e| e.name.len()).max().unwrap_or(4).max(4);
    let type_width = 7; // "service" is the longest
    let sched_width = entries
        .iter()
        .map(|e| e.schedule.len())
        .max()
        .unwrap_or(8)
        .max(8);
    let cmd_width = entries
        .iter()
        .map(|e| e.command.len())
        .max()
        .unwrap_or(7)
        .max(7);

    println!(
        "{:<name_w$}  {:<type_w$}  {:<sched_w$}  {:<cmd_w$}  STATUS",
        "NAME",
        "TYPE",
        "SCHEDULE",
        "COMMAND",
        name_w = name_width,
        type_w = type_width,
        sched_w = sched_width,
        cmd_w = cmd_width,
    );

    for entry in &entries {
        let type_str = match entry.entry_type {
            EntryType::Timer => "timer",
            EntryType::Service => "service",
        };

        println!(
            "{:<name_w$}  {:<type_w$}  {:<sched_w$}  {:<cmd_w$}  {}",
            entry.name,
            type_str,
            entry.schedule,
            entry.command,
            entry.status,
            name_w = name_width,
            type_w = type_width,
            sched_w = sched_width,
            cmd_w = cmd_width,
        );
    }

    Ok(())
}

struct ServiceMetadata {
    unit_type: EntryType,
    cron_expr: String,
    command: String,
}

fn parse_service_metadata(content: &str) -> ServiceMetadata {
    let mut unit_type = EntryType::Timer; // default for backward compat
    let mut cron_expr = "?".to_string();
    let mut command = "?".to_string();
    let mut has_cron = false;

    for line in content.lines() {
        if line.starts_with("# sdtab:type=service") {
            unit_type = EntryType::Service;
        } else if line.starts_with("# sdtab:type=timer") {
            unit_type = EntryType::Timer;
        }
        if let Some(cron) = line.strip_prefix("# sdtab:cron=") {
            cron_expr = cron.to_string();
            has_cron = true;
        }
        if let Some(cmd) = line.strip_prefix("ExecStart=") {
            command = cmd.to_string();
        }
    }

    // Backward compatibility: if no type= line but has cron= → timer
    if has_cron {
        // Already defaults to Timer, no change needed
    }

    ServiceMetadata {
        unit_type,
        cron_expr,
        command,
    }
}

fn format_next_run(raw: &str) -> String {
    if raw.is_empty() || raw == "n/a" {
        return "-".to_string();
    }
    raw.to_string()
}
