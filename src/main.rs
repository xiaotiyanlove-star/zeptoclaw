//! PicoClaw CLI - Ultra-lightweight personal AI assistant
//!
//! This is the main entry point for the PicoClaw command-line interface.
//! It provides commands for running the AI agent in interactive mode,
//! starting the multi-channel gateway, and managing configuration.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use picoclaw::agent::AgentLoop;
use picoclaw::bus::{InboundMessage, MessageBus};
use picoclaw::channels::{ChannelManager, TelegramChannel};
use picoclaw::config::Config;
use picoclaw::providers::ClaudeProvider;
use picoclaw::session::SessionManager;
use picoclaw::tools::filesystem::{EditFileTool, ListDirTool, ReadFileTool, WriteFileTool};
use picoclaw::tools::shell::ShellTool;
use picoclaw::tools::EchoTool;

#[derive(Parser)]
#[command(name = "picoclaw")]
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
    },
    /// Start multi-channel gateway
    Gateway,
    /// Manage authentication
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Show version information
    Version,
    /// Show system status
    Status,
}

#[derive(Subcommand)]
enum AuthAction {
    /// Log in to AI provider
    Login,
    /// Log out from AI provider
    Logout,
    /// Show authentication status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Version) | None => {
            cmd_version();
        }
        Some(Commands::Onboard) => {
            cmd_onboard().await?;
        }
        Some(Commands::Agent { message }) => {
            cmd_agent(message).await?;
        }
        Some(Commands::Gateway) => {
            cmd_gateway().await?;
        }
        Some(Commands::Auth { action }) => {
            cmd_auth(action).await?;
        }
        Some(Commands::Status) => {
            cmd_status().await?;
        }
    }

    Ok(())
}

/// Display version information
fn cmd_version() {
    println!("picoclaw {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Ultra-lightweight personal AI assistant framework");
    println!("https://github.com/picoclaw/picoclaw");
}

/// Initialize configuration directory and save default config
async fn cmd_onboard() -> Result<()> {
    println!("Initializing PicoClaw...");

    // Create config directory
    let config_dir = Config::dir();
    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;
    println!("  Created config directory: {:?}", config_dir);

    // Create workspace directory
    let workspace_dir = config_dir.join("workspace");
    std::fs::create_dir_all(&workspace_dir)
        .with_context(|| format!("Failed to create workspace directory: {:?}", workspace_dir))?;
    println!("  Created workspace directory: {:?}", workspace_dir);

    // Create sessions directory
    let sessions_dir = config_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir)
        .with_context(|| format!("Failed to create sessions directory: {:?}", sessions_dir))?;
    println!("  Created sessions directory: {:?}", sessions_dir);

    // Save default config if it doesn't exist
    let config_path = Config::path();
    if !config_path.exists() {
        let config = Config::default();
        config
            .save()
            .with_context(|| "Failed to save default configuration")?;
        println!("  Created default config: {:?}", config_path);
    } else {
        println!("  Config already exists: {:?}", config_path);
    }

    println!();
    println!("PicoClaw initialized successfully!");
    println!();
    println!("Next steps:");
    println!("  1. Edit {:?} to add your API keys", config_path);
    println!("  2. Run 'picoclaw agent' to start the interactive agent");
    println!("  3. Run 'picoclaw gateway' to start the multi-channel gateway");

    Ok(())
}

/// Create and configure an agent with all tools registered
async fn create_agent(config: Config, bus: Arc<MessageBus>) -> Result<Arc<AgentLoop>> {
    // Create session manager
    let session_manager = SessionManager::new().unwrap_or_else(|_| {
        warn!("Failed to create persistent session manager, using in-memory");
        SessionManager::new_memory()
    });

    // Create agent loop
    let agent = Arc::new(AgentLoop::new(config.clone(), session_manager, bus));

    // Register all tools
    agent.register_tool(Box::new(EchoTool)).await;
    agent.register_tool(Box::new(ReadFileTool)).await;
    agent.register_tool(Box::new(WriteFileTool)).await;
    agent.register_tool(Box::new(ListDirTool)).await;
    agent.register_tool(Box::new(EditFileTool)).await;
    agent.register_tool(Box::new(ShellTool)).await;

    info!(
        "Registered {} tools",
        agent.tool_count().await
    );

    // Set up provider if API key is configured
    if let Some(ref anthropic) = config.providers.anthropic {
        if let Some(ref api_key) = anthropic.api_key {
            if !api_key.is_empty() {
                let provider = ClaudeProvider::new(api_key);
                agent.set_provider(Box::new(provider)).await;
                info!("Configured Claude provider");
            }
        }
    }

    Ok(agent)
}

