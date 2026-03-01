use std::collections::BTreeMap;
use std::fs;

use anyhow::{Context, Result};

use crate::parse_unit;
use crate::sdtabfile::{self, Sdtabfile, ServiceEntry, TimerEntry};

pub fn run(output: Option<&str>) -> Result<()> {
    let units = parse_unit::scan_all_units()?;

    let mut timers = BTreeMap::new();
    let mut services = BTreeMap::new();

    for unit in units {
        match unit.unit_type {
            parse_unit::UnitType::Timer => {
                let schedule = unit.cron_expr.unwrap_or_else(|| "?".to_string());
                let description = sdtabfile::description_if_different(&unit.description, &unit.command);
                timers.insert(
                    unit.name,
                    TimerEntry {
                        schedule,
                        command: unit.command,
                        workdir: unit.workdir,
                        description,
                        memory_max: unit.memory_max,
                        cpu_quota: unit.cpu_quota,
                        io_weight: unit.io_weight,
                        timeout_stop: unit.timeout_stop,
                        exec_start_pre: unit.exec_start_pre,
                        exec_stop_post: unit.exec_stop_post,
                        log_level_max: unit.log_level_max,
                        random_delay: unit.random_delay,
                        env: unit.env,
                    },
                );
            }
            parse_unit::UnitType::Service => {
                let description = sdtabfile::description_if_different(&unit.description, &unit.command);
                services.insert(
                    unit.name,
                    ServiceEntry {
                        command: unit.command,
                        workdir: unit.workdir,
                        description,
                        restart: unit.restart_policy,
                        env_file: unit.env_file,
                        memory_max: unit.memory_max,
                        cpu_quota: unit.cpu_quota,
                        io_weight: unit.io_weight,
                        timeout_stop: unit.timeout_stop,
                        exec_start_pre: unit.exec_start_pre,
                        exec_stop_post: unit.exec_stop_post,
                        log_level_max: unit.log_level_max,
                        env: unit.env,
                    },
                );
            }
        }
    }

    let sdtabfile = Sdtabfile { timers, services };
    let toml_str = toml::to_string_pretty(&sdtabfile)
        .context("Failed to serialize to TOML")?;

    match output {
        Some(path) => {
            fs::write(path, &toml_str)
                .with_context(|| format!("Failed to write {}", path))?;
            println!("Exported to: {}", path);
        }
        None => {
            print!("{}", toml_str);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_description_if_different() {
        assert_eq!(sdtabfile::description_if_different("echo hello", "echo hello"), None);
        assert_eq!(
            sdtabfile::description_if_different("daily report", "uv run ./report.py"),
            Some("daily report".to_string())
        );
    }

    #[test]
    fn test_serialize_timer_minimal() {
        let mut timers = BTreeMap::new();
        timers.insert(
            "report".to_string(),
            TimerEntry {
                schedule: "0 9 * * *".to_string(),
                command: "uv run ./report.py".to_string(),
                workdir: "/home/user/project".to_string(),
                description: Some("daily report".to_string()),
                memory_max: None,
                cpu_quota: None,
                io_weight: None,
                timeout_stop: None,
                exec_start_pre: None,
                exec_stop_post: None,
                log_level_max: None,
                random_delay: None,
                env: vec![],
            },
        );
        let sdtabfile = Sdtabfile {
            timers,
            services: BTreeMap::new(),
        };
        let toml_str = toml::to_string_pretty(&sdtabfile).unwrap();
        assert!(toml_str.contains("[timers.report]"));
        assert!(toml_str.contains("schedule = \"0 9 * * *\""));
        assert!(toml_str.contains("description = \"daily report\""));
        assert!(!toml_str.contains("memory_max"));
    }
}
