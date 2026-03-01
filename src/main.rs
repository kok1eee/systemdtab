mod add;
mod cron;
mod disable;
mod edit;
mod enable;
mod init;
mod list;
mod logs;
mod remove;
mod restart;
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
        /// Memory limit (e.g., 512M, 1G)
        #[arg(long)]
        memory_max: Option<String>,
        /// CPU quota (e.g., 50%, 200%)
        #[arg(long)]
        cpu_quota: Option<String>,
        /// I/O weight: 1-10000 (default: 100, lower = less I/O)
        #[arg(long)]
        io_weight: Option<String>,
        /// Timeout for stopping the process (e.g., 30s, 5m)
        #[arg(long)]
        timeout_stop: Option<String>,
        /// Command to run before ExecStart
        #[arg(long)]
        exec_start_pre: Option<String>,
        /// Command to run after process stops
        #[arg(long)]
        exec_stop_post: Option<String>,
        /// Max log level to store (emerg/alert/crit/err/warning/notice/info/debug)
        #[arg(long)]
        log_level_max: Option<String>,
        /// Randomized delay for timer trigger (e.g., 5m, 30s). Timer only
        #[arg(long)]
        random_delay: Option<String>,
        /// Environment variables (e.g., --env "PATH=/usr/bin" --env "FOO=bar"). Repeatable
        #[arg(long)]
        env: Vec<String>,
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
            memory_max,
            cpu_quota,
            io_weight,
            timeout_stop,
            exec_start_pre,
            exec_stop_post,
            log_level_max,
            random_delay,
            env,
        } => add::run(add::AddOptions {
            schedule,
            command,
            name,
            workdir,
            description,
            env_file,
            restart,
            memory_max,
            cpu_quota,
            io_weight,
            timeout_stop,
            exec_start_pre,
            exec_stop_post,
            log_level_max,
            random_delay,
            env,
        })?,
        Commands::List => list::run()?,
        Commands::Remove { name } => remove::run(&name)?,
        Commands::Edit { name } => edit::run(&name)?,
        Commands::Logs { name, follow, lines, priority } => logs::run(&name, follow, lines, priority)?,
        Commands::Restart { name } => restart::run(&name)?,
        Commands::Status { name } => status::run(&name)?,
        Commands::Enable { name } => enable::run(&name)?,
        Commands::Disable { name } => disable::run(&name)?,
    }

    Ok(())
}
