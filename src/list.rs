use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::{init, systemctl};

struct TimerEntry {
    name: String,
    cron_expr: String,
    command: String,
    next_run: String,
}

pub fn run() -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    if !dir_path.exists() {
        println!("No timers found. Run 'sdtab init' first.");
        return Ok(());
    }

    let mut entries: Vec<TimerEntry> = Vec::new();

    // Find all sdtab-*.timer files
    let read_dir = fs::read_dir(dir_path)?;
    for entry in read_dir {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();

        if !filename.starts_with("sdtab-") || !filename.ends_with(".timer") {
            continue;
        }

        let name = filename
            .strip_prefix("sdtab-")
            .unwrap()
            .strip_suffix(".timer")
            .unwrap()
            .to_string();

        // Read corresponding service file for metadata
        let service_path = dir_path.join(format!("sdtab-{}.service", name));
        let (cron_expr, command) = if service_path.exists() {
            parse_service_metadata(&fs::read_to_string(&service_path)?)
        } else {
            ("?".to_string(), "?".to_string())
        };

        // Get next run time from systemctl
        let timer_unit = format!("sdtab-{}.timer", name);
        let next_run = systemctl::show_property(&timer_unit, "NextElapseUSecRealtime")
            .unwrap_or_else(|_| "?".to_string());
        let next_run = format_next_run(&next_run);

        entries.push(TimerEntry {
            name,
            cron_expr,
            command,
            next_run,
        });
    }

    if entries.is_empty() {
        println!("No timers found.");
        return Ok(());
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));

    // Print table
    let name_width = entries.iter().map(|e| e.name.len()).max().unwrap_or(4).max(4);
    let cron_width = entries
        .iter()
        .map(|e| e.cron_expr.len())
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
        "{:<name_w$}  {:<cron_w$}  {:<cmd_w$}  NEXT",
        "NAME",
        "SCHEDULE",
        "COMMAND",
        name_w = name_width,
        cron_w = cron_width,
        cmd_w = cmd_width,
    );

    for entry in &entries {
        println!(
            "{:<name_w$}  {:<cron_w$}  {:<cmd_w$}  {}",
            entry.name,
            entry.cron_expr,
            entry.command,
            entry.next_run,
            name_w = name_width,
            cron_w = cron_width,
            cmd_w = cmd_width,
        );
    }

    Ok(())
}

fn parse_service_metadata(content: &str) -> (String, String) {
    let mut cron_expr = "?".to_string();
    let mut command = "?".to_string();

    for line in content.lines() {
        if let Some(cron) = line.strip_prefix("# sdtab:cron=") {
            cron_expr = cron.to_string();
        }
        if let Some(cmd) = line.strip_prefix("ExecStart=") {
            command = cmd.to_string();
        }
    }

    (cron_expr, command)
}

fn format_next_run(raw: &str) -> String {
    if raw.is_empty() || raw == "n/a" {
        return "-".to_string();
    }
    // systemctl show returns the timestamp directly; just trim it
    raw.to_string()
}
