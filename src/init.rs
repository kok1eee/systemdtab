use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::{config, systemctl};

pub fn run(slack_webhook: Option<&str>, slack_mention: Option<&str>) -> Result<()> {
    // 1. Enable linger for the current user
    let user = std::env::var("USER").context("Could not determine current user")?;
    println!("Enabling linger for user '{}'...", user);

    let status = Command::new("loginctl")
        .args(["enable-linger", &user])
        .status()
        .context("Failed to execute loginctl")?;

    if !status.success() {
        anyhow::bail!("loginctl enable-linger failed");
    }

    // 2. Create systemd user directory
    let user_dir = unit_dir()?;
    println!("Creating directory: {}", user_dir);
    fs::create_dir_all(&user_dir).context("Failed to create systemd user directory")?;

    // 3. Create config directory and env file
    let config_dir = config_dir()?;
    fs::create_dir_all(&config_dir).context("Failed to create sdtab config directory")?;

    let env_path = global_env_path()?;
    if !Path::new(&env_path).exists() {
        let template = "# sdtab global environment variables\n\
                        # All services and timers inherit these settings.\n\
                        # Format: KEY=VALUE (one per line)\n\
                        #\n\
                        # PATH=/home/ec2-user/.local/share/mise/installs/node/24.4.1/bin:/usr/local/bin:/usr/bin:/bin\n\
                        # PYTHONUNBUFFERED=1\n";
        fs::write(&env_path, template)
            .with_context(|| format!("Failed to write {}", env_path))?;
        println!("Created: {} (edit to set PATH etc.)", env_path);
    } else {
        println!("Exists: {}", env_path);
    }

    // 4. Set up Slack failure notification if webhook or mention provided
    if slack_webhook.is_some() || slack_mention.is_some() {
        setup_notify(slack_webhook, slack_mention)?;
    }

    // 5. Install Claude Code skill file
    install_claude_skill()?;

    // 6. Reload systemd user daemon
    println!("Reloading systemd user daemon...");
    systemctl::daemon_reload()?;

    println!("sdtab initialized successfully.");
    Ok(())
}

fn setup_notify(webhook: Option<&str>, mention: Option<&str>) -> Result<()> {
    // Notification template requires jq for safe JSON construction
    if Command::new("which").arg("jq").output().map_or(true, |o| !o.status.success()) {
        bail!("jq is required for Slack notifications. Install it with: sudo yum install -y jq");
    }

    // Load existing config and update
    let mut cfg = config::load()?;
    if let Some(webhook) = webhook {
        cfg.notify.slack_webhook = Some(webhook.to_string());
    }
    if let Some(mention) = mention {
        // Accept: user ID (U0700J8MN3W), !here, !channel, !subteam^ID
        let valid = mention == "!here"
            || mention == "!channel"
            || mention.starts_with("!subteam^")
            || mention.chars().all(|c| c.is_ascii_alphanumeric());
        if !valid {
            bail!(
                "Invalid slack-mention '{}': expected user ID (U0700J8MN3W), !here, or !channel",
                mention
            );
        }
        cfg.notify.slack_mention = Some(mention.to_string());
    }
    config::save(&cfg)?;
    println!("Saved: {}", config::config_path()?);

    let webhook = cfg.notify.slack_webhook.as_deref()
        .context("Slack webhook URL is required. Use --slack-webhook to set it.")?;

    // Write notify.env
    let notify_env_path = notify_env_path()?;
    let content = format!("SDTAB_SLACK_WEBHOOK={}\n", webhook);
    fs::write(&notify_env_path, &content)
        .with_context(|| format!("Failed to write {}", notify_env_path))?;
    println!("Created: {}", notify_env_path);

    // Build notification message
    let mention_prefix = match &cfg.notify.slack_mention {
        Some(id) if id.starts_with('!') => format!("<!{}> ", id.trim_start_matches('!')),
        Some(id) => format!("<@{}> ", id),
        None => String::new(),
    };

    // Generate template unit sdtab-notify@.service
    // Uses Environment= for systemd specifiers (%i, %H) to avoid shell injection,
    // and jq for safe JSON construction (handles special chars in hostname etc.)
    let unit_dir = unit_dir()?;
    let template_path = format!("{}/sdtab-notify@.service", unit_dir);
    let template = format!(
        "[Unit]\n\
         Description=[sdtab] Failure notification for %i\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         Environment=\"SDTAB_UNIT=%i\" \"SDTAB_HOST=%H\" \"SDTAB_MENTION={mention_prefix}\"\n\
         ExecStart=/bin/sh -c 'printf \"%%s\" \
         \"$SDTAB_MENTION[sdtab] $SDTAB_UNIT failed on $SDTAB_HOST\" \
         | jq -Rs \"{{text:.}}\" \
         | curl -s -X POST -H \"Content-Type: application/json\" -d @- \
         \"$SDTAB_SLACK_WEBHOOK\"'\n\
         EnvironmentFile={env_path}\n",
        mention_prefix = mention_prefix,
        env_path = notify_env_path
    );
    fs::write(&template_path, &template)
        .with_context(|| format!("Failed to write {}", template_path))?;
    println!("Created: {}", template_path);

    Ok(())
}

