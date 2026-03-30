use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::{init, systemctl, unit};

pub fn run(name: &str) -> Result<()> {
    let unit_dir = init::unit_dir()?;
    let dir_path = Path::new(&unit_dir);

    let service_path = dir_path.join(unit::service_filename(name));
    let timer_path = dir_path.join(unit::timer_filename(name));

    if !service_path.exists() {
        bail!("'{}' not found.", name);
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let is_timer = timer_path.exists();

    // Save metadata lines before editing so we can restore them if deleted
    let original_content = fs::read_to_string(&service_path)
        .with_context(|| format!("Failed to read {}", service_path.display()))?;
    let metadata_lines: Vec<&str> = original_content
        .lines()
        .filter(|l| l.starts_with("# sdtab:"))
        .collect();

    eprintln!("Note: lines starting with '# sdtab:' are metadata used by sdtab list/export.");

    let mut cmd = Command::new(&editor);
    cmd.arg(&service_path);
    if is_timer {
        cmd.arg(&timer_path);
    }

    let status = cmd.status().context("Failed to open editor")?;
    if !status.success() {
        bail!("Editor exited with non-zero status");
    }

    // Restore any metadata keys that were removed during editing
    if !metadata_lines.is_empty() {
        let edited_content = fs::read_to_string(&service_path)
            .with_context(|| format!("Failed to read {}", service_path.display()))?;

        // Compare by key (e.g. "# sdtab:cron") to detect partial deletion
        let edited_keys: Vec<&str> = edited_content
            .lines()
            .filter(|l| l.starts_with("# sdtab:"))
            .filter_map(|l| l.split('=').next())
            .collect();

        let missing: Vec<&&str> = metadata_lines
            .iter()
            .filter(|l| {
                let key = l.split('=').next().unwrap_or("");
                !edited_keys.contains(&key)
            })
            .collect();

        if !missing.is_empty() {
            let missing_text: Vec<&str> = missing.iter().map(|l| **l).collect();
            let restored = format!("{}\n{}", missing_text.join("\n"), edited_content);
            fs::write(&service_path, &restored)
                .with_context(|| format!("Failed to write {}", service_path.display()))?;
            eprintln!("Warning: Restored missing sdtab metadata:");
            for line in &missing_text {
                eprintln!("  {}", line);
            }
            eprintln!("  These are required for sdtab list/export to work correctly.");
            eprintln!("  To permanently remove this unit: sdtab remove {}", name);
        }
    }

    // Reload daemon after editing
    systemctl::daemon_reload()?;
    println!("Reloaded systemd user daemon.");

    if is_timer {
        // Touch the Persistent= stamp file to prevent re-execution of past schedules.
        // Without this, changing OnCalendar causes systemd to treat the new time
        // as a missed execution and fire immediately.
        touch_timer_stamp(name);
    } else {
        println!("Hint: run `sdtab restart {}` to apply changes.", name);
    }

    Ok(())
}

/// Touch the systemd timer stamp file to mark "last triggered = now".
/// This prevents Persistent=true from re-firing after a schedule change.
fn touch_timer_stamp(name: &str) {
    let home = std::env::var("HOME").unwrap_or_default();
    let stamp_file = Path::new(&home)
        .join(".local/share/systemd/timers")
        .join(format!("stamp-{}", unit::timer_filename(name)));

    if stamp_file.exists() {
        // Open and close the file with write to update mtime
        match fs::OpenOptions::new().write(true).open(&stamp_file) {
            Ok(f) => {
                // Set mtime to now via file metadata
                let now = std::time::SystemTime::now();
                if let Err(e) = f.set_modified(now) {
                    eprintln!("Warning: failed to update stamp file mtime: {}", e);
                } else {
                    println!("Updated timer stamp to prevent Persistent= re-execution.");
                }
            }
            Err(e) => {
                eprintln!("Warning: failed to open stamp file: {}", e);
            }
        }
    }
}
