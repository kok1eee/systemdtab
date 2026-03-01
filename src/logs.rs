use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{bail, Result};

use crate::{init, unit};

pub fn run(name: &str, follow: bool, lines: u32, priority: Option<String>) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = std::path::Path::new(&unit_dir);

    let service_path = dir_path.join(unit::service_filename(name));
    let timer_path = dir_path.join(unit::timer_filename(name));

    if !service_path.exists() && !timer_path.exists() {
        bail!("'{}' not found.", name);
    }

    let unit_name = unit::service_filename(name);

    let mut cmd = Command::new("journalctl");
    cmd.args(["--user-unit", &unit_name]);
    cmd.args(["-n", &lines.to_string()]);
    cmd.arg("--no-pager");

    if follow {
        cmd.arg("-f");
    }

    if let Some(ref prio) = priority {
        cmd.args(["-p", prio]);
    }

    // Replace the current process with journalctl
    let err = cmd.exec();
    bail!("Failed to exec journalctl: {}", err);
}
