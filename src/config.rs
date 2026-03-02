use std::fs;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::init;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub notify: NotifyConfig,
}

#[derive(Serialize, Deserialize, Default)]
pub struct NotifyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slack_webhook: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slack_mention: Option<String>,
}

pub fn config_path() -> Result<String> {
    let config_dir = init::config_dir()?;
    Ok(format!("{}/config.toml", config_dir))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    match fs::read_to_string(&path) {
        Ok(content) => {
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", path))?;
            Ok(config)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e).with_context(|| format!("Failed to read {}", path)),
    }
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    let config_dir = init::config_dir()?;
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create {}", config_dir))?;
    let content = toml::to_string_pretty(config)
        .context("Failed to serialize config")?;
    fs::write(&path, &content)
        .with_context(|| format!("Failed to write {}", path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.notify.slack_webhook.is_none());
    }

    #[test]
    fn test_serialize_empty_config() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("[notify]"));
        assert!(!toml_str.contains("slack_webhook"));
    }

    #[test]
    fn test_serialize_with_webhook() {
        let config = Config {
            notify: NotifyConfig {
                slack_webhook: Some("https://hooks.slack.com/services/T/B/X".to_string()),
                slack_mention: None,
            },
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("slack_webhook = \"https://hooks.slack.com/services/T/B/X\""));
    }

    #[test]
    fn test_deserialize_config() {
        let toml_str = r#"
[notify]
slack_webhook = "https://hooks.slack.com/services/T/B/X"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.notify.slack_webhook,
            Some("https://hooks.slack.com/services/T/B/X".to_string())
        );
    }

    #[test]
    fn test_deserialize_empty() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.notify.slack_webhook.is_none());
    }
}
