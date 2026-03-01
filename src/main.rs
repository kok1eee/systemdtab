mod add;
mod apply;
mod cron;
mod disable;
mod edit;
mod enable;
mod export;
mod init;
mod list;
mod logs;
mod parse_unit;
mod remove;
mod restart;
mod sdtabfile;
mod status;
mod systemctl;
mod unit;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sdtab", about = "systemd timer management made simple")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Initialize sdtab (enable linger, create directories)
    Init,
    /// Add a new timer or service
    ///
    /// sdtab add "<schedule>" "<command>"
    /// Use @service for persistent daemons: sdtab add "@service" "<command>"
    Add(add::AddOptions),
    /// List all managed timers
    List,
    /// Remove a timer or service
    Remove {
        /// Timer/service name to remove
        name: String,
    },
    /// Edit unit files with $EDITOR
    Edit {
        /// Timer/service name to edit
        name: String,
    },
    /// Show logs for a timer or service
    Logs {
        /// Timer/service name
        name: String,
        /// Follow log output (tail -f)
        #[arg(short, long)]
        follow: bool,
        /// Number of log lines to show
        #[arg(short = 'n', long, default_value = "50")]
        lines: u32,
        /// Filter by priority (emerg/alert/crit/err/warning/notice/info/debug)
        #[arg(short, long)]
        priority: Option<String>,
    },
    /// Restart a timer or service
    Restart {
        /// Timer/service name to restart
        name: String,
    },
    /// Show detailed status of a timer or service
    Status {
        /// Timer/service name
        name: String,
    },
    /// Enable (start) a timer or service
    Enable {
        /// Timer/service name to enable
        name: String,
    },
    /// Disable (stop) a timer or service without removing
    Disable {
        /// Timer/service name to disable
        name: String,
    },
    /// Export current configuration to TOML
    Export {
        /// Output file path (stdout if omitted)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Apply configuration from a TOML file
    Apply {
        /// Path to Sdtabfile.toml
        file: String,
        /// Remove units not in the file
        #[arg(long)]
        prune: bool,
        /// Show changes without applying
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init::run()?,
        Commands::Add(opts) => add::run(opts)?,
        Commands::List => list::run()?,
        Commands::Remove { name } => remove::run(&name)?,
        Commands::Edit { name } => edit::run(&name)?,
        Commands::Logs { name, follow, lines, priority } => logs::run(&name, follow, lines, priority)?,
        Commands::Restart { name } => restart::run(&name)?,
        Commands::Status { name } => status::run(&name)?,
        Commands::Enable { name } => enable::run(&name)?,
        Commands::Disable { name } => disable::run(&name)?,
        Commands::Export { output } => export::run(output.as_deref())?,
        Commands::Apply { file, prune, dry_run } => apply::run(&file, prune, dry_run)?,
    }

    Ok(())
}
