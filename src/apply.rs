use std::collections::{BTreeMap, HashSet};
use std::fs;

use anyhow::{Context, Result};

use crate::sdtabfile::{self, Sdtabfile, ServiceEntry, TimerEntry};
use crate::{add, parse_unit, remove};

enum DiffStatus {
    Added,
    Changed,
    Unchanged,
    Removed,
}

struct DiffEntry {
    name: String,
    unit_type: parse_unit::UnitType,
    status: DiffStatus,
}

pub fn run(file: &str, prune: bool, dry_run: bool) -> Result<()> {
    let toml_content = fs::read_to_string(file)
        .with_context(|| format!("Failed to read {}", file))?;
    let sdtabfile: Sdtabfile = toml::from_str(&toml_content)
        .with_context(|| format!("Failed to parse {}", file))?;

    let current_units = parse_unit::scan_all_units()?;
    let mut current_map: BTreeMap<String, &parse_unit::ParsedUnit> = BTreeMap::new();
    for unit in &current_units {
        current_map.insert(unit.name.clone(), unit);
    }

    let mut diff_entries: Vec<DiffEntry> = Vec::new();
    let mut desired_names: HashSet<String> = HashSet::new();

    // Process timers
    for (name, entry) in &sdtabfile.timers {
        desired_names.insert(name.clone());

        let status = match current_map.get(name) {
            None => DiffStatus::Added,
            Some(current) => {
                if timer_matches(current, entry) {
                    DiffStatus::Unchanged
                } else {
                    DiffStatus::Changed
                }
            }
        };

        diff_entries.push(DiffEntry {
            name: name.clone(),
            unit_type: parse_unit::UnitType::Timer,
            status,
        });
    }

    // Process services
    for (name, entry) in &sdtabfile.services {
        desired_names.insert(name.clone());

        let status = match current_map.get(name) {
            None => DiffStatus::Added,
            Some(current) => {
                if service_matches(current, entry) {
                    DiffStatus::Unchanged
                } else {
                    DiffStatus::Changed
                }
            }
        };

        diff_entries.push(DiffEntry {
            name: name.clone(),
            unit_type: parse_unit::UnitType::Service,
            status,
        });
    }

    // Find units to prune
    for unit in &current_units {
        if !desired_names.contains(&unit.name) {
            diff_entries.push(DiffEntry {
                name: unit.name.clone(),
                unit_type: unit.unit_type.clone(),
                status: DiffStatus::Removed,
            });
        }
    }

    // Display summary
    let mut added = 0;
    let mut changed = 0;
    let mut unchanged = 0;
    let mut removed = 0;

    for entry in &diff_entries {
        let type_label = entry.unit_type.label();
        match entry.status {
            DiffStatus::Added => {
                println!("  + {} ({})", entry.name, type_label);
                added += 1;
            }
            DiffStatus::Changed => {
                println!("  ~ {} ({})", entry.name, type_label);
                changed += 1;
            }
            DiffStatus::Unchanged => {
                println!("  = {} ({})", entry.name, type_label);
                unchanged += 1;
            }
            DiffStatus::Removed => {
                if prune {
                    println!("  - {} ({})", entry.name, type_label);
                    removed += 1;
                }
            }
        }
    }

    // Show warning for unmanaged units when not pruning
    if !prune {
        let orphans: Vec<&DiffEntry> = diff_entries
            .iter()
            .filter(|e| matches!(e.status, DiffStatus::Removed))
            .collect();
        if !orphans.is_empty() {
            println!();
            println!("Warning: the following units are not in the file:");
            for entry in &orphans {
                println!("  {} ({})", entry.name, entry.unit_type.label());
            }
            println!("Use --prune to remove them.");
        }
    }

    println!();

    if dry_run {
        println!(
            "Dry run: {} to add, {} to update, {} unchanged, {} to remove",
            added, changed, unchanged, removed
        );
        return Ok(());
    }

    if added == 0 && changed == 0 && removed == 0 {
        println!("Nothing to do. All {} unit(s) are up to date.", unchanged);
        return Ok(());
    }

    // Apply changes
    for entry in &diff_entries {
        match entry.status {
            DiffStatus::Unchanged => {}
            DiffStatus::Changed => {
                remove::run(&entry.name)?;
                apply_entry(&sdtabfile, &entry.name, &entry.unit_type)?;
            }
            DiffStatus::Added => {
                apply_entry(&sdtabfile, &entry.name, &entry.unit_type)?;
            }
            DiffStatus::Removed => {
                if prune {
                    remove::run(&entry.name)?;
                }
            }
        }
    }

    println!();
    println!(
        "Applied: {} added, {} updated, {} unchanged, {} removed",
        added, changed, unchanged, removed
    );

    Ok(())
}

