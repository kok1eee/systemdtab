use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::{cron, init, systemctl, unit};

pub fn run(
    schedule: &str,
    command: &str,
    name: Option<String>,
    workdir: Option<String>,
    description: Option<String>,
    env_file: Option<String>,
    restart: Option<String>,
) -> Result<()> {
    if schedule.trim() == "@service" {
        run_service(command, name, workdir, description, env_file, restart)
    } else {
        run_timer(schedule, command, name, workdir, description)
    }
}

fn run_timer(
    schedule: &str,
    command: &str,
    name: Option<String>,
    workdir: Option<String>,
    description: Option<String>,
) -> Result<()> {
    let parsed = cron::parse(schedule)?;
    let name = name.unwrap_or_else(|| unit::derive_name(command));

    let unit_dir = init::unit_dir()?;
    let timer_path = Path::new(&unit_dir).join(unit::timer_filename(&name));
    if timer_path.exists() {
        bail!(
            "Timer '{}' already exists. Remove it first with: sdtab remove {}",
            name,
            name
        );
    }

    let workdir = resolve_workdir(workdir)?;
    let description = description.unwrap_or_else(|| command.to_string());

    let config = unit::UnitConfig {
        name: name.clone(),
        command: command.to_string(),
        workdir,
        description,
        unit_type: unit::UnitType::Timer,
        cron_expr: Some(schedule.to_string()),
        schedule: Some(parsed),
        restart_policy: None,
        env_file: None,
    };

    let service_content = unit::generate_service(&config);
    let timer_content = unit::generate_timer(&config);

    let service_path = Path::new(&unit_dir).join(unit::service_filename(&name));
    fs::write(&service_path, &service_content)
        .with_context(|| format!("Failed to write {}", service_path.display()))?;
    fs::write(&timer_path, &timer_content)
        .with_context(|| format!("Failed to write {}", timer_path.display()))?;

    println!("Created: {}", service_path.display());
    println!("Created: {}", timer_path.display());

    systemctl::daemon_reload()?;
    let timer_unit = unit::timer_filename(&name);
    systemctl::enable_and_start(&timer_unit)?;

    println!("Timer '{}' is now active.", name);
    println!("  Schedule: {}", schedule);
    println!("  Command:  {}", command);

    Ok(())
}

fn run_service(
    command: &str,
    name: Option<String>,
    workdir: Option<String>,
    description: Option<String>,
    env_file: Option<String>,
    restart: Option<String>,
) -> Result<()> {
    if let Some(ref r) = restart {
        match r.as_str() {
            "always" | "on-failure" | "no" => {}
            _ => bail!(
                "Invalid restart policy '{}'. Must be one of: always, on-failure, no",
                r
            ),
        }
    }

    if let Some(ref path) = env_file {
        if !Path::new(path).exists() {
            bail!("Environment file not found: {}", path);
        }
    }

    let name = name.unwrap_or_else(|| unit::derive_name(command));

    let unit_dir = init::unit_dir()?;
    let service_path = Path::new(&unit_dir).join(unit::service_filename(&name));
    if service_path.exists() {
        bail!(
            "Service '{}' already exists. Remove it first with: sdtab remove {}",
            name,
            name
        );
    }

    let workdir = resolve_workdir(workdir)?;
    let description = description.unwrap_or_else(|| command.to_string());

    let config = unit::UnitConfig {
        name: name.clone(),
        command: command.to_string(),
        workdir,
        description,
        unit_type: unit::UnitType::Service,
        cron_expr: None,
        schedule: None,
        restart_policy: restart.clone(),
        env_file: env_file.clone(),
    };

    let service_content = unit::generate_daemon_service(&config);

    fs::write(&service_path, &service_content)
        .with_context(|| format!("Failed to write {}", service_path.display()))?;

    println!("Created: {}", service_path.display());

    systemctl::daemon_reload()?;
    let service_unit = unit::service_filename(&name);
    systemctl::enable_and_start(&service_unit)?;

    let restart_display = restart.as_deref().unwrap_or("always");
    println!("Service '{}' is now active.", name);
    println!("  Command: {}", command);
    println!("  Restart: {}", restart_display);
    if let Some(ref ef) = env_file {
        println!("  EnvFile: {}", ef);
    }

    Ok(())
}

fn resolve_workdir(workdir: Option<String>) -> Result<String> {
    match workdir {
        Some(w) => Ok(w),
        None => Ok(std::env::current_dir()
            .context("Failed to get current directory")?
            .to_string_lossy()
            .to_string()),
    }
}
