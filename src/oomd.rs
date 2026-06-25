use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use clap::Subcommand;

/// Global systemd-oomd policy management.
///
/// Unit-level ManagedOOM* directives (set via `sdtab add --managed-oom-*`) only
/// take effect once systemd-oomd is enabled and the enclosing slices opt in.
/// `sdtab oomd setup` wires up that global policy in one idempotent command:
/// oomd.conf thresholds, plus memory-pressure-kill / swap-kill on the user's
/// app.slice, enables the daemon, and applies it to live cgroups.
///
/// Design note — kill policies are scoped to the user's app.slice (where sdtab
/// timers/services live) and user@.service, NOT the root slice. Interactive
/// `session-N.scope` cgroups (terminals / Claude Code) sit under user-N.slice
/// OUTSIDE user@.service, so they are never an oomd kill target. This prevents
/// the collateral failure where a heavy batch fills RAM, pushes an idle session
/// into swap, and a root-level swap-kill then kills the *session* instead of the
/// batch (observed 2026-06-25).
#[derive(Subcommand)]
pub enum OomdCommand {
    /// Write global oomd drop-ins, enable systemd-oomd, and apply at runtime.
    /// Requires sudo for the /etc system drop-ins.
    Setup(SetupOptions),
}

#[derive(clap::Args)]
pub struct SetupOptions {
    /// Swap usage % that triggers oomd swap-kill within app.slice
    #[arg(long, default_value = "80%")]
    swap_used_limit: String,
    /// Memory pressure % limit before oomd kills the worst cgroup in a slice
    #[arg(long, default_value = "55%")]
    memory_pressure_limit: String,
    /// How long memory pressure must exceed the limit before killing
    #[arg(long, default_value = "20s")]
    memory_pressure_duration: String,
    /// Preview the drop-ins and actions without applying anything
    #[arg(long)]
    dry_run: bool,
}

const OOMD_CONF: &str = "/etc/systemd/oomd.conf.d/10-sdtab.conf";
const USER_SERVICE: &str = "/etc/systemd/system/user@.service.d/10-sdtab-oomd.conf";

pub fn run(cmd: OomdCommand) -> Result<()> {
    match cmd {
        OomdCommand::Setup(opts) => setup(opts),
    }
}

fn oomd_conf_content(swap: &str, pressure: &str, duration: &str) -> String {
    format!(
        "[OOM]\n\
         SwapUsedLimit={swap}\n\
         DefaultMemoryPressureLimit={pressure}\n\
         DefaultMemoryPressureDurationSec={duration}\n",
        swap = swap,
        pressure = pressure,
        duration = duration,
    )
}