/// Interactive or single-message agent mode
async fn cmd_agent(message: Option<String>) -> Result<()> {
    // Load configuration
    let config = Config::load().with_context(|| "Failed to load configuration")?;

    // Create message bus
    let bus = Arc::new(MessageBus::new());

    // Create agent
    let agent = create_agent(config.clone(), bus.clone()).await?;

    // Check if provider is configured
    let has_provider = config.providers.anthropic
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .map(|k| !k.is_empty())
        .unwrap_or(false);

    if !has_provider {
        eprintln!("Warning: No AI provider configured. Set PICOCLAW_PROVIDERS_ANTHROPIC_API_KEY");
        eprintln!("or add your API key to {:?}", Config::path());
        eprintln!();
    }

    if let Some(msg) = message {
        // Single message mode
        let inbound = InboundMessage::new("cli", "user", "cli", &msg);
        match agent.process_message(&inbound).await {
            Ok(response) => {
                println!("{}", response);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Interactive mode
        println!("PicoClaw Interactive Agent");
        println!("Type your message and press Enter. Type 'quit' or 'exit' to stop.");
        println!();

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            print!("> ");
            stdout.flush()?;

            let mut input = String::new();
            match stdin.lock().read_line(&mut input) {
                Ok(0) => {
                    // EOF
                    println!();
                    break;
                }
                Ok(_) => {
                    let input = input.trim();
                    if input.is_empty() {
                        continue;
                    }
                    if input == "quit" || input == "exit" {
                        println!("Goodbye!");
                        break;
                    }

                    // Process message
                    let inbound = InboundMessage::new("cli", "user", "cli", input);
                    match agent.process_message(&inbound).await {
                        Ok(response) => {
                            println!();
                            println!("{}", response);
                            println!();
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            eprintln!();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Start multi-channel gateway
async fn cmd_gateway() -> Result<()> {
    println!("Starting PicoClaw Gateway...");

    // Load configuration
    let config = Config::load().with_context(|| "Failed to load configuration")?;

    // Create message bus
    let bus = Arc::new(MessageBus::new());

    // Create agent
    let agent = create_agent(config.clone(), bus.clone()).await?;

    // Check if provider is configured
    let has_provider = config.providers.anthropic
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .map(|k| !k.is_empty())
        .unwrap_or(false);

    if !has_provider {
        error!("No AI provider configured. Set PICOCLAW_PROVIDERS_ANTHROPIC_API_KEY");
        error!("or add your API key to {:?}", Config::path());
        std::process::exit(1);
    }

    // Create channel manager
    let channel_manager = ChannelManager::new(bus.clone(), config.clone());

    // Register Telegram channel if enabled
    if let Some(ref telegram_config) = config.channels.telegram {
        if telegram_config.enabled {
            if telegram_config.token.is_empty() {
                warn!("Telegram channel enabled but token is empty");
            } else {
                let telegram = TelegramChannel::new(telegram_config.clone(), bus.clone());
                channel_manager.register(Box::new(telegram)).await;
                info!("Registered Telegram channel");
            }
        }
    }

    // Check if any channels are registered
    let channel_count = channel_manager.channel_count().await;
    if channel_count == 0 {
        warn!("No channels configured. Enable channels in {:?}", Config::path());
        warn!("The agent loop will still run but won't receive messages from external sources.");
    } else {
        info!("Registered {} channel(s)", channel_count);
    }

    // Start all channels
    channel_manager
        .start_all()
        .await
        .with_context(|| "Failed to start channels")?;

    // Start agent loop in background
    let agent_clone = Arc::clone(&agent);
    let agent_handle = tokio::spawn(async move {
        if let Err(e) = agent_clone.start().await {
            error!("Agent loop error: {}", e);
        }
    });

    println!();
    println!("Gateway is running. Press Ctrl+C to stop.");
    println!();

    // Wait for Ctrl+C
    tokio::signal::ctrl_c()
        .await
        .with_context(|| "Failed to listen for Ctrl+C")?;

    println!();
    println!("Shutting down...");

    // Stop agent
    agent.stop();

    // Stop all channels
    channel_manager
        .stop_all()
        .await
        .with_context(|| "Failed to stop channels")?;

    // Wait for agent to stop
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), agent_handle).await;

    println!("Gateway stopped.");
    Ok(())
}

/// Manage authentication
async fn cmd_auth(action: AuthAction) -> Result<()> {
    match action {
        AuthAction::Login => {
            println!("Authentication login is not yet implemented.");
            println!();
            println!("To configure API keys, either:");
            println!("  1. Set environment variables:");
            println!("     export PICOCLAW_PROVIDERS_ANTHROPIC_API_KEY=sk-ant-...");
            println!();
            println!("  2. Edit your config file:");
            println!("     {:?}", Config::path());
        }
        AuthAction::Logout => {
            println!("Authentication logout is not yet implemented.");
        }
        AuthAction::Status => {
            cmd_auth_status().await?;
        }
    }
    Ok(())
}

/// Show authentication status
async fn cmd_auth_status() -> Result<()> {
    let config = Config::load().with_context(|| "Failed to load configuration")?;

    println!("Authentication Status");
    println!("=====================");
    println!();

    // Anthropic/Claude
    let anthropic_status = config
        .providers
        .anthropic
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .map(|k| if k.is_empty() { "not set" } else { "configured" })
        .unwrap_or("not set");
    println!("  Anthropic (Claude): {}", anthropic_status);

    // OpenAI
    let openai_status = config
        .providers
        .openai
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .map(|k| if k.is_empty() { "not set" } else { "configured" })
        .unwrap_or("not set");
    println!("  OpenAI:             {}", openai_status);

    // OpenRouter
    let openrouter_status = config
        .providers
        .openrouter
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .map(|k| if k.is_empty() { "not set" } else { "configured" })
        .unwrap_or("not set");
    println!("  OpenRouter:         {}", openrouter_status);

    // Groq
    let groq_status = config
        .providers
        .groq
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .map(|k| if k.is_empty() { "not set" } else { "configured" })
        .unwrap_or("not set");
    println!("  Groq:               {}", groq_status);

    // Gemini
    let gemini_status = config
        .providers
        .gemini
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .map(|k| if k.is_empty() { "not set" } else { "configured" })
        .unwrap_or("not set");
    println!("  Gemini:             {}", gemini_status);

    // Zhipu
    let zhipu_status = config
        .providers
        .zhipu
        .as_ref()
        .and_then(|p| p.api_key.as_ref())
        .map(|k| if k.is_empty() { "not set" } else { "configured" })
        .unwrap_or("not set");
    println!("  Zhipu:              {}", zhipu_status);

    println!();

    // Channel tokens
    println!("Channel Tokens");
    println!("--------------");

    // Telegram
    let telegram_status = config
        .channels
        .telegram
        .as_ref()
        .map(|c| {
            if c.token.is_empty() {
                "not set"
            } else if c.enabled {
                "configured (enabled)"
            } else {
                "configured (disabled)"
            }
        })
        .unwrap_or("not set");
    println!("  Telegram: {}", telegram_status);

    // Discord
    let discord_status = config
        .channels
        .discord
        .as_ref()
        .map(|c| {
            if c.token.is_empty() {
                "not set"
            } else if c.enabled {
                "configured (enabled)"
            } else {
                "configured (disabled)"
            }
        })
        .unwrap_or("not set");
    println!("  Discord:  {}", discord_status);

    // Slack
    let slack_status = config
        .channels
        .slack
        .as_ref()
        .map(|c| {
            if c.bot_token.is_empty() {
                "not set"
            } else if c.enabled {
                "configured (enabled)"
            } else {
                "configured (disabled)"
            }
        })
        .unwrap_or("not set");
    println!("  Slack:    {}", slack_status);

    println!();

    Ok(())
}

/// Show system status
async fn cmd_status() -> Result<()> {
    let config = Config::load().unwrap_or_default();

    println!("PicoClaw Status");
    println!("===============");
    println!();

    // Version
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!();

    // Configuration
    println!("Configuration");
    println!("-------------");
    println!("  Config directory: {:?}", Config::dir());
    println!("  Config file:      {:?}", Config::path());
    println!("  Config exists:    {}", Config::path().exists());
    println!();

    // Workspace
    println!("Workspace");
    println!("---------");
    let workspace_path = config.workspace_path();
    println!("  Path:   {:?}", workspace_path);
    println!("  Exists: {}", workspace_path.exists());
    println!();

    // Sessions
    println!("Sessions");
    println!("--------");
    let sessions_path = Config::dir().join("sessions");
    println!("  Path:   {:?}", sessions_path);
    println!("  Exists: {}", sessions_path.exists());
    if sessions_path.exists() {
        let session_count = std::fs::read_dir(&sessions_path)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        println!("  Count:  {}", session_count);
    }
    println!();

    // Agent defaults
    println!("Agent Defaults");
    println!("--------------");
    println!("  Model:              {}", config.agents.defaults.model);
    println!("  Max tokens:         {}", config.agents.defaults.max_tokens);
    println!("  Temperature:        {}", config.agents.defaults.temperature);
    println!("  Max tool iterations: {}", config.agents.defaults.max_tool_iterations);
    println!();

    // Gateway
    println!("Gateway");
    println!("-------");
    println!("  Host: {}", config.gateway.host);
    println!("  Port: {}", config.gateway.port);
    println!();

    // Provider status
    let has_provider = config.get_api_key().is_some();
    println!("Provider: {}", if has_provider { "configured" } else { "not configured" });
    println!();

    // Registered tools (static list)
    println!("Available Tools");
    println!("---------------");
    println!("  - echo");
    println!("  - read_file");
    println!("  - write_file");
    println!("  - list_dir");
    println!("  - edit_file");
    println!("  - shell");
    println!();

    Ok(())
}
