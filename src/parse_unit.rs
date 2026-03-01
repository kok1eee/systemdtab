use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::init;

#[derive(Debug, Clone)]
pub enum UnitType {
    Timer,
    Service,
}

impl UnitType {
    pub fn label(&self) -> &'static str {
        match self {
            UnitType::Timer => "timer",
            UnitType::Service => "service",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedUnit {
    pub name: String,
    pub unit_type: UnitType,
    pub command: String,
    pub workdir: String,
    pub description: String,
    pub cron_expr: Option<String>,
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

pub fn scan_all_units() -> Result<Vec<ParsedUnit>> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    if !dir_path.exists() {
        return Ok(vec![]);
    }

    let global_env_path = init::global_env_path().unwrap_or_default();
    let mut units = Vec::new();

    let read_dir = fs::read_dir(dir_path)?;
    for entry in read_dir {
        let entry = entry?;
        let os_name = entry.file_name();
        let filename = os_name.to_string_lossy();

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

        // Read timer file if it exists (try read directly, handle NotFound)
        let timer_path = dir_path.join(format!("sdtab-{}.timer", name));
        let timer_content = match fs::read_to_string(&timer_path) {
            Ok(content) => Some(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };

        let parsed = parse_service_file(&name, &service_content, timer_content.as_deref(), &global_env_path);
        units.push(parsed);
    }

    units.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(units)
}

pub fn parse_service_file(
    name: &str,
    service_content: &str,
    timer_content: Option<&str>,
    global_env_path: &str,
) -> ParsedUnit {
    let mut unit_type = UnitType::Timer;
    let mut cron_expr = None;
    let mut command = String::new();
    let mut original_command: Option<String> = None;
    let mut workdir = String::new();
    let mut description = String::new();
    let mut restart_policy = None;
    let mut env_file = None;
    let mut memory_max = None;
    let mut cpu_quota = None;
    let mut io_weight = None;
    let mut timeout_stop = None;
    let mut exec_start_pre = None;
    let mut exec_stop_post = None;
    let mut log_level_max = None;
    let mut env = Vec::new();

    for line in service_content.lines() {
        let line = line.trim();

        // Metadata comments
        if line == "# sdtab:type=service" {
            unit_type = UnitType::Service;
        } else if line == "# sdtab:type=timer" {
            unit_type = UnitType::Timer;
        }
        if let Some(val) = line.strip_prefix("# sdtab:cron=") {
            cron_expr = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("# sdtab:restart=") {
            restart_policy = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("# sdtab:command=") {
            original_command = Some(val.to_string());
        }

        // Unit file directives
        if let Some(val) = line.strip_prefix("ExecStart=") {
            command = val.to_string();
        }
        if let Some(val) = line.strip_prefix("WorkingDirectory=") {
            workdir = val.to_string();
        }
        if let Some(val) = line.strip_prefix("Description=[sdtab] ") {
            // Format: "name: desc" — extract description part
            if let Some(pos) = val.find(": ") {
                description = val[pos + 2..].to_string();
            } else {
                description = val.to_string();
            }
        }
        if let Some(val) = line.strip_prefix("EnvironmentFile=") {
            // Skip global env file (starts with -)
            if let Some(path) = val.strip_prefix('-') {
                // Global env file — skip if it matches the known global path
                if path != global_env_path {
                    env_file = Some(path.to_string());
                }
            } else {
                env_file = Some(val.to_string());
            }
        }
        if let Some(val) = line.strip_prefix("MemoryMax=") {
            memory_max = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("CPUQuota=") {
            cpu_quota = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("IOWeight=") {
            io_weight = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("TimeoutStopSec=") {
            timeout_stop = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("ExecStartPre=") {
            exec_start_pre = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("ExecStopPost=") {
            exec_stop_post = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("LogLevelMax=") {
            log_level_max = Some(val.to_string());
        }
        if let Some(val) = line.strip_prefix("Environment=") {
            env.push(val.to_string());
        }
    }

    // Parse timer content for random delay
    let mut random_delay = None;
    if let Some(timer) = timer_content {
        for line in timer.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("RandomizedDelaySec=") {
                random_delay = Some(val.to_string());
            }
        }
    }

    // Use original_command if available, otherwise fall back to ExecStart
    if let Some(orig) = original_command {
        command = orig;
    }

    ParsedUnit {
        name: name.to_string(),
        unit_type,
        command,
        workdir,
        description,
        cron_expr,
        restart_policy,
        env_file,
        memory_max,
        cpu_quota,
        io_weight,
        timeout_stop,
        exec_start_pre,
        exec_stop_post,
        log_level_max,
        random_delay,
        env,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timer_service_file() {
        let service = "\
# sdtab:type=timer
# sdtab:cron=0 9 * * *
# sdtab:command=uv run ./report.py
[Unit]
Description=[sdtab] report: daily report

[Service]
Type=oneshot
ExecStart=/home/user/.local/share/mise/shims/uv run ./report.py
WorkingDirectory=/home/user/project
EnvironmentFile=-/home/user/.config/sdtab/env
MemoryMax=512M
CPUQuota=50%
";

        let parsed = parse_service_file("report", service, None, "/home/user/.config/sdtab/env");
        assert!(matches!(parsed.unit_type, UnitType::Timer));
        assert_eq!(parsed.command, "uv run ./report.py");
        assert_eq!(parsed.workdir, "/home/user/project");
        assert_eq!(parsed.description, "daily report");
        assert_eq!(parsed.cron_expr, Some("0 9 * * *".to_string()));
        assert_eq!(parsed.memory_max, Some("512M".to_string()));
        assert_eq!(parsed.cpu_quota, Some("50%".to_string()));
        assert!(parsed.env_file.is_none());
    }

    #[test]
    fn parse_daemon_service_file() {
        let service = "\
# sdtab:type=service
# sdtab:restart=always
# sdtab:command=node --env-file=.env dist/index.js
[Unit]
Description=[sdtab] web: PSP OCR Web
After=network-online.target

[Service]
Type=simple
ExecStart=/home/user/.local/share/mise/shims/node --env-file=.env dist/index.js
WorkingDirectory=/home/user/project
Restart=always
RestartSec=5
EnvironmentFile=-/home/user/.config/sdtab/env
EnvironmentFile=/home/user/.env
MemoryMax=256M

[Install]
WantedBy=default.target
";

        let parsed = parse_service_file("web", service, None, "/home/user/.config/sdtab/env");
        assert!(matches!(parsed.unit_type, UnitType::Service));
        assert_eq!(parsed.command, "node --env-file=.env dist/index.js");
        assert_eq!(parsed.restart_policy, Some("always".to_string()));
        assert_eq!(parsed.env_file, Some("/home/user/.env".to_string()));
        assert_eq!(parsed.memory_max, Some("256M".to_string()));
    }

    #[test]
    fn parse_fallback_to_exec_start() {
        let service = "\
# sdtab:type=timer
# sdtab:cron=@daily
[Unit]
Description=[sdtab] task: echo hello

[Service]
Type=oneshot
ExecStart=/usr/bin/echo hello
WorkingDirectory=/home/user
EnvironmentFile=-/home/user/.config/sdtab/env
";

        let parsed = parse_service_file("task", service, None, "/home/user/.config/sdtab/env");
        // No sdtab:command, so should fall back to ExecStart
        assert_eq!(parsed.command, "/usr/bin/echo hello");
    }

    #[test]
    fn parse_timer_with_random_delay() {
        let service = "\
# sdtab:type=timer
# sdtab:cron=0 9 * * *
[Unit]
Description=[sdtab] task: test

[Service]
Type=oneshot
ExecStart=/usr/bin/test
WorkingDirectory=/home/user
";
        let timer = "\
[Unit]
Description=[sdtab] task timer

[Timer]
OnCalendar=*-*-* 09:00:00
Persistent=true
RandomizedDelaySec=5m

[Install]
WantedBy=timers.target
";

        let parsed = parse_service_file("task", service, Some(timer), "");
        assert_eq!(parsed.random_delay, Some("5m".to_string()));
    }

    #[test]
    fn parse_environment_variables() {
        let service = "\
# sdtab:type=service
# sdtab:restart=always
[Unit]
Description=[sdtab] app: my app

[Service]
Type=simple
ExecStart=/usr/bin/app
WorkingDirectory=/home/user
Restart=always
RestartSec=5
Environment=FOO=bar
Environment=BAZ=qux
";

        let parsed = parse_service_file("app", service, None, "");
        assert_eq!(parsed.env, vec!["FOO=bar", "BAZ=qux"]);
    }
}
