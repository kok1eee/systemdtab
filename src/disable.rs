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
        systemctl::stop_and_disable(&timer_unit)?;
        println!("Disabled timer '{}'. Unit files are preserved.", name);
    } else {
        let service_unit = unit::service_filename(name);
        systemctl::stop_and_disable(&service_unit)?;
        println!("Disabled service '{}'. Unit files are preserved.", name);
    }

    Ok(())
}
