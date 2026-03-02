use anyhow::Result;

use crate::{parse_unit, systemctl, unit};

struct Entry {
    name: String,
    type_str: &'static str,
    schedule: String,
    command: String,
    status: String,
}

pub fn run(json: bool) -> Result<()> {
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
        let (type_str, schedule, status) = match unit.unit_type {
            parse_unit::UnitType::Service => {
                let service_unit = unit::service_filename(&unit.name);
                let active_state = systemctl::show_property(&service_unit, "ActiveState")
                    .unwrap_or_else(|_| "unknown".to_string());
                ("service", "@service".to_string(), active_state)
            }
            parse_unit::UnitType::Timer => {
                let timer_unit = unit::timer_filename(&unit.name);
                let next_run =
                    systemctl::show_property(&timer_unit, "NextElapseUSecRealtime")
                        .unwrap_or_else(|_| "?".to_string());
                let next_run = format_next_run(&next_run);
                let schedule = unit.cron_expr.as_deref().unwrap_or("?").to_string();
                ("timer", schedule, next_run)
            }
        };

        entries.push(Entry {
            name: unit.name.clone(),
            type_str,
            schedule,
            command: unit.command.clone(),
            status,
        });
    }

    if json {
        print_json(&entries);
    } else {
        print_table(&entries);
    }

    Ok(())
}

fn print_table(entries: &[Entry]) {
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

    for entry in entries {
        println!(
            "{:<name_w$}  {:<type_w$}  {:<sched_w$}  {:<cmd_w$}  {}",
            entry.name,
            entry.type_str,
            entry.schedule,
            entry.command,
            entry.status,
            name_w = name_width,
            type_w = type_width,
            sched_w = sched_width,
            cmd_w = cmd_width,
        );
    }
}

fn print_json(entries: &[Entry]) {
    println!("[");
    for (i, entry) in entries.iter().enumerate() {
        let comma = if i + 1 < entries.len() { "," } else { "" };
        println!(
            "  {{\"name\":{},\"type\":\"{}\",\"schedule\":{},\"command\":{},\"status\":{}}}{}",
            json_str(&entry.name),
            entry.type_str,
            json_str(&entry.schedule),
            json_str(&entry.command),
            json_str(&entry.status),
            comma,
        );
    }
    println!("]");
}

fn json_str(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

fn format_next_run(raw: &str) -> String {
    if raw.is_empty() || raw == "n/a" {
        return "-".to_string();
    }
    raw.to_string()
}
