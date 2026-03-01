use std::process::Command;

use anyhow::{bail, Context, Result};

fn run(args: &[&str]) -> Result<String> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .context("Failed to execute systemctl")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        bail!(
            "systemctl --user {} failed: {}",
            args.join(" "),
            stderr.trim()
        );
    }

    Ok(stdout.trim().to_string())
}

pub fn daemon_reload() -> Result<()> {
    run(&["daemon-reload"])?;
    Ok(())
}

pub fn enable_and_start(unit: &str) -> Result<()> {
    run(&["enable", "--now", unit])?;
    Ok(())
}

pub fn stop_and_disable(unit: &str) -> Result<()> {
    run(&["disable", "--now", unit])?;
    Ok(())
}

#[allow(dead_code)]
pub fn is_active(unit: &str) -> Result<bool> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(["is-active", unit])
        .output()
        .context("Failed to execute systemctl")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim() == "active")
}

pub fn restart(unit: &str) -> Result<()> {
    run(&["restart", unit])?;
    Ok(())
}

pub fn show_property(unit: &str, property: &str) -> Result<String> {
    let output = run(&["show", "-p", property, "--value", unit])?;
    Ok(output)
}
