use std::fs;
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

    let content = fs::read_to_string(&service_path)?;
    let is_timer = timer_path.exists();
    let is_service = content.contains("# sdtab:type=service");

    println!("Name:    {}", name);

    if is_service {
        println!("Type:    service");
        print_service_status(name)?;
    } else if is_timer {
        println!("Type:    timer");
        print_timer_status(name)?;
    }

    // Show common service properties
    let service_unit = unit::service_filename(name);
    if let Ok(cmd) = systemctl::show_property(&service_unit, "ExecStart") {
        // ExecStart format: { path=... ; argv[]=... } - extract the meaningful part
        let cmd_display = extract_exec_command(&cmd);
        println!("Command: {}", cmd_display);
    }
    if let Ok(workdir) = systemctl::show_property(&service_unit, "WorkingDirectory") {
        if !workdir.is_empty() {
            println!("WorkDir: {}", workdir);
        }
    }

    Ok(())
}

fn print_service_status(name: &str) -> Result<()> {
    let service_unit = unit::service_filename(name);

    let active = systemctl::show_property(&service_unit, "ActiveState")
        .unwrap_or_else(|_| "unknown".to_string());
    let sub = systemctl::show_property(&service_unit, "SubState")
        .unwrap_or_else(|_| "unknown".to_string());
    let pid = systemctl::show_property(&service_unit, "MainPID")
        .unwrap_or_else(|_| "?".to_string());

    println!("Status:  {} ({})", active, sub);
    if active == "active" && pid != "0" {
        println!("PID:     {}", pid);
    }

    if let Ok(memory) = systemctl::show_property(&service_unit, "MemoryCurrent") {
        if memory != "[not set]" && memory != "infinity" {
            if let Ok(bytes) = memory.parse::<u64>() {
                println!("Memory:  {}", format_bytes(bytes));
            }
        }
    }

    Ok(())
}

fn print_timer_status(name: &str) -> Result<()> {
    let timer_unit = unit::timer_filename(name);
    let service_unit = unit::service_filename(name);

    let active = systemctl::show_property(&timer_unit, "ActiveState")
        .unwrap_or_else(|_| "unknown".to_string());
    println!("Status:  {}", active);

    if let Ok(next) = systemctl::show_property(&timer_unit, "NextElapseUSecRealtime") {
        if !next.is_empty() && next != "n/a" {
            println!("Next:    {}", next);
        }
    }

    if let Ok(last) = systemctl::show_property(&service_unit, "ExecMainStartTimestamp") {
        if !last.is_empty() && last != "n/a" {
            println!("Last:    {}", last);
        }
    }

    if let Ok(result) = systemctl::show_property(&service_unit, "Result") {
        println!("Result:  {}", result);
    }

    Ok(())
}

fn extract_exec_command(raw: &str) -> String {
    // systemctl show format: { path=/usr/bin/foo ; argv[]=/usr/bin/foo arg1 arg2 ; ... }
    if let Some(start) = raw.find("argv[]=") {
        let rest = &raw[start + 7..];
        if let Some(end) = rest.find(';') {
            return rest[..end].trim().to_string();
        }
        return rest.trim().trim_end_matches('}').trim().to_string();
    }
    raw.to_string()
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
