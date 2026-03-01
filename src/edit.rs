use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::{init, systemctl, unit};

pub fn run(name: &str) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    let service_path = dir_path.join(unit::service_filename(name));
    let timer_path = dir_path.join(unit::timer_filename(name));

    if !service_path.exists() {
        bail!("'{}' not found.", name);
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let is_timer = timer_path.exists();

    // Open editor with service file (and timer file if it exists)
    let mut cmd = Command::new(&editor);
    cmd.arg(&service_path);
    if is_timer {
        cmd.arg(&timer_path);
    }

    let status = cmd.status().context("Failed to open editor")?;
    if !status.success() {
        bail!("Editor exited with non-zero status");
    }

    // Reload daemon after editing
    systemctl::daemon_reload()?;
    println!("Reloaded systemd user daemon.");

    // For services (not timers), suggest restart
    if !is_timer {
        println!("Hint: run `sdtab restart {}` to apply changes.", name);
    }

    Ok(())
}
