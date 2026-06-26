use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser)]
#[command(name = "nimpipe")]
#[command(about = "NVIDIA NIM OpenAI-compatible proxy")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the proxy server in the foreground.
    Start {
        /// Run in foreground (do not daemonize).
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the running proxy server.
    Stop,
    /// Show proxy status.
    Status,
    /// Show recent logs.
    Logs,
    /// Reload configuration without restarting.
    Reload,
    /// Install autostart service (coming soon).
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
pub enum ServiceAction {
    Install,
    Uninstall,
}

pub async fn run_command(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Start { foreground } => {
            if !foreground {
                info!(
                    "Starting nimpipe in foreground. Use --foreground explicitly, or use service install for background."
                );
            }
            crate::server_main().await
        }
        Commands::Stop => {
            println!("stop: not implemented in v1. Send SIGTERM to the running process.");
            Ok(())
        }
        Commands::Status => {
            println!("status: not implemented in v1.");
            Ok(())
        }
        Commands::Logs => {
            println!("logs: not implemented in v1. Tail the log file manually.");
            Ok(())
        }
        Commands::Reload => {
            println!(
                "reload: not implemented in CLI v1. Use the web UI reload button or restart the server."
            );
            Ok(())
        }
        Commands::Service { action } => match action {
            ServiceAction::Install => {
                println!("service install: coming soon.");
                Ok(())
            }
            ServiceAction::Uninstall => {
                println!("service uninstall: coming soon.");
                Ok(())
            }
        },
    }
}