fn apply_entry(sdtabfile: &Sdtabfile, name: &str, unit_type: &parse_unit::UnitType) -> Result<()> {
    match unit_type {
        parse_unit::UnitType::Timer => {
            let entry = &sdtabfile.timers[name];
            add::run(add::AddOptions {
                schedule: entry.schedule.clone(),
                command: entry.command.clone(),
                name: Some(name.to_string()),
                workdir: Some(entry.workdir.clone()),
                description: entry.description.clone(),
                env_file: None,
                restart: None,
                memory_max: entry.memory_max.clone(),
                cpu_quota: entry.cpu_quota.clone(),
                io_weight: entry.io_weight.clone(),
                timeout_stop: entry.timeout_stop.clone(),
                exec_start_pre: entry.exec_start_pre.clone(),
                exec_stop_post: entry.exec_stop_post.clone(),
                log_level_max: entry.log_level_max.clone(),
                random_delay: entry.random_delay.clone(),
                env: entry.env.clone(),
            })?;
        }
        parse_unit::UnitType::Service => {
            let entry = &sdtabfile.services[name];
            add::run(add::AddOptions {
                schedule: "@service".to_string(),
                command: entry.command.clone(),
                name: Some(name.to_string()),
                workdir: Some(entry.workdir.clone()),
                description: entry.description.clone(),
                env_file: entry.env_file.clone(),
                restart: entry.restart.clone(),
                memory_max: entry.memory_max.clone(),
                cpu_quota: entry.cpu_quota.clone(),
                io_weight: entry.io_weight.clone(),
                timeout_stop: entry.timeout_stop.clone(),
                exec_start_pre: entry.exec_start_pre.clone(),
                exec_stop_post: entry.exec_stop_post.clone(),
                log_level_max: entry.log_level_max.clone(),
                random_delay: None,
                env: entry.env.clone(),
            })?;
        }
    }
    Ok(())
}

fn timer_matches(current: &parse_unit::ParsedUnit, desired: &TimerEntry) -> bool {
    let cron = current.cron_expr.as_deref().unwrap_or("");
    cron == desired.schedule
        && current.command == desired.command
        && current.workdir == desired.workdir
        && sdtabfile::desc_matches(&current.description, &current.command, &desired.description)
        && current.memory_max == desired.memory_max
        && current.cpu_quota == desired.cpu_quota
        && current.io_weight == desired.io_weight
        && current.timeout_stop == desired.timeout_stop
        && current.exec_start_pre == desired.exec_start_pre
        && current.exec_stop_post == desired.exec_stop_post
        && current.log_level_max == desired.log_level_max
        && current.random_delay == desired.random_delay
        && current.env == desired.env
}

fn service_matches(current: &parse_unit::ParsedUnit, desired: &ServiceEntry) -> bool {
    let current_restart = current.restart_policy.as_deref().unwrap_or("always");
    let desired_restart = desired.restart.as_deref().unwrap_or("always");
    current.command == desired.command
        && current.workdir == desired.workdir
        && sdtabfile::desc_matches(&current.description, &current.command, &desired.description)
        && current_restart == desired_restart
        && current.env_file == desired.env_file
        && current.memory_max == desired.memory_max
        && current.cpu_quota == desired.cpu_quota
        && current.io_weight == desired.io_weight
        && current.timeout_stop == desired.timeout_stop
        && current.exec_start_pre == desired.exec_start_pre
        && current.exec_stop_post == desired.exec_stop_post
        && current.log_level_max == desired.log_level_max
        && current.env == desired.env
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_toml_timers() {
        let toml = r#"
[timers.report]
schedule = "0 9 * * *"
command = "uv run ./report.py"
workdir = "/home/user/project"
description = "daily report"
memory_max = "512M"

[timers.backup]
schedule = "@daily/3"
command = "./backup.sh"
workdir = "/home/user"
"#;
        let sdtabfile: Sdtabfile = toml::from_str(toml).unwrap();
        assert_eq!(sdtabfile.timers.len(), 2);

        let report = &sdtabfile.timers["report"];
        assert_eq!(report.schedule, "0 9 * * *");
        assert_eq!(report.command, "uv run ./report.py");
        assert_eq!(report.description, Some("daily report".to_string()));
        assert_eq!(report.memory_max, Some("512M".to_string()));

        let backup = &sdtabfile.timers["backup"];
        assert_eq!(backup.schedule, "@daily/3");
        assert!(backup.description.is_none());
        assert!(backup.memory_max.is_none());
    }

    #[test]
    fn test_parse_toml_services() {
        let toml = r#"
[services.web]
command = "node dist/index.js"
workdir = "/home/user/project"
description = "Web Server"
restart = "on-failure"
env_file = "/home/user/.env"
memory_max = "256M"
env = ["NODE_ENV=production"]

[services.bot]
command = "python bot.py"
workdir = "/home/user"
"#;
        let sdtabfile: Sdtabfile = toml::from_str(toml).unwrap();
        assert_eq!(sdtabfile.services.len(), 2);

        let web = &sdtabfile.services["web"];
        assert_eq!(web.restart, Some("on-failure".to_string()));
        assert_eq!(web.env_file, Some("/home/user/.env".to_string()));
        assert_eq!(web.env, vec!["NODE_ENV=production"]);

        let bot = &sdtabfile.services["bot"];
        assert!(bot.restart.is_none());
        assert!(bot.env.is_empty());
    }

    #[test]
    fn test_parse_toml_mixed() {
        let toml = r#"
[timers.report]
schedule = "0 9 * * *"
command = "./report.sh"
workdir = "/home/user"

[services.web]
command = "node index.js"
workdir = "/home/user/app"
"#;
        let sdtabfile: Sdtabfile = toml::from_str(toml).unwrap();
        assert_eq!(sdtabfile.timers.len(), 1);
        assert_eq!(sdtabfile.services.len(), 1);
    }

    #[test]
    fn test_parse_toml_empty() {
        let toml = "";
        let sdtabfile: Sdtabfile = toml::from_str(toml).unwrap();
        assert!(sdtabfile.timers.is_empty());
        assert!(sdtabfile.services.is_empty());
    }

    #[test]
    fn test_desc_matches() {
        assert!(sdtabfile::desc_matches("echo hello", "echo hello", &None));
        assert!(sdtabfile::desc_matches("daily report", "echo hello", &Some("daily report".to_string())));
        assert!(!sdtabfile::desc_matches("wrong", "echo hello", &Some("daily report".to_string())));
        assert!(!sdtabfile::desc_matches("different", "echo hello", &None));
    }
}
