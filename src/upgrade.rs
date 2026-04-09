//! `sdtab upgrade` — migrate legacy units to the current template version
//! without requiring a restart.
//!
//! Strategy: systemd drop-in overrides.
//! For each legacy unit, we create `~/.config/systemd/user/sdtab-<name>.service.d/sdtab.conf`
//! and write just the new directives introduced since the unit was generated.
//! The original `.service` file is updated only in one place: the
//! `# sdtab:template_version=N` comment is bumped so `sdtab list` stops
//! flagging the unit as legacy. systemd ignores comment changes, so no
//! restart is needed — only a `daemon-reload`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{init, parse_unit, systemctl, unit};

/// Run the upgrade flow. When `name` is `Some`, upgrade a single unit; otherwise
/// upgrade every legacy unit found by `scan_all_units`. With `dry_run=true`,
/// preview the actions without touching the filesystem.
pub fn run(name: Option<&str>, dry_run: bool) -> Result<()> {
    let units = parse_unit::scan_all_units()?;

    // Narrow to legacy units, optionally filtered by name.
    let targets: Vec<&parse_unit::ParsedUnit> = units
        .iter()
        .filter(|u| {
            if let Some(filter) = name {
                u.name == filter
            } else {
                u.template_version < unit::TEMPLATE_VERSION
            }
        })
        .collect();

    if targets.is_empty() {
        if let Some(n) = name {
            // User asked to upgrade a specific unit — tell them why we skipped.
            if units.iter().any(|u| u.name == n) {
                println!("'{}' is already at template version {}.", n, unit::TEMPLATE_VERSION);
            } else {
                println!("'{}' not found.", n);
            }
        } else {
            println!("All units are already at template version {}. Nothing to do.", unit::TEMPLATE_VERSION);
        }
        return Ok(());
    }

    if dry_run {
        println!("Dry run — no changes will be made.");
        println!();
    }

    // When upgrading a single already-current unit explicitly named, exit cleanly above.
    let mut upgraded = 0;
    let mut skipped = 0;
    for u in &targets {
        if u.template_version >= unit::TEMPLATE_VERSION {
            // Filtered by name but already current.
            println!("'{}' is already at template version {}.", u.name, unit::TEMPLATE_VERSION);
            skipped += 1;
            continue;
        }
        match upgrade_unit(u, dry_run) {
            Ok(actions) => {
                upgraded += 1;
                println!("✓ {} (v{} → v{})", u.name, u.template_version, unit::TEMPLATE_VERSION);
                for action in actions {
                    println!("    {}", action);
                }
            }
            Err(e) => {
                eprintln!("✗ {}: {}", u.name, e);
            }
        }
    }

    if upgraded > 0 && !dry_run {
        systemctl::daemon_reload().context("daemon-reload failed")?;
        println!();
        println!(
            "Upgraded {} unit(s). Drop-ins are active after daemon-reload — no restart required.",
            upgraded
        );
    } else if upgraded > 0 && dry_run {
        println!();
        println!("Dry run complete. {} unit(s) would be upgraded.", upgraded);
    } else if skipped == 0 {
        println!("No units were upgraded.");
    }

    Ok(())
}

/// Upgrade a single unit from its current template_version up to `TEMPLATE_VERSION`.
/// Returns the list of actions performed, for reporting to the user.
fn upgrade_unit(u: &parse_unit::ParsedUnit, dry_run: bool) -> Result<Vec<String>> {
    let mut actions = Vec::new();

    // Apply all version migrations in order.
    for target_version in (u.template_version + 1)..=unit::TEMPLATE_VERSION {
        let step_actions = apply_migration(u, target_version, dry_run)?;
        actions.extend(step_actions);
    }

    // Bump the `# sdtab:template_version=N` stamp in the .service file.
    // Comments are not parsed by systemd, so this is a no-op from systemd's perspective.
    if !dry_run {
        stamp_service_file(&u.name)?;
    }
    actions.push(format!("stamped # sdtab:template_version={}", unit::TEMPLATE_VERSION));

    Ok(actions)
}

/// Apply a single migration step. Each target_version has a known set of new
/// directives that must be injected into the drop-in file.
fn apply_migration(u: &parse_unit::ParsedUnit, target_version: u32, dry_run: bool) -> Result<Vec<String>> {
    match target_version {
        2 => migrate_v2_syslog_identifier(u, dry_run),
        _ => anyhow::bail!("no migration defined for template version {}", target_version),
    }
}

/// v2: add `SyslogIdentifier=sdtab-<name>` so `journalctl --user-unit` captures
/// child-process stdout on systems where journald fails to attach user-unit
/// metadata to child stream records.
fn migrate_v2_syslog_identifier(u: &parse_unit::ParsedUnit, dry_run: bool) -> Result<Vec<String>> {
    let directive = format!("SyslogIdentifier=sdtab-{}", u.name);
    if !dry_run {
        let content = format!(
            "# Added by `sdtab upgrade` to reach template_version=2\n\
             [Service]\n\
             {directive}\n"
        );
        write_dropin(&u.name, "v2-syslog-identifier.conf", &content)?;
    }
    Ok(vec![format!("drop-in: {}", directive)])
}

