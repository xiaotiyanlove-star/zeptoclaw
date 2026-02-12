//! PicoClaw - Ultra-lightweight personal AI assistant framework

pub mod agent;
pub mod bus;
pub mod channels;
pub mod config;
pub mod error;
pub mod providers;
pub mod session;
pub mod skills;
pub mod tools;
pub mod utils;

pub use bus::{InboundMessage, MediaAttachment, MediaType, MessageBus, OutboundMessage};
pub use config::Config;
pub use error::{PicoError, Result};
pub use providers::{
    ChatOptions, ClaudeProvider, LLMProvider, LLMResponse, LLMToolCall, ToolDefinition, Usage,
};
pub use session::{Message, Role, Session, SessionManager, ToolCall};
