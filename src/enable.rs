use std::path::Path;

use anyhow::{bail, Result};

use crate::{init, systemctl, unit};

pub fn run(name: &str) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    let service_path = dir_path.join(unit::service_filename(name));
    let timer_path = dir_path.join(unit::timer_filename(name));

    if !service_path.exists() && !timer_path.exists() {
        bail!("'{}' not found.", name);
    }

    let is_timer = timer_path.exists();
    let (unit_file, kind) = if is_timer {
        (unit::timer_filename(name), "timer")
    } else {
        (unit::service_filename(name), "service")
    };

    if let Err(e) = systemctl::enable_and_start(&unit_file) {
        eprintln!();
        eprintln!("  sdtab logs {}      # View logs", name);
        eprintln!("  sdtab status {}    # Check detailed status", name);
        bail!("Failed to enable {} '{}': {}", kind, name, e);
    }
    println!("Enabled {} '{}'.", kind, name);

    Ok(())
}
