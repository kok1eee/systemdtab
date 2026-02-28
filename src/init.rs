use std::fs;
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

    // 3. Reload systemd user daemon
    println!("Reloading systemd user daemon...");
    systemctl::daemon_reload()?;

    println!("sdtab initialized successfully.");
    Ok(())
}

pub fn unit_dir() -> Result<String> {
    let home = std::env::var("HOME").context("Could not determine HOME directory")?;
    Ok(format!("{}/.config/systemd/user", home))
}