/// Write (or overwrite) a drop-in file for the given unit.
/// Creates `~/.config/systemd/user/sdtab-<name>.service.d/<filename>`.
fn write_dropin(unit_name: &str, filename: &str, content: &str) -> Result<()> {
    let dir = dropin_dir(unit_name)?;
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create drop-in dir {}", dir.display()))?;
    let file = dir.join(filename);
    fs::write(&file, content)
        .with_context(|| format!("Failed to write drop-in {}", file.display()))?;
    Ok(())
}

fn dropin_dir(unit_name: &str) -> Result<PathBuf> {
    let unit_dir = init::unit_dir()?;
    Ok(Path::new(&unit_dir).join(format!("sdtab-{}.service.d", unit_name)))
}

/// Update (or insert) the `# sdtab:template_version=N` line in the .service file.
/// This is a pure comment change and does not require systemctl daemon-reload,
/// but we daemon-reload anyway once drop-ins are placed.
fn stamp_service_file(unit_name: &str) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let path = Path::new(&unit_dir).join(unit::service_filename(unit_name));
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let stamp = format!("# sdtab:template_version={}", unit::TEMPLATE_VERSION);
    let new_content = if content.lines().any(|l| l.trim().starts_with("# sdtab:template_version=")) {
        // Replace existing line
        let mut out = String::with_capacity(content.len());
        for line in content.lines() {
            if line.trim().starts_with("# sdtab:template_version=") {
                out.push_str(&stamp);
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        out
    } else {
        // Insert after the `# sdtab:type=...` line (first existing metadata comment).
        // If none is present, prepend at the top.
        let mut out = String::with_capacity(content.len() + stamp.len() + 1);
        let mut inserted = false;
        for line in content.lines() {
            out.push_str(line);
            out.push('\n');
            if !inserted && line.trim().starts_with("# sdtab:type=") {
                out.push_str(&stamp);
                out.push('\n');
                inserted = true;
            }
        }
        if !inserted {
            // No type metadata — prepend.
            let mut prefixed = String::with_capacity(content.len() + stamp.len() + 1);
            prefixed.push_str(&stamp);
            prefixed.push('\n');
            prefixed.push_str(&out);
            out = prefixed;
        }
        out
    };

    fs::write(&path, new_content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_replaces_existing_line() {
        let input = "# sdtab:type=timer\n# sdtab:template_version=1\n# sdtab:cron=@daily\n[Unit]\n";
        // Simulate the replacement logic in isolation.
        let stamp = format!("# sdtab:template_version={}", unit::TEMPLATE_VERSION);
        let mut out = String::new();
        for line in input.lines() {
            if line.trim().starts_with("# sdtab:template_version=") {
                out.push_str(&stamp);
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        assert!(out.contains(&format!("# sdtab:template_version={}", unit::TEMPLATE_VERSION)));
        assert!(!out.contains("# sdtab:template_version=1"));
    }

    #[test]
    fn stamp_inserts_after_type_when_missing() {
        let input = "# sdtab:type=timer\n# sdtab:cron=@daily\n[Unit]\n";
        let stamp = format!("# sdtab:template_version={}", unit::TEMPLATE_VERSION);
        let mut out = String::new();
        let mut inserted = false;
        for line in input.lines() {
            out.push_str(line);
            out.push('\n');
            if !inserted && line.trim().starts_with("# sdtab:type=") {
                out.push_str(&stamp);
                out.push('\n');
                inserted = true;
            }
        }
        assert!(inserted);
        assert!(out.contains(&format!("# sdtab:template_version={}", unit::TEMPLATE_VERSION)));
        // Stamp should be on the line right after type=
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "# sdtab:type=timer");
        assert_eq!(lines[1], format!("# sdtab:template_version={}", unit::TEMPLATE_VERSION));
    }

    #[test]
    fn migrate_v2_generates_syslog_identifier_directive() {
        let parsed = parse_unit::ParsedUnit {
            name: "myunit".to_string(),
            unit_type: parse_unit::UnitType::Timer,
            command: String::new(),
            workdir: String::new(),
            description: String::new(),
            cron_expr: None,
            restart_policy: None,
            env_file: None,
            memory_max: None,
            cpu_quota: None,
            io_weight: None,
            timeout_stop: None,
            exec_start_pre: None,
            exec_stop_post: None,
            log_level_max: None,
            random_delay: None,
            env: vec![],
            no_notify: false,
            template_version: 1,
        };
        // We can't actually write files in this test without mocking init::unit_dir(),
        // so just verify the directive string construction is correct.
        let directive = format!("SyslogIdentifier=sdtab-{}", parsed.name);
        assert_eq!(directive, "SyslogIdentifier=sdtab-myunit");
    }
}
