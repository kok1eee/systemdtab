mod add;
mod cron;
mod init;
mod list;
mod remove;
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
    /// Add a new timer
    Add {
        /// Cron schedule expression (e.g. "0 9 * * *" or "@daily")
        schedule: String,
        /// Command to execute
        command: String,
        /// Timer name (auto-generated from command if omitted)
        #[arg(long)]
        name: Option<String>,
        /// Working directory (defaults to current directory)
        #[arg(long)]
        workdir: Option<String>,
        /// Description
        #[arg(long)]
        description: Option<String>,
    },
    /// List all managed timers
    List,
    /// Remove a timer
    Remove {
        /// Timer name to remove
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
        } => add::run(&schedule, &command, name, workdir, description)?,
        Commands::List => list::run()?,
        Commands::Remove { name } => remove::run(&name)?,
    }

    Ok(())
}
