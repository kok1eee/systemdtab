mod add;
mod cron;
mod init;
mod list;
mod remove;
mod systemctl;
mod unit;

use anyhow::{bail, Result};
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
    /// Timer:   sdtab add "<schedule>" "<command>"
    /// Service: sdtab add --service "<command>"
    Add {
        /// For timers: <schedule> <command>. For services: <command>
        #[arg(required = true, num_args = 1..=2)]
        args: Vec<String>,
        /// Run as a persistent daemon service instead of a timer
        #[arg(long)]
        service: bool,
        /// Timer/service name (auto-generated from command if omitted)
        #[arg(long)]
        name: Option<String>,
        /// Working directory (defaults to current directory)
        #[arg(long)]
        workdir: Option<String>,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Environment file path (services only)
        #[arg(long)]
        env_file: Option<String>,
        /// Restart policy: always, on-failure, no (services only, default: always)
        #[arg(long)]
        restart: Option<String>,
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
            args,
            service,
            name,
            workdir,
            description,
            env_file,
            restart,
        } => {
            let (schedule, command) = if service {
                if args.len() != 1 {
                    bail!("--service requires exactly one argument: <command>");
                }
                (None, args[0].clone())
            } else {
                if args.len() != 2 {
                    bail!("Timer requires two arguments: <schedule> <command>");
                }
                (Some(args[0].as_str()), args[1].clone())
            };
            add::run(schedule, &command, service, name, workdir, description, env_file, restart)?
        }
        Commands::List => list::run()?,
        Commands::Remove { name } => remove::run(&name)?,
    }

    Ok(())
}
