use anyhow::Result;
use clap::ValueEnum;

use crate::parse_unit;

#[derive(Clone, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

const BASH_SCRIPT: &str = include_str!("completions/bash.sh");
const ZSH_SCRIPT: &str = include_str!("completions/zsh.sh");
const FISH_SCRIPT: &str = include_str!("completions/fish.sh");

pub fn run(shell: Shell) -> Result<()> {
    let script = match shell {
        Shell::Bash => BASH_SCRIPT,
        Shell::Zsh => ZSH_SCRIPT,
        Shell::Fish => FISH_SCRIPT,
    };
    print!("{}", script);
    Ok(())
}

/// Hidden subcommand: print one unit name per line (used by completion scripts)
pub fn print_names() -> Result<()> {
    let units = parse_unit::scan_all_units()?;
    for unit in units {
        println!("{}", unit.name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_script_non_empty() {
        assert!(!BASH_SCRIPT.is_empty());
        assert!(BASH_SCRIPT.contains("complete -F _sdtab sdtab"));
        assert!(BASH_SCRIPT.contains("sdtab __names"));
    }

    #[test]
    fn zsh_script_non_empty() {
        assert!(!ZSH_SCRIPT.is_empty());
        assert!(ZSH_SCRIPT.contains("#compdef sdtab"));
        assert!(ZSH_SCRIPT.contains("sdtab __names"));
    }

    #[test]
    fn fish_script_non_empty() {
        assert!(!FISH_SCRIPT.is_empty());
        assert!(FISH_SCRIPT.contains("complete -c sdtab"));
        assert!(FISH_SCRIPT.contains("sdtab __names"));
    }
}
