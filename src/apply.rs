use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::sdtabfile::{self, Sdtabfile, ServiceEntry, TimerEntry};
use crate::{add, cron, init, parse_unit, remove, systemctl, unit};

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
    let mut needs_reload = false;
    for entry in &diff_entries {
        match entry.status {
            DiffStatus::Unchanged => {}
            DiffStatus::Changed => {
                update_entry(&sdtabfile, &entry.name, &entry.unit_type)?;
                needs_reload = true;
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

    // Reload once after all file writes, then restart only units that need it
    if needs_reload {
        systemctl::daemon_reload()?;
        for entry in &diff_entries {
            if !matches!(entry.status, DiffStatus::Changed) {
                continue;
            }
            let restart_needed = match entry.unit_type {
                parse_unit::UnitType::Timer => {
                    current_map.get(&entry.name).is_none_or(|current| {
                        timer_needs_restart(current, &sdtabfile.timers[&entry.name])
                    })
                }
                parse_unit::UnitType::Service => {
                    current_map.get(&entry.name).is_none_or(|current| {
                        service_needs_restart(current, &sdtabfile.services[&entry.name])
                    })
                }
            };
            if restart_needed {
                let unit_name = match entry.unit_type {
                    parse_unit::UnitType::Timer => unit::timer_filename(&entry.name),
                    parse_unit::UnitType::Service => unit::service_filename(&entry.name),
                };
                systemctl::restart(&unit_name)?;
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

/// Update an existing unit by overwriting files in-place (no stop/remove).
/// After all updates, the caller does a single daemon-reload + restart.
fn update_entry(sdtabfile: &Sdtabfile, name: &str, unit_type: &parse_unit::UnitType) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    match unit_type {
        parse_unit::UnitType::Timer => {
            let entry = &sdtabfile.timers[name];
            let parsed = cron::parse(&entry.schedule)?;
            let workdir = entry.workdir.clone();
            let resolved_command = init::resolve_command(&entry.command)?;
            let description = entry.description.clone().unwrap_or_else(|| entry.command.clone());
            let display_schedule = parsed.display.clone().unwrap_or_else(|| entry.schedule.clone());
            let original_command = if resolved_command != entry.command {
                Some(entry.command.clone())
            } else {
                None
            };

            let config = unit::UnitConfig {
                name: name.to_string(),
                command: resolved_command,
                workdir,
                description,
                cron_expr: Some(display_schedule),
                schedule: Some(parsed),
                restart_policy: None,
                env_file: entry.env_file.clone(),
                memory_max: entry.memory_max.clone(),
                cpu_quota: entry.cpu_quota.clone(),
                io_weight: entry.io_weight.clone(),
                timeout_stop: entry.timeout_stop.clone(),
                exec_start_pre: entry.exec_start_pre.clone(),
                exec_stop_post: entry.exec_stop_post.clone(),
                log_level_max: entry.log_level_max.clone(),
                random_delay: entry.random_delay.clone(),
                env: entry.env.clone(),
                original_command,
            };

            let service_path = dir_path.join(unit::service_filename(name));
            fs::write(&service_path, unit::generate_service(&config))
                .with_context(|| format!("Failed to write {}", service_path.display()))?;

            let timer_path = dir_path.join(unit::timer_filename(name));
            fs::write(&timer_path, unit::generate_timer(&config))
                .with_context(|| format!("Failed to write {}", timer_path.display()))?;
        }
        parse_unit::UnitType::Service => {
            let entry = &sdtabfile.services[name];
            let workdir = entry.workdir.clone();
            let resolved_command = init::resolve_command(&entry.command)?;
            let description = entry.description.clone().unwrap_or_else(|| entry.command.clone());
            let original_command = if resolved_command != entry.command {
                Some(entry.command.clone())
            } else {
                None
            };

            let config = unit::UnitConfig {
                name: name.to_string(),
                command: resolved_command,
                workdir,
                description,
                cron_expr: None,
                schedule: None,
                restart_policy: entry.restart.clone(),
                env_file: entry.env_file.clone(),
                memory_max: entry.memory_max.clone(),
                cpu_quota: entry.cpu_quota.clone(),
                io_weight: entry.io_weight.clone(),
                timeout_stop: entry.timeout_stop.clone(),
                exec_start_pre: entry.exec_start_pre.clone(),
                exec_stop_post: entry.exec_stop_post.clone(),
                log_level_max: entry.log_level_max.clone(),
                random_delay: None,
                env: entry.env.clone(),
                original_command,
            };

            let service_path = dir_path.join(unit::service_filename(name));
            fs::write(&service_path, unit::generate_daemon_service(&config))
                .with_context(|| format!("Failed to write {}", service_path.display()))?;
        }
    }
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
                env_file: entry.env_file.clone(),
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
                dry_run: false,
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
                dry_run: false,
            })?;
        }
    }
    Ok(())
}

/// Timer schedule or random_delay changed → need to restart the .timer unit.
/// Service-only changes (command, env, etc.) are picked up on next trigger via daemon-reload.
fn timer_needs_restart(current: &parse_unit::ParsedUnit, desired: &TimerEntry) -> bool {
    let cron = current.cron_expr.as_deref().unwrap_or("");
    cron != desired.schedule || current.random_delay != desired.random_delay
}

/// Anything other than description changed → need to restart the service.
fn service_needs_restart(current: &parse_unit::ParsedUnit, desired: &ServiceEntry) -> bool {
    let current_restart = current.restart_policy.as_deref().unwrap_or("always");
    let desired_restart = desired.restart.as_deref().unwrap_or("always");
    current.command != desired.command
        || current.workdir != desired.workdir
        || current_restart != desired_restart
        || current.env_file != desired.env_file
        || current.memory_max != desired.memory_max
        || current.cpu_quota != desired.cpu_quota
        || current.io_weight != desired.io_weight
        || current.timeout_stop != desired.timeout_stop
        || current.exec_start_pre != desired.exec_start_pre
        || current.exec_stop_post != desired.exec_stop_post
        || current.log_level_max != desired.log_level_max
        || current.env != desired.env
}

fn timer_matches(current: &parse_unit::ParsedUnit, desired: &TimerEntry) -> bool {
    let cron = current.cron_expr.as_deref().unwrap_or("");
    cron == desired.schedule
        && current.command == desired.command
        && current.workdir == desired.workdir
        && sdtabfile::desc_matches(&current.description, &current.command, &desired.description)
        && current.env_file == desired.env_file
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

    fn make_parsed_unit(name: &str, unit_type: parse_unit::UnitType) -> parse_unit::ParsedUnit {
        parse_unit::ParsedUnit {
            name: name.to_string(),
            unit_type,
            command: "./run.sh".to_string(),
            workdir: "/home/user".to_string(),
            description: "./run.sh".to_string(),
            cron_expr: Some("0 9 * * *".to_string()),
            restart_policy: None,
            env_file: None,
            memory_max: None,
            cpu_quota: None,
            io_weight: None,
            timeout_stop: None,
            exec_start_pre: None,
            exec_stop_post: None,
            log_level_max: None,
            random_delay: None,
            env: vec![],
        }
    }

    fn make_timer_entry() -> TimerEntry {
        TimerEntry {
            schedule: "0 9 * * *".to_string(),
            command: "./run.sh".to_string(),
            workdir: "/home/user".to_string(),
            description: None,
            env_file: None,
            memory_max: None,
            cpu_quota: None,
            io_weight: None,
            timeout_stop: None,
            exec_start_pre: None,
            exec_stop_post: None,
            log_level_max: None,
            random_delay: None,
            env: vec![],
        }
    }

    fn make_service_entry() -> ServiceEntry {
        ServiceEntry {
            command: "./run.sh".to_string(),
            workdir: "/home/user".to_string(),
            description: None,
            restart: None,
            env_file: None,
            memory_max: None,
            cpu_quota: None,
            io_weight: None,
            timeout_stop: None,
            exec_start_pre: None,
            exec_stop_post: None,
            log_level_max: None,
            env: vec![],
        }
    }

    #[test]
    fn test_timer_needs_restart_schedule_changed() {
        let current = make_parsed_unit("report", parse_unit::UnitType::Timer);
        let mut desired = make_timer_entry();
        desired.schedule = "0 10 * * *".to_string();
        assert!(timer_needs_restart(&current, &desired));
    }

    #[test]
    fn test_timer_needs_restart_random_delay_changed() {
        let current = make_parsed_unit("report", parse_unit::UnitType::Timer);
        let mut desired = make_timer_entry();
        desired.random_delay = Some("5m".to_string());
        assert!(timer_needs_restart(&current, &desired));
    }

    #[test]
    fn test_timer_no_restart_command_changed() {
        // Command change in a timer only affects the .service file,
        // which is picked up on next trigger — no timer restart needed
        let current = make_parsed_unit("report", parse_unit::UnitType::Timer);
        let mut desired = make_timer_entry();
        desired.command = "./new-run.sh".to_string();
        assert!(!timer_needs_restart(&current, &desired));
    }

    #[test]
    fn test_timer_no_restart_description_changed() {
        let current = make_parsed_unit("report", parse_unit::UnitType::Timer);
        let desired = TimerEntry {
            description: Some("new description".to_string()),
            ..make_timer_entry()
        };
        assert!(!timer_needs_restart(&current, &desired));
    }

    #[test]
    fn test_service_needs_restart_command_changed() {
        let current = make_parsed_unit("web", parse_unit::UnitType::Service);
        let mut desired = make_service_entry();
        desired.command = "node new-server.js".to_string();
        assert!(service_needs_restart(&current, &desired));
    }

    #[test]
    fn test_service_needs_restart_env_changed() {
        let current = make_parsed_unit("web", parse_unit::UnitType::Service);
        let mut desired = make_service_entry();
        desired.env = vec!["FOO=bar".to_string()];
        assert!(service_needs_restart(&current, &desired));
    }

    #[test]
    fn test_service_no_restart_description_only() {
        let current = make_parsed_unit("web", parse_unit::UnitType::Service);
        let desired = ServiceEntry {
            description: Some("new description".to_string()),
            ..make_service_entry()
        };
        assert!(!service_needs_restart(&current, &desired));
    }
}
