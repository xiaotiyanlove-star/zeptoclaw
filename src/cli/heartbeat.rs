//! Heartbeat command handler.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use zeptoclaw::bus::{InboundMessage, MessageBus};
use zeptoclaw::config::Config;
use zeptoclaw::heartbeat::{ensure_heartbeat_file, HeartbeatService, HEARTBEAT_PROMPT};

use super::common::{create_agent, expand_tilde};

/// Resolve the heartbeat file path from config.
pub(crate) fn heartbeat_file_path(config: &Config) -> PathBuf {
    config
        .heartbeat
        .file_path
        .as_deref()
        .map(expand_tilde)
        .unwrap_or_else(|| Config::dir().join("HEARTBEAT.md"))
}

/// Heartbeat utility command.
pub(crate) async fn cmd_heartbeat(show: bool, edit: bool) -> Result<()> {
    let config = Config::load().with_context(|| "Failed to load configuration")?;
    let hb_path = heartbeat_file_path(&config);

    if ensure_heartbeat_file(&hb_path).await? {
        println!("Created heartbeat file at {:?}", hb_path);
    }

    if show {
        let content = tokio::fs::read_to_string(&hb_path).await?;
        println!("{}", content);
        return Ok(());
    }

    if edit {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
        let status = std::process::Command::new(editor)
            .arg(&hb_path)
            .status()
            .with_context(|| "Failed to launch editor")?;
        if !status.success() {
            eprintln!("Editor exited with status: {}", status);
        }
        return Ok(());
    }

    let content = tokio::fs::read_to_string(&hb_path)
        .await
        .unwrap_or_default();
    if HeartbeatService::is_empty(&content) {
        println!("Heartbeat file has no actionable tasks.");
        return Ok(());
    }

    let bus = Arc::new(MessageBus::new());
    let agent = create_agent(config, bus).await?;
    let inbound = InboundMessage::new("cli", "heartbeat", "heartbeat:cli", HEARTBEAT_PROMPT);
    let response = agent.process_message(&inbound).await?;
    println!("{}", response);
    Ok(())
}
