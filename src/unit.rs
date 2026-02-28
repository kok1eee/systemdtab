use crate::cron::CronSchedule;

pub struct UnitConfig {
    pub name: String,
    pub command: String,
    pub workdir: String,
    pub description: String,
    pub cron_expr: String,
    pub schedule: CronSchedule,
}

pub fn generate_service(config: &UnitConfig) -> String {
    format!(
        "# sdtab:cron={cron}\n\
         [Unit]\n\
         Description=[sdtab] {name}: {desc}\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         ExecStart={command}\n\
         WorkingDirectory={workdir}\n",
        cron = config.cron_expr,
        name = config.name,
        desc = config.description,
        command = config.command,
        workdir = config.workdir,
    )
}

pub fn generate_timer(config: &UnitConfig) -> String {
    let trigger = if let Some(ref cal) = config.schedule.on_calendar {
        format!("OnCalendar={}", cal)
    } else if let Some(ref boot) = config.schedule.on_boot_sec {
        format!("OnBootSec={}", boot)
    } else {
        unreachable!("CronSchedule must have either on_calendar or on_boot_sec");
    };

    format!(
        "[Unit]\n\
         Description=[sdtab] {name} timer\n\
         \n\
         [Timer]\n\
         {trigger}\n\
         Persistent=true\n\
         \n\
         [Install]\n\
         WantedBy=timers.target\n",
        name = config.name,
        trigger = trigger,
    )
}

pub fn service_filename(name: &str) -> String {
    format!("sdtab-{}.service", name)
}

pub fn timer_filename(name: &str) -> String {
    format!("sdtab-{}.timer", name)
}

/// Extract a timer name from a command string.
/// e.g. "uv run ./report.py" → "report"
///      "python script.py" → "script"
///      "./my-tool --flag" → "my-tool"
pub fn derive_name(command: &str) -> String {
    let parts: Vec<&str> = command.split_whitespace().collect();

    // Find the best candidate: skip common runners, take the first path-like argument
    let candidate = if parts.len() >= 2 {
        let runners = ["python", "python3", "uv", "node", "bash", "sh", "ruby", "perl"];
        let first = parts[0].rsplit('/').next().unwrap_or(parts[0]);

        if runners.contains(&first) {
            // For "uv run ./report.py", skip "uv" and "run" and take "report.py"
            if first == "uv" && parts.len() >= 3 && parts[1] == "run" {
                parts[2]
            } else {
                parts[1]
            }
        } else {
            parts[0]
        }
    } else {
        parts[0]
    };

    // Extract basename, remove extension
    let basename = candidate.rsplit('/').next().unwrap_or(candidate);
    let name = basename
        .strip_suffix(".py")
        .or_else(|| basename.strip_suffix(".sh"))
        .or_else(|| basename.strip_suffix(".rb"))
        .or_else(|| basename.strip_suffix(".js"))
        .or_else(|| basename.strip_suffix(".ts"))
        .unwrap_or(basename);

    // Remove leading dots/dashes
    let name = name.trim_start_matches('.').trim_start_matches('-');

    if name.is_empty() {
        "task".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_name_uv_run() {
        assert_eq!(derive_name("uv run ./report.py"), "report");
    }

    #[test]
    fn derive_name_python() {
        assert_eq!(derive_name("python script.py"), "script");
    }

    #[test]
    fn derive_name_direct_command() {
        assert_eq!(derive_name("./my-tool --flag"), "my-tool");
    }

    #[test]
    fn derive_name_simple_command() {
        assert_eq!(derive_name("echo hello"), "echo");
    }

    #[test]
    fn test_service_generation() {
        let config = UnitConfig {
            name: "report".to_string(),
            command: "uv run ./report.py".to_string(),
            workdir: "/home/user/project".to_string(),
            description: "daily report".to_string(),
            cron_expr: "0 9 * * *".to_string(),
            schedule: CronSchedule {
                on_calendar: Some("*-*-* 09:00:00".to_string()),
                on_boot_sec: None,
            },
        };

        let service = generate_service(&config);
        assert!(service.contains("# sdtab:cron=0 9 * * *"));
        assert!(service.contains("Description=[sdtab] report: daily report"));
        assert!(service.contains("ExecStart=uv run ./report.py"));
        assert!(service.contains("WorkingDirectory=/home/user/project"));
    }

    #[test]
    fn test_timer_generation_calendar() {
        let config = UnitConfig {
            name: "report".to_string(),
            command: "uv run ./report.py".to_string(),
            workdir: "/home/user/project".to_string(),
            description: "daily report".to_string(),
            cron_expr: "0 9 * * *".to_string(),
            schedule: CronSchedule {
                on_calendar: Some("*-*-* 09:00:00".to_string()),
                on_boot_sec: None,
            },
        };

        let timer = generate_timer(&config);
        assert!(timer.contains("OnCalendar=*-*-* 09:00:00"));
        assert!(timer.contains("Persistent=true"));
        assert!(timer.contains("WantedBy=timers.target"));
    }

    #[test]
    fn test_timer_generation_reboot() {
        let config = UnitConfig {
            name: "startup".to_string(),
            command: "./boot.sh".to_string(),
            workdir: "/home/user".to_string(),
            description: "run on boot".to_string(),
            cron_expr: "@reboot".to_string(),
            schedule: CronSchedule {
                on_calendar: None,
                on_boot_sec: Some("1min".to_string()),
            },
        };

        let timer = generate_timer(&config);
        assert!(timer.contains("OnBootSec=1min"));
    }
}
