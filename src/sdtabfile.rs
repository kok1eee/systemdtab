use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Sdtabfile {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub timers: BTreeMap<String, TimerEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub services: BTreeMap<String, ServiceEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TimerEntry {
    pub schedule: String,
    pub command: String,
    pub workdir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_max: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_quota: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_weight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_stop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec_start_pre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec_stop_post: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level_max: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_delay: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ServiceEntry {
    pub command: String,
    pub workdir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "is_default_restart"
    )]
    pub restart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_max: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_quota: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_weight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_stop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec_start_pre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec_stop_post: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level_max: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
}

/// Convert description to Option: None if it equals command (convention: omit when same)
pub fn description_if_different(desc: &str, command: &str) -> Option<String> {
    if desc == command {
        None
    } else {
        Some(desc.to_string())
    }
}

/// Check if current description matches desired (None means desc == command)
pub fn desc_matches(current_desc: &str, current_cmd: &str, desired_desc: &Option<String>) -> bool {
    description_if_different(current_desc, current_cmd) == *desired_desc
}

fn is_default_restart(val: &Option<String>) -> bool {
    match val {
        None => true,
        Some(v) => v == "always",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_default_restart() {
        assert!(is_default_restart(&None));
        assert!(is_default_restart(&Some("always".to_string())));
        assert!(!is_default_restart(&Some("on-failure".to_string())));
        assert!(!is_default_restart(&Some("no".to_string())));
    }

    #[test]
    fn test_roundtrip_timer() {
        let mut timers = BTreeMap::new();
        timers.insert(
            "report".to_string(),
            TimerEntry {
                schedule: "0 9 * * *".to_string(),
                command: "uv run ./report.py".to_string(),
                workdir: "/home/user/project".to_string(),
                description: Some("daily report".to_string()),
                memory_max: Some("512M".to_string()),
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
        let file = Sdtabfile {
            timers,
            services: BTreeMap::new(),
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        let parsed: Sdtabfile = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.timers["report"].schedule, "0 9 * * *");
        assert_eq!(parsed.timers["report"].memory_max, Some("512M".to_string()));
    }

    #[test]
    fn test_roundtrip_service() {
        let mut services = BTreeMap::new();
        services.insert(
            "web".to_string(),
            ServiceEntry {
                command: "node index.js".to_string(),
                workdir: "/home/user".to_string(),
                description: None,
                restart: Some("on-failure".to_string()),
                env_file: Some("/home/user/.env".to_string()),
                memory_max: None,
                cpu_quota: None,
                io_weight: None,
                timeout_stop: None,
                exec_start_pre: None,
                exec_stop_post: None,
                log_level_max: None,
                env: vec!["NODE_ENV=production".to_string()],
            },
        );
        let file = Sdtabfile {
            timers: BTreeMap::new(),
            services,
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        assert!(toml_str.contains("restart = \"on-failure\""));
        // default restart (always) should be omitted
        assert!(!toml_str.contains("description"));

        let parsed: Sdtabfile = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.services["web"].restart, Some("on-failure".to_string()));
        assert_eq!(parsed.services["web"].env, vec!["NODE_ENV=production"]);
    }

    #[test]
    fn test_serialize_default_restart_omitted() {
        let mut services = BTreeMap::new();
        services.insert(
            "bot".to_string(),
            ServiceEntry {
                command: "python bot.py".to_string(),
                workdir: "/home/user".to_string(),
                description: None,
                restart: Some("always".to_string()),
                env_file: None,
                memory_max: None,
                cpu_quota: None,
                io_weight: None,
                timeout_stop: None,
                exec_start_pre: None,
                exec_stop_post: None,
                log_level_max: None,
                env: vec![],
            },
        );
        let file = Sdtabfile {
            timers: BTreeMap::new(),
            services,
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        assert!(!toml_str.contains("restart"));
    }

    #[test]
    fn test_parse_empty_toml() {
        let file: Sdtabfile = toml::from_str("").unwrap();
        assert!(file.timers.is_empty());
        assert!(file.services.is_empty());
    }
}
