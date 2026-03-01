use crate::cron::CronSchedule;
use crate::init;

pub enum UnitType {
    Timer,
    Service,
}

pub struct UnitConfig {
    pub name: String,
    pub command: String,
    pub workdir: String,
    pub description: String,
    #[allow(dead_code)]
    pub unit_type: UnitType,
    pub cron_expr: Option<String>,
    pub schedule: Option<CronSchedule>,
    pub restart_policy: Option<String>,
    pub env_file: Option<String>,
    pub memory_max: Option<String>,
    pub cpu_quota: Option<String>,
    pub io_weight: Option<String>,
    pub timeout_stop: Option<String>,
    pub exec_start_pre: Option<String>,
    pub exec_stop_post: Option<String>,
    pub log_level_max: Option<String>,
    pub random_delay: Option<String>,
    pub env: Vec<String>,
}

pub fn generate_service(config: &UnitConfig) -> String {
    let cron = config.cron_expr.as_deref().unwrap_or("");
    let resource_lines = generate_service_options(config);
    let global_env = global_env_line();
    format!(
        "# sdtab:type=timer\n\
         # sdtab:cron={cron}\n\
         [Unit]\n\
         Description=[sdtab] {name}: {desc}\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         ExecStart={command}\n\
         WorkingDirectory={workdir}\n\
         {global_env}\
         {resource_lines}",
        cron = cron,
        name = config.name,
        desc = config.description,
        command = config.command,
        workdir = config.workdir,
        global_env = global_env,
        resource_lines = resource_lines,
    )
}

pub fn generate_daemon_service(config: &UnitConfig) -> String {
    let restart = config
        .restart_policy
        .as_deref()
        .unwrap_or("always");
    let restart_meta = format!("# sdtab:restart={}\n", restart);

    let env_line = match &config.env_file {
        Some(path) => format!("EnvironmentFile={}\n", path),
        None => String::new(),
    };

    let resource_lines = generate_service_options(config);
    let global_env = global_env_line();

    format!(
        "# sdtab:type=service\n\
         {restart_meta}\
         [Unit]\n\
         Description=[sdtab] {name}: {desc}\n\
         After=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={command}\n\
         WorkingDirectory={workdir}\n\
         Restart={restart}\n\
         RestartSec=5\n\
         {global_env}\
         {env_line}\
         {resource_lines}\
         [Install]\n\
         WantedBy=default.target\n",
        restart_meta = restart_meta,
        name = config.name,
        desc = config.description,
        command = config.command,
        workdir = config.workdir,
        restart = restart,
        global_env = global_env,
        env_line = env_line,
        resource_lines = resource_lines,
    )
}

pub fn generate_timer(config: &UnitConfig) -> String {
    let schedule = config.schedule.as_ref().expect("Timer requires a schedule");
    let trigger = if let Some(ref cal) = schedule.on_calendar {
        format!("OnCalendar={}", cal)
    } else if let Some(ref boot) = schedule.on_boot_sec {
        format!("OnBootSec={}", boot)
    } else {
        unreachable!("CronSchedule must have either on_calendar or on_boot_sec");
    };

    let random_delay = match &config.random_delay {
        Some(val) => format!("RandomizedDelaySec={}\n", val),
        None => String::new(),
    };

    format!(
        "[Unit]\n\
         Description=[sdtab] {name} timer\n\
         \n\
         [Timer]\n\
         {trigger}\n\
         Persistent=true\n\
         {random_delay}\
         \n\
         [Install]\n\
         WantedBy=timers.target\n",
        name = config.name,
        trigger = trigger,
        random_delay = random_delay,
    )
}

fn global_env_line() -> String {
    match init::global_env_path() {
        Ok(path) => format!("EnvironmentFile=-{}\n", path),
        Err(_) => String::new(),
    }
}

