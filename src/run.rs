use std::path::Path;

use anyhow::{bail, Result};

use crate::{init, systemctl, unit};

/// Trigger a unit once, immediately (not on its schedule).
/// For timers, runs the associated .service directly — the timer is left untouched.
/// For services, runs `systemctl --user start` (no-op if already active).
pub fn run(name: &str) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    let service_path = dir_path.join(unit::service_filename(name));
    let timer_path = dir_path.join(unit::timer_filename(name));

    if !service_path.exists() {
        bail!("'{}' not found.", name);
    }

    let service_unit = unit::service_filename(name);
    systemctl::start(&service_unit)?;

    if timer_path.exists() {
        println!(
            "Triggered service '{}' manually. Timer schedule is unchanged.",
            name
        );
    } else {
        println!("Started service '{}'.", name);
    }
    println!("Follow logs: sdtab logs {} -f", name);

    Ok(())
}
