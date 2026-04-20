use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::{config, init, parse_unit, systemctl, unit};

pub fn run() -> Result<()> {
    let color = std::io::stdout().is_terminal();
    let mut warnings: u32 = 0;
    let mut errors: u32 = 0;

    println!("{}", heading("sdtab doctor", color));

    // 1. linger
    match check_linger() {
        Ok(true) => ok_line("linger enabled", color),
        Ok(false) => {
            warn_line("linger not enabled — run `sdtab init` (or `loginctl enable-linger $USER`)", color);
            warnings += 1;
        }
        Err(e) => {
            warn_line(&format!("could not check linger: {}", e), color);
            warnings += 1;
        }
    }

    // 2. unit dir
    let unit_dir = init::unit_dir()?;
    if Path::new(&unit_dir).is_dir() {
        ok_line(&format!("unit directory: {}", unit_dir), color);
    } else {
        err_line(&format!("unit directory missing: {}", unit_dir), color);
        errors += 1;
    }

    // 3. systemctl --user responding
    match Command::new("systemctl").args(["--user", "is-system-running"]).output() {
        Ok(out) => {
            let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
            // degraded is common on user instance; accept anything except a failure to run
            if out.status.code().is_some() {
                ok_line(&format!("systemctl --user responding ({})", state), color);
            } else {
                warn_line("systemctl --user returned no status", color);
                warnings += 1;
            }
        }
        Err(e) => {
            err_line(&format!("systemctl --user not responding: {}", e), color);
            errors += 1;
        }
    }

    // 4. config / webhook
    let cfg = config::load().unwrap_or_default();
    let cfg_path = config::config_path()?;
    if Path::new(&cfg_path).exists() {
        ok_line(&format!("config: {}", cfg_path), color);
    } else {
        info_line(&format!("config: {} (not yet created — defaults in use)", cfg_path), color);
    }

    if let Some(ref _url) = cfg.notify.slack_webhook {
        ok_line("slack webhook: configured", color);
        let template = format!("{}/sdtab-notify@.service", unit_dir);
        if Path::new(&template).exists() {
            ok_line("notify template sdtab-notify@.service present", color);
        } else {
            warn_line(
                "webhook configured but sdtab-notify@.service is missing — re-run `sdtab init`",
                color,
            );
            warnings += 1;
        }
    } else {
        info_line("slack webhook: not configured (failure notifications disabled)", color);
    }

    // 5. failed units
    let units = parse_unit::scan_all_units().unwrap_or_default();
    let mut failed: Vec<String> = Vec::new();
    for u in &units {
        let service_unit = unit::service_filename(&u.name);
        let timer_unit = unit::timer_filename(&u.name);
        let svc_state =
            systemctl::show_property(&service_unit, "ActiveState").unwrap_or_default();
        let timer_state =
            systemctl::show_property(&timer_unit, "ActiveState").unwrap_or_default();
        if svc_state == "failed" || timer_state == "failed" {
            let which = if svc_state == "failed" { "service" } else { "timer" };
            failed.push(format!("{} ({}: failed)", u.name, which));
        }
    }

    if units.is_empty() {
        info_line("no sdtab-managed units yet", color);
    } else if failed.is_empty() {
        ok_line(&format!("all {} managed unit(s) healthy", units.len()), color);
    } else {
        warn_line(&format!("failed units ({}):", failed.len()), color);
        for f in &failed {
            println!("    - {}", f);
        }
        warnings += failed.len() as u32;
    }

    // Summary
    println!();
    match (errors, warnings) {
        (0, 0) => println!("{}", green("All checks passed.", color)),
        (0, w) => println!(
            "{}",
            yellow(&format!("{} warning(s). Investigate with `sdtab logs <name>`.", w), color)
        ),
        (e, w) => {
            println!(
                "{}",
                red(&format!("{} error(s), {} warning(s).", e, w), color)
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

fn check_linger() -> Result<bool> {
    let user = std::env::var("USER").unwrap_or_default();
    let out = Command::new("loginctl")
        .args(["show-user", &user, "--property=Linger", "--value"])
        .output()?;
    let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(val == "yes")
}

fn heading(text: &str, color: bool) -> String {
    if color {
        format!("\x1b[1m{}\x1b[0m", text)
    } else {
        text.to_string()
    }
}

fn ok_line(msg: &str, color: bool) {
    println!("  {} {}", green("✓", color), msg);
}

fn warn_line(msg: &str, color: bool) {
    println!("  {} {}", yellow("!", color), msg);
}

fn err_line(msg: &str, color: bool) {
    println!("  {} {}", red("✗", color), msg);
}

fn info_line(msg: &str, color: bool) {
    println!("  {} {}", dim("·", color), msg);
}

fn green(s: &str, color: bool) -> String {
    if color { format!("\x1b[32m{}\x1b[0m", s) } else { s.to_string() }
}
fn yellow(s: &str, color: bool) -> String {
    if color { format!("\x1b[33m{}\x1b[0m", s) } else { s.to_string() }
}
fn red(s: &str, color: bool) -> String {
    if color { format!("\x1b[31m{}\x1b[0m", s) } else { s.to_string() }
}
fn dim(s: &str, color: bool) -> String {
    if color { format!("\x1b[2m{}\x1b[0m", s) } else { s.to_string() }
}
