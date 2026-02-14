//! CLI module — command parsing and dispatch
//!
//! All CLI logic lives here. `main.rs` calls `cli::run()`.

pub mod agent;
pub mod common;
pub mod config;
pub mod gateway;
pub mod heartbeat;
pub mod onboard;
pub mod skills;
pub mod status;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "zeptoclaw")]
#[command(about = "Ultra-lightweight personal AI assistant", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize configuration and workspace
    Onboard,
    /// Start interactive agent mode
    Agent {
        /// Direct message to process (non-interactive mode)
        #[arg(short, long)]
        message: Option<String>,
        /// Stream the response token-by-token
        #[arg(long)]
        stream: bool,
    },
    /// Start multi-channel gateway
    Gateway {
        /// Run in container isolation [optional: docker, apple]
        #[arg(long, num_args = 0..=1, default_missing_value = "auto", value_name = "BACKEND")]
        containerized: Option<String>,
    },
    /// Run agent in stdin/stdout mode (for containerized execution)
    AgentStdin,
    /// Trigger or inspect heartbeat tasks
    Heartbeat {
        /// Show heartbeat file contents
        #[arg(short, long)]
        show: bool,
        /// Edit heartbeat file in $EDITOR
        #[arg(short, long)]
        edit: bool,
    },
    /// Manage skills
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
    },
    /// Manage authentication
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Show version information
    Version,
    /// Show system status
    Status,
    /// Validate configuration file
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum SkillsAction {
    /// List skills (ready-only by default)
    List {
        /// Include unavailable skills
        #[arg(short, long)]
        all: bool,
    },
    /// Show full skill content
    Show {
        /// Skill name
        name: String,
    },
    /// Create a new workspace skill template
    Create {
        /// Skill name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum AuthAction {
    /// Log in to AI provider
    Login,
    /// Log out from AI provider
    Logout,
    /// Show authentication status
    Status,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Check configuration for errors and warnings
    Check,
}

/// Entry point for the CLI — called from main().
pub async fn run() -> Result<()> {
    // Initialize logging (JSON format when RUST_LOG_FORMAT=json)
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let use_json = std::env::var("RUST_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if use_json {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(false)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Version) | None => {
            cmd_version();
        }
        Some(Commands::Onboard) => {
            onboard::cmd_onboard().await?;
        }
        Some(Commands::Agent { message, stream }) => {
            agent::cmd_agent(message, stream).await?;
        }
        Some(Commands::Gateway { containerized }) => {
            gateway::cmd_gateway(containerized).await?;
        }
        Some(Commands::AgentStdin) => {
            agent::cmd_agent_stdin().await?;
        }
        Some(Commands::Heartbeat { show, edit }) => {
            heartbeat::cmd_heartbeat(show, edit).await?;
        }
        Some(Commands::Skills { action }) => {
            skills::cmd_skills(action).await?;
        }
        Some(Commands::Auth { action }) => {
            status::cmd_auth(action).await?;
        }
        Some(Commands::Status) => {
            status::cmd_status().await?;
        }
        Some(Commands::Config { action }) => {
            config::cmd_config(action).await?;
        }
    }

    Ok(())
}

/// Display version information
fn cmd_version() {
    println!("zeptoclaw {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Ultra-lightweight personal AI assistant framework");
    println!("https://github.com/qhkm/zeptoclaw");
}
