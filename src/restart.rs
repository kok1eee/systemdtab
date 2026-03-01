use std::path::Path;

use anyhow::{bail, Result};

use crate::{init, systemctl, unit};

pub fn run(name: &str) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    let service_path = dir_path.join(unit::service_filename(name));
    let timer_path = dir_path.join(unit::timer_filename(name));

    if !service_path.exists() {
        bail!("'{}' not found.", name);
    }

    if timer_path.exists() {
        bail!("'{}' is a timer. Only services can be restarted.", name);
    }

    let service_unit = unit::service_filename(name);
    systemctl::restart(&service_unit)?;
    println!("Restarted service '{}'.", name);

    Ok(())
}
