//! `sdtab restart` — restart one or more daemon services.
//!
//! Accepts literal names, glob patterns (`hikken-*`), or `--all`.
//! Timers are scheduled by systemd and cannot be restarted, so any matched
//! timer is skipped with a warning. The loop continues past individual
//! failures so users see the status of every requested service before the
//! command exits non-zero.

use anyhow::{bail, Result};

use crate::{parse_unit, systemctl, unit};

pub fn run(names: &[String], all: bool) -> Result<()> {
    let units = parse_unit::scan_all_units()?;

    let selected: Vec<&parse_unit::ParsedUnit> = if all {
        if !names.is_empty() {
            bail!("--all cannot be combined with explicit names.");
        }
        units
            .iter()
            .filter(|u| matches!(u.unit_type, parse_unit::UnitType::Service))
            .collect()
    } else {
        resolve_names(&units, names)?
    };

    if selected.is_empty() {
        bail!("No services matched.");
    }

    let mut failed = 0usize;
    for svc in &selected {
        let unit_name = unit::service_filename(&svc.name);
        match systemctl::restart(&unit_name) {
            Ok(_) => println!("✓ restarted {}", svc.name),
            Err(e) => {
                eprintln!("✗ {}: {}", svc.name, e);
                failed += 1;
            }
        }
    }

    println!();
    if failed == 0 {
        println!("Restarted {} service(s).", selected.len());
        Ok(())
    } else {
        println!(
            "Restarted {} of {} service(s); {} failed.",
            selected.len() - failed,
            selected.len(),
            failed
        );
        bail!("{} service(s) failed to restart.", failed);
    }
}

fn resolve_names<'a>(
    all_units: &'a [parse_unit::ParsedUnit],
    names: &[String],
) -> Result<Vec<&'a parse_unit::ParsedUnit>> {
    let mut resolved: Vec<&parse_unit::ParsedUnit> = Vec::new();

    for pattern in names {
        if is_glob(pattern) {
            let mut glob_matches: Vec<&parse_unit::ParsedUnit> = Vec::new();
            let mut skipped_timers: Vec<String> = Vec::new();
            for u in all_units {
                if glob_match(pattern, &u.name) {
                    match u.unit_type {
                        parse_unit::UnitType::Service => glob_matches.push(u),
                        parse_unit::UnitType::Timer => skipped_timers.push(u.name.clone()),
                    }
                }
            }
            if glob_matches.is_empty() {
                if skipped_timers.is_empty() {
                    bail!("pattern '{}' matched no units.", pattern);
                } else {
                    bail!("pattern '{}' matched only timers; nothing to restart.", pattern);
                }
            }
            if !skipped_timers.is_empty() {
                eprintln!(
                    "note: pattern '{}' matched {} timer(s) which cannot be restarted: {}",
                    pattern,
                    skipped_timers.len(),
                    skipped_timers.join(", ")
                );
            }
            for m in glob_matches {
                push_unique(&mut resolved, m);
            }
        } else {
            let Some(unit) = all_units.iter().find(|u| u.name == *pattern) else {
                bail!("'{}' not found.", pattern);
            };
            if matches!(unit.unit_type, parse_unit::UnitType::Timer) {
                bail!("'{}' is a timer; only services can be restarted.", pattern);
            }
            push_unique(&mut resolved, unit);
        }
    }

    Ok(resolved)
}

fn push_unique<'a>(
    resolved: &mut Vec<&'a parse_unit::ParsedUnit>,
    unit: &'a parse_unit::ParsedUnit,
) {
    if !resolved.iter().any(|r| r.name == unit.name) {
        resolved.push(unit);
    }
}

fn is_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?')
}

/// Simple glob matching supporting `*` (any sequence) and `?` (single char).
/// Iterative backtracking; O(n*m) worst case, fine for sdtab-scale unit names.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = name.chars().collect();
    let mut pi = 0usize;
    let mut si = 0usize;
    let mut star: Option<(usize, usize)> = None;

    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi + 1, si));
            pi += 1;
        } else if let Some((star_pi, star_si)) = star {
            pi = star_pi;
            si = star_si + 1;
            star = Some((star_pi, star_si + 1));
        } else {
            return false;
        }
    }

    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_exact_match() {
        assert!(glob_match("foo", "foo"));
        assert!(!glob_match("foo", "bar"));
    }

    #[test]
    fn glob_prefix_star() {
        assert!(glob_match("hikken-*", "hikken-others"));
        assert!(glob_match("hikken-*", "hikken-"));
        assert!(!glob_match("hikken-*", "other"));
    }

    #[test]
    fn glob_suffix_star() {
        assert!(glob_match("*-report", "daily-report"));
        assert!(glob_match("*-report", "-report"));
        assert!(!glob_match("*-report", "report-daily"));
    }

    #[test]
    fn glob_middle_star() {
        assert!(glob_match("foo-*-bar", "foo-xyz-bar"));
        assert!(glob_match("foo-*-bar", "foo--bar"));
        assert!(!glob_match("foo-*-bar", "foo-bar"));
    }

    #[test]
    fn glob_multi_star() {
        assert!(glob_match("*a*b*", "xxabxyb"));
        assert!(glob_match("*a*b*", "ab"));
        assert!(!glob_match("*a*b*", "ba"));
    }

    #[test]
    fn glob_question_mark() {
        assert!(glob_match("f?o", "foo"));
        assert!(glob_match("f?o", "fxo"));
        assert!(!glob_match("f?o", "fo"));
        assert!(!glob_match("f?o", "food"));
    }

    #[test]
    fn glob_empty_pattern() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "foo"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn is_glob_detects_metacharacters() {
        assert!(is_glob("hikken-*"));
        assert!(is_glob("f?o"));
        assert!(!is_glob("plain-name"));
    }
}
