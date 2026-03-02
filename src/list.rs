use std::io::IsTerminal;

use anyhow::Result;
use serde::Serialize;

use crate::{parse_unit, systemctl, unit, SortOrder};

#[derive(Serialize)]
struct Entry {
    name: String,
    #[serde(rename = "type")]
    type_str: &'static str,
    schedule: String,
    command: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    sort_key: u64, // epoch usec for time sort (0 = service/unknown)
}

pub fn run(json: bool, sort: SortOrder) -> Result<()> {
    let units = parse_unit::scan_all_units()?;

    if units.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No timers or services found.");
        }
        return Ok(());
    }

    let mut entries: Vec<Entry> = Vec::new();

    for unit in &units {
        let (type_str, schedule, status, sort_key) = match unit.unit_type {
            parse_unit::UnitType::Service => {
                let service_unit = unit::service_filename(&unit.name);
                let active_state = systemctl::show_property(&service_unit, "ActiveState")
                    .unwrap_or_else(|_| "unknown".to_string());
                ("service", "@service".to_string(), active_state, 0u64)
            }
            parse_unit::UnitType::Timer => {
                let timer_unit = unit::timer_filename(&unit.name);
                let next_run_raw =
                    systemctl::show_property(&timer_unit, "NextElapseUSecRealtime")
                        .unwrap_or_else(|_| "?".to_string());
                let next_run = format_next_run(&next_run_raw);
                let schedule = unit.cron_expr.as_deref().unwrap_or("?").to_string();
                let epoch = parse_datetime_sort_key(&next_run_raw);
                ("timer", schedule, next_run, epoch)
            }
        };

        // description がコマンドと同じなら表示しない
        let description = if unit.description != unit.command {
            Some(unit.description.clone())
        } else {
            None
        };

        entries.push(Entry {
            name: unit.name.clone(),
            type_str,
            schedule,
            command: unit.command.clone(),
            status,
            description,
            sort_key,
        });
    }

    // Sort
    match sort {
        SortOrder::Time => {
            // Services (sort_key=0) first, then timers by next run time
            entries.sort_by(|a, b| {
                let type_ord = a.sort_key.cmp(&b.sort_key);
                if type_ord == std::cmp::Ordering::Equal {
                    a.name.cmp(&b.name)
                } else {
                    type_ord
                }
            });
        }
        SortOrder::Name => {
            entries.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }

    if json {
        print_json(&entries);
    } else {
        print_table(&entries);
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max - 1;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

fn format_status(status: &str, use_color: bool) -> String {
    let (marker, color_code) = match status {
        "active" => ("●", "\x1b[32m"),   // green
        "failed" => ("●", "\x1b[31m"),   // red
        "inactive" => ("○", "\x1b[33m"), // yellow
        _ => ("○", "\x1b[90m"),          // gray
    };

    if use_color {
        format!("{color_code}{marker}\x1b[0m {status}")
    } else {
        format!("{marker} {status}")
    }
}

fn print_table(entries: &[Entry]) {
    let use_color = std::io::stdout().is_terminal();

    let name_width = entries.iter().map(|e| e.name.len()).max().unwrap_or(4).max(4);
    let type_width = 7; // "service" is the longest
    let sched_width = entries
        .iter()
        .map(|e| e.schedule.len())
        .max()
        .unwrap_or(8)
        .max(8);
    let cmd_max = 40;
    let cmd_width = entries
        .iter()
        .map(|e| e.command.len().min(cmd_max))
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

    for entry in entries {
        let cmd = truncate(&entry.command, cmd_max);
        let status = format_status(&entry.status, use_color);
        println!(
            "{:<name_w$}  {:<type_w$}  {:<sched_w$}  {:<cmd_w$}  {}",
            entry.name,
            entry.type_str,
            entry.schedule,
            cmd,
            status,
            name_w = name_width,
            type_w = type_width,
            sched_w = sched_width,
            cmd_w = cmd_width,
        );

        if let Some(desc) = &entry.description {
            if use_color {
                println!("  \x1b[90m{desc}\x1b[0m");
            } else {
                println!("  {desc}");
            }
        }
    }
}

fn print_json(entries: &[Entry]) {
    let json = serde_json::to_string_pretty(entries).expect("Failed to serialize JSON");
    println!("{}", json);
}

fn format_next_run(raw: &str) -> String {
    if raw.is_empty() || raw == "n/a" {
        return "-".to_string();
    }
    raw.to_string()
}

/// Parse systemd's NextElapseUSecRealtime datetime string into a sortable u64.
/// Input format: "Wed 2026-03-04 02:00:00 JST" -> 20260304020000
fn parse_datetime_sort_key(raw: &str) -> u64 {
    // Try to extract "YYYY-MM-DD HH:MM:SS" from the string
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() >= 3 {
        // parts[1] = "2026-03-04", parts[2] = "02:00:00"
        let date_nums: String = parts[1].replace('-', "");
        let time_nums: String = parts[2].replace(':', "");
        if let Ok(val) = format!("{}{}", date_nums, time_nums).parse::<u64>() {
            return val;
        }
    }
    u64::MAX // unknown = sort last
}
