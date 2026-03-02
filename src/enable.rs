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

    if is_timer {
        let timer_unit = unit::timer_filename(name);
        if let Err(e) = systemctl::enable_and_start(&timer_unit) {
            eprintln!("Error: Failed to enable timer '{}': {}", name, e);
            eprintln!();
            eprintln!("  sdtab logs {}      # View logs", name);
            eprintln!("  sdtab status {}    # Check detailed status", name);
            std::process::exit(1);
        }
        println!("Enabled timer '{}'.", name);
    } else {
        let service_unit = unit::service_filename(name);
        if let Err(e) = systemctl::enable_and_start(&service_unit) {
            eprintln!("Error: Failed to enable service '{}': {}", name, e);
            eprintln!();
            eprintln!("  sdtab logs {}      # View logs", name);
            eprintln!("  sdtab status {}    # Check detailed status", name);
            std::process::exit(1);
        }
        println!("Enabled service '{}'.", name);
    }

    Ok(())
}