pub fn notify_env_path() -> Result<String> {
    let config = config_dir()?;
    Ok(format!("{}/notify.env", config))
}

const SKILL_CONTENT: &str = include_str!("../skill/sdtab.md");

const MANAGED_MARKER: &str = "managed by sdtab";

fn install_claude_skill() -> Result<()> {
    let home = std::env::var("HOME").context("Could not determine HOME directory")?;
    let skill_dir = format!("{}/.claude/commands", home);
    let skill_path = format!("{}/sdtab.md", skill_dir);

    // If file exists and lacks the managed marker, user has customized it — skip
    if Path::new(&skill_path).exists() {
        if let Ok(existing) = fs::read_to_string(&skill_path) {
            if !existing.contains(MANAGED_MARKER) {
                println!("Skipped: {} (customized by user)", skill_path);
                return Ok(());
            }
        }
    }

    fs::create_dir_all(&skill_dir)
        .with_context(|| format!("Failed to create {}", skill_dir))?;

    fs::write(&skill_path, SKILL_CONTENT)
        .with_context(|| format!("Failed to write {}", skill_path))?;

    println!("Installed: {} (Claude Code skill)", skill_path);
    Ok(())
}

pub fn unit_dir() -> Result<String> {
    let home = std::env::var("HOME").context("Could not determine HOME directory")?;
    Ok(format!("{}/.config/systemd/user", home))
}

pub fn config_dir() -> Result<String> {
    let home = std::env::var("HOME").context("Could not determine HOME directory")?;
    Ok(format!("{}/.config/sdtab", home))
}

pub fn global_env_path() -> Result<String> {
    let config = config_dir()?;
    Ok(format!("{}/env", config))
}

/// Read PATH from the global env file and return the directories
pub fn read_env_path() -> Result<Vec<PathBuf>> {
    let env_path = global_env_path()?;
    let content = match fs::read_to_string(&env_path) {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(val) = line.strip_prefix("PATH=") {
            return Ok(val.split(':').map(PathBuf::from).collect());
        }
    }

    Ok(vec![])
}

/// Resolve a command name to an absolute path using the global env PATH.
/// If the command already starts with '/', return as-is.
/// Only resolves the first token (the binary name).
pub fn resolve_command(command: &str) -> Result<String> {
    let parts: Vec<&str> = command.splitn(2, char::is_whitespace).collect();
    let binary = parts[0];
    let args = if parts.len() > 1 { parts[1] } else { "" };

    // Already absolute path
    if binary.starts_with('/') {
        return Ok(command.to_string());
    }

    // Relative path like ./start.sh
    if binary.starts_with("./") || binary.starts_with("../") {
        return Ok(command.to_string());
    }

    // Try to resolve from global env PATH
    let path_dirs = read_env_path()?;
    for dir in &path_dirs {
        let candidate = dir.join(binary);
        if candidate.exists() {
            let resolved = candidate.to_string_lossy();
            if args.is_empty() {
                return Ok(resolved.to_string());
            } else {
                return Ok(format!("{} {}", resolved, args));
            }
        }
    }

    // Fallback: try system which
    if let Ok(output) = Command::new("which")
        .arg(binary)
        .output()
    {
        if output.status.success() {
            let full_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if args.is_empty() {
                return Ok(full_path);
            } else {
                return Ok(format!("{} {}", full_path, args));
            }
        }
    }

    anyhow::bail!(
        "Command '{}' not found. Add its directory to PATH in {}",
        binary,
        global_env_path().unwrap_or_default()
    )
}
