use std::fs;
use std::path::Path;

use anyhow::{bail, Result};

use crate::{init, systemctl, unit};

pub fn run(name: &str) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    let service_path = dir_path.join(unit::service_filename(name));
    let timer_path = dir_path.join(unit::timer_filename(name));

    if !timer_path.exists() && !service_path.exists() {
        bail!("'{}' not found.", name);
    }

    // Stop and disable: timer if exists, otherwise service
    if timer_path.exists() {
        let timer_unit = unit::timer_filename(name);
        if let Err(e) = systemctl::stop_and_disable(&timer_unit) {
            eprintln!("Warning: failed to disable timer: {}", e);
        }
    } else {
        let service_unit = unit::service_filename(name);
        if let Err(e) = systemctl::stop_and_disable(&service_unit) {
            eprintln!("Warning: failed to disable service: {}", e);
        }
    }

    // Remove unit files
    if service_path.exists() {
        fs::remove_file(&service_path)?;
        println!("Removed: {}", service_path.display());
    }
    if timer_path.exists() {
        fs::remove_file(&timer_path)?;
        println!("Removed: {}", timer_path.display());
    }

    // Reload daemon
    systemctl::daemon_reload()?;

    println!("'{}' has been removed.", name);

    Ok(())
}
