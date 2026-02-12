//! Providers module - LLM providers (OpenAI, Anthropic, etc.)
//!
//! This module defines the `LLMProvider` trait and common types for
//! interacting with various LLM providers. Each provider (OpenAI, Claude, etc.)
//! implements the `LLMProvider` trait to provide a consistent interface.
//!
//! # Example
//!
//! ```rust,ignore
//! use picoclaw::providers::{LLMProvider, ChatOptions, ToolDefinition};
//! use picoclaw::providers::claude::ClaudeProvider;
//! use picoclaw::session::Message;
//!
//! async fn example() {
//!     let provider = ClaudeProvider::new("your-api-key");
//!     let messages = vec![Message::user("Hello!")];
//!     let options = ChatOptions::new().with_max_tokens(1000);
//!
//!     let response = provider.chat(messages, vec![], None, options).await.unwrap();
//!     println!("Response: {}", response.content);
//! }
//! ```

pub mod claude;
mod types;

pub use claude::ClaudeProvider;
pub use types::{
    ChatOptions, LLMProvider, LLMResponse, LLMToolCall, ToolDefinition, Usage,
};
