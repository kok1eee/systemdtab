use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{bail, Result};

use crate::{init, parse_unit, systemctl, unit};

pub fn run(
    name: Option<&str>,
    follow: bool,
    lines: u32,
    priority: Option<String>,
    all: bool,
    failed: bool,
    since: Option<&str>,
) -> Result<()> {
    let aggregate = all || failed;

    if aggregate && name.is_some() {
        bail!("Cannot combine a unit name with --all / --failed.");
    }
    if !aggregate && name.is_none() {
        bail!(
            "No unit name given. Pass a <name>, or use --all / --failed to aggregate across all sdtab units."
        );
    }

    let mut cmd = Command::new("journalctl");
    cmd.arg("--user");
    cmd.args(["-n", &lines.to_string()]);
    cmd.arg("--no-pager");

    if follow {
        cmd.arg("-f");
    }
    if let Some(ref prio) = priority {
        cmd.args(["-p", prio]);
    }
    if let Some(ref s) = since {
        cmd.args(["--since", s]);
    }

    if let Some(name) = name {
        let unit_dir = init::unit_dir()?;
        let dir_path = std::path::Path::new(&unit_dir);
        let service_path = dir_path.join(unit::service_filename(name));
        let timer_path = dir_path.join(unit::timer_filename(name));
        if !service_path.exists() && !timer_path.exists() {
            bail!("'{}' not found.", name);
        }
        let unit_name = unit::service_filename(name);
        cmd.args(["--user-unit", &unit_name]);
    } else {
        let units = parse_unit::scan_all_units()?;
        let target_names: Vec<String> = if failed {
            units
                .into_iter()
                .filter(|u| is_failed(&u.name))
                .map(|u| u.name)
                .collect()
        } else {
            units.into_iter().map(|u| u.name).collect()
        };

        if target_names.is_empty() {
            if failed {
                println!("No failed sdtab units.");
            } else {
                println!("No sdtab units found.");
            }
            return Ok(());
        }

        for n in &target_names {
            cmd.arg("--user-unit");
            cmd.arg(unit::service_filename(n));
        }
    }

    let err = cmd.exec();
    bail!("Failed to exec journalctl: {}", err);
}

fn is_failed(name: &str) -> bool {
    let service_unit = unit::service_filename(name);
    let service_state = systemctl::show_property(&service_unit, "ActiveState")
        .unwrap_or_else(|_| String::new());
    if service_state == "failed" {
        return true;
    }
    let timer_unit = unit::timer_filename(name);
    let timer_state = systemctl::show_property(&timer_unit, "ActiveState")
        .unwrap_or_else(|_| String::new());
    timer_state == "failed"
}
