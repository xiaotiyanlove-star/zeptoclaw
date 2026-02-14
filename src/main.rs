//! ZeptoClaw CLI — Ultra-lightweight personal AI assistant
//!
//! All CLI logic lives in the `cli` module. This file is just the entry point.

mod cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}
