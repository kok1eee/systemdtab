mod add;
mod cron;
mod edit;
mod init;
mod list;
mod logs;
mod remove;
mod restart;
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
enum Commands {
    /// Initialize sdtab (enable linger, create directories)
    Init,
    /// Add a new timer or service
    ///
    /// sdtab add "<schedule>" "<command>"
    /// Use @service for persistent daemons: sdtab add "@service" "<command>"
    Add {
        /// Schedule: cron expression, @daily, @reboot, @service, etc.
        schedule: String,
        /// Command to execute
        command: String,
        /// Timer/service name (auto-generated from command if omitted)
        #[arg(long)]
        name: Option<String>,
        /// Working directory (defaults to current directory)
        #[arg(long)]
        workdir: Option<String>,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Environment file path (@service only)
        #[arg(long)]
        env_file: Option<String>,
        /// Restart policy: always, on-failure, no (@service only, default: always)
        #[arg(long)]
        restart: Option<String>,
    },
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
    },
    /// Restart a timer or service
    Restart {
        /// Timer/service name to restart
        name: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init::run()?,
        Commands::Add {
            schedule,
            command,
            name,
            workdir,
            description,
            env_file,
            restart,
        } => add::run(&schedule, &command, name, workdir, description, env_file, restart)?,
        Commands::List => list::run()?,
        Commands::Remove { name } => remove::run(&name)?,
        Commands::Edit { name } => edit::run(&name)?,
        Commands::Logs { name, follow, lines } => logs::run(&name, follow, lines)?,
        Commands::Restart { name } => restart::run(&name)?,
    }

    Ok(())
}
