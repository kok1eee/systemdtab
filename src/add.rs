use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::{cron, init, systemctl, unit};

pub struct AddOptions {
    pub schedule: String,
    pub command: String,
    pub name: Option<String>,
    pub workdir: Option<String>,
    pub description: Option<String>,
    pub env_file: Option<String>,
    pub restart: Option<String>,
    pub memory_max: Option<String>,
    pub cpu_quota: Option<String>,
    pub io_weight: Option<String>,
    pub timeout_stop: Option<String>,
    pub exec_start_pre: Option<String>,
    pub exec_stop_post: Option<String>,
    pub log_level_max: Option<String>,
    pub random_delay: Option<String>,
}

pub fn run(opts: AddOptions) -> Result<()> {
    if opts.schedule.trim() == "@service" {
        run_service(opts)
    } else {
        run_timer(opts)
    }
}

fn run_timer(opts: AddOptions) -> Result<()> {
    let parsed = cron::parse(&opts.schedule)?;
    let name = opts.name.unwrap_or_else(|| unit::derive_name(&opts.command));

    let unit_dir = init::unit_dir()?;
    let service_path = Path::new(&unit_dir).join(unit::service_filename(&name));
    let timer_path = Path::new(&unit_dir).join(unit::timer_filename(&name));

    if service_path.exists() || timer_path.exists() {
        bail!(
            "Unit '{}' already exists. Remove it first with: sdtab remove {}",
            name,
            name
        );
    }

    let workdir = resolve_workdir(opts.workdir)?;
    let description = opts.description.unwrap_or_else(|| opts.command.clone());
    let display_schedule = parsed.display.clone().unwrap_or_else(|| opts.schedule.clone());

    let config = unit::UnitConfig {
        name: name.clone(),
        command: opts.command.clone(),
        workdir,
        description,
        unit_type: unit::UnitType::Timer,
        cron_expr: Some(display_schedule.clone()),
        schedule: Some(parsed),
        restart_policy: None,
        env_file: None,
        memory_max: opts.memory_max,
        cpu_quota: opts.cpu_quota,
        io_weight: opts.io_weight,
        timeout_stop: opts.timeout_stop,
        exec_start_pre: opts.exec_start_pre,
        exec_stop_post: opts.exec_stop_post,
        log_level_max: opts.log_level_max,
        random_delay: opts.random_delay,
    };

    let service_content = unit::generate_service(&config);
    let timer_content = unit::generate_timer(&config);

    let service_path = Path::new(&unit_dir).join(unit::service_filename(&name));
    fs::write(&service_path, &service_content)
        .with_context(|| format!("Failed to write {}", service_path.display()))?;
    println!("Created: {}", service_path.display());

    fs::write(&timer_path, &timer_content)
        .with_context(|| format!("Failed to write {}", timer_path.display()))?;
    println!("Created: {}", timer_path.display());

    systemctl::daemon_reload()?;
    let timer_unit = unit::timer_filename(&name);
    systemctl::enable_and_start(&timer_unit)?;

    println!("Timer '{}' is now active.", name);
    println!("  Schedule: {}", display_schedule);
    println!("  Command:  {}", opts.command);

    Ok(())
}

fn run_service(opts: AddOptions) -> Result<()> {
    if let Some(ref r) = opts.restart {
        match r.as_str() {
            "always" | "on-failure" | "no" => {}
            _ => bail!(
                "Invalid restart policy '{}'. Must be one of: always, on-failure, no",
                r
            ),
        }
    }

    if let Some(ref path) = opts.env_file {
        if !Path::new(path).exists() {
            bail!("Environment file not found: {}", path);
        }
    }

    let name = opts.name.unwrap_or_else(|| unit::derive_name(&opts.command));

    let unit_dir = init::unit_dir()?;
    let service_path = Path::new(&unit_dir).join(unit::service_filename(&name));
    if service_path.exists() {
        bail!(
            "Service '{}' already exists. Remove it first with: sdtab remove {}",
            name,
            name
        );
    }

    let workdir = resolve_workdir(opts.workdir)?;
    let description = opts.description.unwrap_or_else(|| opts.command.clone());

    let config = unit::UnitConfig {
        name: name.clone(),
        command: opts.command.clone(),
        workdir,
        description,
        unit_type: unit::UnitType::Service,
        cron_expr: None,
        schedule: None,
        restart_policy: opts.restart.clone(),
        env_file: opts.env_file.clone(),
        memory_max: opts.memory_max,
        cpu_quota: opts.cpu_quota,
        io_weight: opts.io_weight,
        timeout_stop: opts.timeout_stop,
        exec_start_pre: opts.exec_start_pre,
        exec_stop_post: opts.exec_stop_post,
        log_level_max: opts.log_level_max,
        random_delay: None, // timer only
    };

    let service_content = unit::generate_daemon_service(&config);

    fs::write(&service_path, &service_content)
        .with_context(|| format!("Failed to write {}", service_path.display()))?;

    println!("Created: {}", service_path.display());

    systemctl::daemon_reload()?;
    let service_unit = unit::service_filename(&name);
    systemctl::enable_and_start(&service_unit)?;

    let restart_display = opts.restart.as_deref().unwrap_or("always");
    println!("Service '{}' is now active.", name);
    println!("  Command: {}", opts.command);
    println!("  Restart: {}", restart_display);
    if let Some(ref ef) = opts.env_file {
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
