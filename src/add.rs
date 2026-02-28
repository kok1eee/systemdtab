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
) -> Result<()> {
    // 1. Parse cron expression
    let parsed = cron::parse(schedule)?;

    // 2. Determine name
    let name = name.unwrap_or_else(|| unit::derive_name(command));

    // 3. Check for existing timer
    let unit_dir = init::unit_dir()?;
    let timer_path = Path::new(&unit_dir).join(unit::timer_filename(&name));
    if timer_path.exists() {
        bail!(
            "Timer '{}' already exists. Remove it first with: sdtab remove {}",
            name,
            name
        );
    }

    // 4. Determine working directory
    let workdir = match workdir {
        Some(w) => w,
        None => std::env::current_dir()
            .context("Failed to get current directory")?
            .to_string_lossy()
            .to_string(),
    };

    let description = description.unwrap_or_else(|| command.to_string());

    // 5. Generate unit files
    let config = unit::UnitConfig {
        name: name.clone(),
        command: command.to_string(),
        workdir,
        description,
        cron_expr: schedule.to_string(),
        schedule: parsed,
    };

    let service_content = unit::generate_service(&config);
    let timer_content = unit::generate_timer(&config);

    // 6. Write unit files
    let service_path = Path::new(&unit_dir).join(unit::service_filename(&name));
    fs::write(&service_path, &service_content)
        .with_context(|| format!("Failed to write {}", service_path.display()))?;
    fs::write(&timer_path, &timer_content)
        .with_context(|| format!("Failed to write {}", timer_path.display()))?;

    println!("Created: {}", service_path.display());
    println!("Created: {}", timer_path.display());

    // 7. Reload and enable
    systemctl::daemon_reload()?;
    let timer_unit = unit::timer_filename(&name);
    systemctl::enable_and_start(&timer_unit)?;

    println!("Timer '{}' is now active.", name);
    println!("  Schedule: {}", schedule);
    println!("  Command:  {}", command);

    Ok(())
}
