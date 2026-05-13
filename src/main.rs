mod adapter;
mod config;
mod daemon;
mod detect;
mod layout;
mod matcher;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "madori", version, about = "Intelligent display layout manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config file (default: /etc/madori/config.json)
    #[arg(short, long, global = true)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as udev-watching daemon
    Daemon,
    /// Detect current state and apply layout once
    Apply(ApplyArgs),
    /// Output current connection state + match result as JSON
    Dump,
    /// Raw EDID info for all connected outputs (debug)
    Detect,
    /// Human-readable display status
    Status,
}

#[derive(Args)]
struct ApplyArgs {
    /// Show what would be applied without actually changing anything
    #[arg(long)]
    dry_run: bool,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Daemon => daemon::run_daemon(),
        Commands::Apply(args) => {
            if args.dry_run {
                daemon::apply_dry_run(cli.config.as_deref())
            } else {
                daemon::apply_once(cli.config.as_deref())
            }
        }
        Commands::Dump => daemon::dump_state(cli.config.as_deref()),
        Commands::Detect => cmd_detect(),
        Commands::Status => daemon::show_status(cli.config.as_deref()),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn cmd_detect() -> Result<(), Box<dyn std::error::Error>> {
    let connectors = detect::detect_connectors()?;
    let json = serde_json::to_string_pretty(&connectors)?;
    println!("{}", json);
    Ok(())
}