fn setup(opts: SetupOptions) -> Result<()> {
    let uid = current_uid()?;
    let home = std::env::var("HOME").context("HOME not set")?;
    let user_app_slice = format!("{}/.config/systemd/user/app.slice.d/10-sdtab-oomd.conf", home);

    let oomd_conf = oomd_conf_content(
        &opts.swap_used_limit,
        &opts.memory_pressure_limit,
        &opts.memory_pressure_duration,
    );
    // user@.service: memory-pressure-kill (PID1-managed → survives reboot, covers
    // app.slice). app.slice: both pressure-kill and swap-kill, scoped to batches.
    let user_service = "[Service]\nManagedOOMMemoryPressure=kill\n".to_string();
    let app_slice = "[Slice]\nManagedOOMMemoryPressure=kill\nManagedOOMSwap=kill\n".to_string();

    // (path, content, is_system) — system files go to /etc and need sudo.
    let files: [(&str, &String, bool); 3] = [
        (OOMD_CONF, &oomd_conf, true),
        (USER_SERVICE, &user_service, true),
        (user_app_slice.as_str(), &app_slice, false),
    ];

    if opts.dry_run {
        println!("# sdtab oomd setup (dry-run)\n");
        for (path, content, is_system) in &files {
            println!("--- {} {} ---", path, if *is_system { "(sudo)" } else { "(user)" });
            print!("{}", content);
            println!();
        }
        println!("Then:");
        println!("  sudo systemctl daemon-reload && systemctl --user daemon-reload");
        println!("  sudo systemctl enable --now systemd-oomd");
        println!("  systemctl --user set-property --runtime app.slice ManagedOOMMemoryPressure=kill ManagedOOMSwap=kill");
        println!("  sudo systemctl set-property --runtime user@{}.service ManagedOOMMemoryPressure=kill", uid);
        println!("\nNote: kill policies are scoped to app.slice / user@.service; interactive");
        println!("sessions (outside user@.service) are never targeted.");
        return Ok(());
    }

    for (path, content, is_system) in &files {
        if *is_system {
            write_system_file(path, content)?;
        } else {
            write_user_file(path, content)?;
        }
        println!("Wrote {}", path);
    }

    // Reload both managers so the drop-ins take effect.
    run_checked(Command::new("sudo").args(["systemctl", "daemon-reload"]))?;
    run_checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;

    // Enable + start the daemon.
    run_checked(Command::new("sudo").args(["systemctl", "enable", "--now", "systemd-oomd"]))?;

    // Apply at runtime so already-active cgroups are monitored immediately
    // (daemon-reload alone does not rewrite the ManagedOOM xattr on live cgroups).
    run_checked(Command::new("systemctl").args([
        "--user", "set-property", "--runtime", "app.slice",
        "ManagedOOMMemoryPressure=kill", "ManagedOOMSwap=kill",
    ]))?;
    let user_svc = format!("user@{}.service", uid);
    run_checked(Command::new("sudo").args([
        "systemctl", "set-property", "--runtime", user_svc.as_str(), "ManagedOOMMemoryPressure=kill",
    ]))?;

    println!("\nsystemd-oomd is enabled and monitoring app.slice. Verify with: sudo oomctl");
    println!(
        "If you have older manual oomd drop-ins (e.g. 10-amu.conf, or a root-slice\n\
         swap-kill at /etc/systemd/system/-.slice.d/10-oomd.conf), remove them — the\n\
         root-slice swap-kill in particular can collaterally kill interactive sessions."
    );
    Ok(())
}

/// Write a /etc system file via `sudo tee` (sdtab itself runs unprivileged).
fn write_system_file(path: &str, content: &str) -> Result<()> {
    let dir = Path::new(path)
        .parent()
        .with_context(|| format!("No parent directory for {}", path))?
        .to_string_lossy()
        .to_string();
    run_checked(Command::new("sudo").args(["mkdir", "-p", dir.as_str()]))?;

    let mut child = Command::new("sudo")
        .arg("tee")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to spawn sudo tee for {}", path))?;
    child
        .stdin
        .as_mut()
        .context("failed to open sudo tee stdin")?
        .write_all(content.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        bail!("sudo tee failed for {}", path);
    }
    Ok(())
}

fn write_user_file(path: &str, content: &str) -> Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("Failed to write {}", path))?;
    Ok(())
}

fn run_checked(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("Failed to run {:?}", cmd))?;
    if !status.success() {
        bail!("command failed: {:?}", cmd);
    }
    Ok(())
}

fn current_uid() -> Result<u32> {
    let out = Command::new("id").arg("-u").output().context("Failed to run id -u")?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .context("Failed to parse uid from `id -u`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oomd_conf_includes_all_thresholds() {
        let c = oomd_conf_content("80%", "55%", "20s");
        assert!(c.contains("[OOM]"));
        assert!(c.contains("SwapUsedLimit=80%"));
        assert!(c.contains("DefaultMemoryPressureLimit=55%"));
        assert!(c.contains("DefaultMemoryPressureDurationSec=20s"));
    }

    #[test]
    fn oomd_conf_respects_custom_thresholds() {
        let c = oomd_conf_content("60%", "40%", "10s");
        assert!(c.contains("SwapUsedLimit=60%"));
        assert!(c.contains("DefaultMemoryPressureLimit=40%"));
        assert!(c.contains("DefaultMemoryPressureDurationSec=10s"));
    }
}
