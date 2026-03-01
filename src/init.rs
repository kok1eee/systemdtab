use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::systemctl;

pub fn run() -> Result<()> {
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

    // 4. Reload systemd user daemon
    println!("Reloading systemd user daemon...");
    systemctl::daemon_reload()?;

    println!("sdtab initialized successfully.");
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