fn generate_service_options(config: &UnitConfig) -> String {
    let mut lines = String::new();
    if let Some(ref val) = config.exec_start_pre {
        lines.push_str(&format!("ExecStartPre={}\n", val));
    }
    if let Some(ref val) = config.exec_stop_post {
        lines.push_str(&format!("ExecStopPost={}\n", val));
    }
    if let Some(ref val) = config.timeout_stop {
        lines.push_str(&format!("TimeoutStopSec={}\n", val));
    }
    if let Some(ref val) = config.memory_max {
        lines.push_str(&format!("MemoryMax={}\n", val));
    }
    if let Some(ref val) = config.cpu_quota {
        lines.push_str(&format!("CPUQuota={}\n", val));
    }
    if let Some(ref val) = config.io_weight {
        lines.push_str(&format!("IOWeight={}\n", val));
    }
    if let Some(ref val) = config.log_level_max {
        lines.push_str(&format!("LogLevelMax={}\n", val));
    }
    for env_val in &config.env {
        lines.push_str(&format!("Environment={}\n", env_val));
    }
    lines
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
            unit_type: UnitType::Timer,
            cron_expr: Some("0 9 * * *".to_string()),
            schedule: Some(CronSchedule {
                on_calendar: Some("*-*-* 09:00:00".to_string()),
                on_boot_sec: None,
                is_service: false,
                display: None,
            }),
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
        };

        let service = generate_service(&config);
        assert!(service.contains("# sdtab:type=timer"));
        assert!(service.contains("# sdtab:cron=0 9 * * *"));
        assert!(service.contains("Description=[sdtab] report: daily report"));
        assert!(service.contains("ExecStart=uv run ./report.py"));
        assert!(service.contains("WorkingDirectory=/home/user/project"));
        assert!(service.contains("Type=oneshot"));
    }

    #[test]
    fn test_timer_generation_calendar() {
        let config = UnitConfig {
            name: "report".to_string(),
            command: "uv run ./report.py".to_string(),
            workdir: "/home/user/project".to_string(),
            description: "daily report".to_string(),
            unit_type: UnitType::Timer,
            cron_expr: Some("0 9 * * *".to_string()),
            schedule: Some(CronSchedule {
                on_calendar: Some("*-*-* 09:00:00".to_string()),
                on_boot_sec: None,
                is_service: false,
                display: None,
            }),
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
            unit_type: UnitType::Timer,
            cron_expr: Some("@reboot".to_string()),
            schedule: Some(CronSchedule {
                on_calendar: None,
                on_boot_sec: Some("1min".to_string()),
                is_service: false,
                display: None,
            }),
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
        };

        let timer = generate_timer(&config);
        assert!(timer.contains("OnBootSec=1min"));
    }

    #[test]
    fn test_daemon_service_generation() {
        let config = UnitConfig {
            name: "agent".to_string(),
            command: "ambient-task-agent serve --port 3000".to_string(),
            workdir: "/home/user/project".to_string(),
            description: "ambient-task-agent serve --port 3000".to_string(),
            unit_type: UnitType::Service,
            cron_expr: None,
            schedule: None,
            restart_policy: Some("on-failure".to_string()),
            env_file: Some("/home/user/.config/bot/.env".to_string()),
            memory_max: None,
            cpu_quota: None,
            io_weight: None,
            timeout_stop: None,
            exec_start_pre: None,
            exec_stop_post: None,
            log_level_max: None,
            random_delay: None,
            env: vec![],
        };

        let service = generate_daemon_service(&config);
        assert!(service.contains("# sdtab:type=service"));
        assert!(service.contains("# sdtab:restart=on-failure"));
        assert!(service.contains("Type=simple"));
        assert!(service.contains("ExecStart=ambient-task-agent serve --port 3000"));
        assert!(service.contains("Restart=on-failure"));
        assert!(service.contains("RestartSec=5"));
        assert!(service.contains("EnvironmentFile=/home/user/.config/bot/.env"));
        assert!(service.contains("WantedBy=default.target"));
    }

    #[test]
    fn test_daemon_service_defaults() {
        let config = UnitConfig {
            name: "bot".to_string(),
            command: "python bot.py".to_string(),
            workdir: "/home/user".to_string(),
            description: "python bot.py".to_string(),
            unit_type: UnitType::Service,
            cron_expr: None,
            schedule: None,
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
        };

        let service = generate_daemon_service(&config);
        assert!(service.contains("Restart=always"));
        // Global env is always present (with - prefix for optional)
        assert!(service.contains("EnvironmentFile=-"));
        // User-specified env file (without - prefix) should NOT be present
        assert!(!service.contains("EnvironmentFile=/"));
    }

    #[test]
    fn test_resource_limits_service() {
        let config = UnitConfig {
            name: "heavy".to_string(),
            command: "./heavy-task.sh".to_string(),
            workdir: "/home/user".to_string(),
            description: "heavy task".to_string(),
            unit_type: UnitType::Timer,
            cron_expr: Some("0 10 * * *".to_string()),
            schedule: Some(CronSchedule {
                on_calendar: Some("*-*-* 10:00:00".to_string()),
                on_boot_sec: None,
                is_service: false,
                display: None,
            }),
            restart_policy: None,
            env_file: None,
            memory_max: Some("512M".to_string()),
            cpu_quota: Some("50%".to_string()),
            io_weight: Some("10".to_string()),
            timeout_stop: Some("30s".to_string()),
            exec_start_pre: None,
            exec_stop_post: None,
            log_level_max: Some("warning".to_string()),
            random_delay: Some("5m".to_string()),
            env: vec![],
        };

        let service = generate_service(&config);
        assert!(service.contains("MemoryMax=512M"));
        assert!(service.contains("CPUQuota=50%"));
        assert!(service.contains("IOWeight=10"));
        assert!(service.contains("TimeoutStopSec=30s"));
        assert!(service.contains("LogLevelMax=warning"));

        let timer = generate_timer(&config);
        assert!(timer.contains("RandomizedDelaySec=5m"));
    }

    #[test]
    fn test_resource_limits_daemon() {
        let config = UnitConfig {
            name: "bot".to_string(),
            command: "python bot.py".to_string(),
            workdir: "/home/user".to_string(),
            description: "python bot.py".to_string(),
            unit_type: UnitType::Service,
            cron_expr: None,
            schedule: None,
            restart_policy: Some("always".to_string()),
            env_file: None,
            memory_max: Some("1G".to_string()),
            cpu_quota: None,
            io_weight: None,
            timeout_stop: None,
            exec_start_pre: Some("/usr/bin/test -f /tmp/ready".to_string()),
            exec_stop_post: Some("/usr/bin/curl -s http://notify/down".to_string()),
            log_level_max: None,
            random_delay: None,
            env: vec![],
        };

        let service = generate_daemon_service(&config);
        assert!(service.contains("MemoryMax=1G"));
        assert!(!service.contains("CPUQuota"));
        assert!(service.contains("ExecStartPre=/usr/bin/test -f /tmp/ready"));
        assert!(service.contains("ExecStopPost=/usr/bin/curl -s http://notify/down"));
    }

    #[test]
    fn test_no_resource_limits() {
        let config = UnitConfig {
            name: "light".to_string(),
            command: "echo hello".to_string(),
            workdir: "/home/user".to_string(),
            description: "light task".to_string(),
            unit_type: UnitType::Timer,
            cron_expr: Some("@daily".to_string()),
            schedule: Some(CronSchedule {
                on_calendar: Some("*-*-* 00:00:00".to_string()),
                on_boot_sec: None,
                is_service: false,
                display: None,
            }),
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
        };

        let service = generate_service(&config);
        assert!(!service.contains("MemoryMax"));
        assert!(!service.contains("CPUQuota"));
    }
}
