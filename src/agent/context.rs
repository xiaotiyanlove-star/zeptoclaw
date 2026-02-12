//! Context builder for agent conversations
//!
//! This module provides the `ContextBuilder` for constructing the system prompt
//! and message history for LLM conversations.

use crate::session::Message;

/// Default system prompt for ZeptoClaw agent
const DEFAULT_SYSTEM_PROMPT: &str = r#"You are ZeptoClaw, an ultra-lightweight personal AI assistant.

You have access to tools to help accomplish tasks. Use them when needed.

Be concise but helpful. Focus on completing the user's request efficiently."#;

/// Builder for constructing conversation context for LLM calls.
///
/// The `ContextBuilder` helps construct the full message list including
/// system prompts, skills information, conversation history, and user input.
///
/// # Example
///
/// ```rust
/// use zeptoclaw::agent::ContextBuilder;
/// use zeptoclaw::session::Message;
///
/// let builder = ContextBuilder::new()
///     .with_skills("- /help: Show help information");
///
/// let messages = builder.build_messages(vec![], "Hello!");
/// assert_eq!(messages.len(), 2); // system + user message
/// ```
pub struct ContextBuilder {
    /// The system prompt to use
    system_prompt: String,
    /// Optional skills content to append to system prompt
    skills_prompt: Option<String>,
}

impl ContextBuilder {
    /// Create a new context builder with the default system prompt.
    ///
    /// # Example
    /// ```rust
    /// use zeptoclaw::agent::ContextBuilder;
    ///
    /// let builder = ContextBuilder::new();
    /// let system = builder.build_system_message();
    /// assert!(system.content.contains("ZeptoClaw"));
    /// ```
    pub fn new() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            skills_prompt: None,
        }
    }

    /// Set a custom system prompt.
    ///
    /// # Arguments
    /// * `prompt` - The custom system prompt to use
    ///
    /// # Example
    /// ```rust
    /// use zeptoclaw::agent::ContextBuilder;
    ///
    /// let builder = ContextBuilder::new()
    ///     .with_system_prompt("You are a helpful assistant.");
    /// let system = builder.build_system_message();
    /// assert!(system.content.contains("helpful assistant"));
    /// ```
    pub fn with_system_prompt(mut self, prompt: &str) -> Self {
        self.system_prompt = prompt.to_string();
        self
    }

    /// Add skills information to the system prompt.
    ///
    /// Skills content is appended to the system prompt under an
    /// "Available Skills" section.
    ///
    /// # Arguments
    /// * `skills_content` - The skills documentation to include
    ///
    /// # Example
    /// ```rust
    /// use zeptoclaw::agent::ContextBuilder;
    ///
    /// let builder = ContextBuilder::new()
    ///     .with_skills("- /search: Search the web\n- /help: Show help");
    /// let system = builder.build_system_message();
    /// assert!(system.content.contains("Available Skills"));
    /// assert!(system.content.contains("/search"));
    /// ```
    pub fn with_skills(mut self, skills_content: &str) -> Self {
        self.skills_prompt = Some(skills_content.to_string());
        self
    }

    /// Build the system message with all configured content.
    ///
    /// # Returns
    /// A `Message` with role `System` containing the full system prompt.
    ///
    /// # Example
    /// ```rust
    /// use zeptoclaw::agent::ContextBuilder;
    /// use zeptoclaw::session::Role;
    ///
    /// let builder = ContextBuilder::new();
    /// let system = builder.build_system_message();
    /// assert_eq!(system.role, Role::System);
    /// ```
    pub fn build_system_message(&self) -> Message {
        let mut content = self.system_prompt.clone();
        if let Some(ref skills) = self.skills_prompt {
            content.push_str("\n\n## Available Skills\n\n");
            content.push_str(skills);
        }
        Message::system(&content)
    }

    /// Build the full message list for an LLM call.
    ///
    /// This constructs a message list with:
    /// 1. System message (with skills if configured)
    /// 2. Conversation history
    /// 3. New user input (if non-empty)
    ///
    /// # Arguments
    /// * `history` - The conversation history to include
    /// * `user_input` - The new user message (empty string is ignored)
    ///
    /// # Returns
    /// A vector of messages ready for the LLM.
    ///
    /// # Example
    /// ```rust
    /// use zeptoclaw::agent::ContextBuilder;
    /// use zeptoclaw::session::Message;
    ///
    /// let builder = ContextBuilder::new();
    /// let history = vec![
    ///     Message::user("Hello"),
    ///     Message::assistant("Hi there!"),
    /// ];
    /// let messages = builder.build_messages(history, "How are you?");
    /// assert_eq!(messages.len(), 4); // system + 2 history + new user
    /// ```
    pub fn build_messages(&self, history: Vec<Message>, user_input: &str) -> Vec<Message> {
        let mut messages = vec![self.build_system_message()];
        messages.extend(history);
        if !user_input.is_empty() {
            messages.push(Message::user(user_input));
        }
        messages
    }

    /// Get the current system prompt.
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Check if skills are configured.
    pub fn has_skills(&self) -> bool {
        self.skills_prompt.is_some()
    }
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Role;

    #[test]
    fn test_context_builder_new() {
        let builder = ContextBuilder::new();
        assert!(builder.system_prompt().contains("ZeptoClaw"));
        assert!(!builder.has_skills());
    }

    #[test]
    fn test_context_builder_default() {
        let builder = ContextBuilder::default();
        assert!(builder.system_prompt().contains("ZeptoClaw"));
    }

    #[test]
    fn test_context_builder_custom_system_prompt() {
        let builder = ContextBuilder::new().with_system_prompt("Custom prompt here");
        assert_eq!(builder.system_prompt(), "Custom prompt here");
    }

    #[test]
    fn test_context_builder_with_skills() {
        let builder = ContextBuilder::new().with_skills("- /test: Test skill");
        assert!(builder.has_skills());

        let system = builder.build_system_message();
        assert!(system.content.contains("Available Skills"));
        assert!(system.content.contains("/test"));
    }

    #[test]
    fn test_build_system_message() {
        let builder = ContextBuilder::new();
        let system = builder.build_system_message();

        assert_eq!(system.role, Role::System);
        assert!(system.content.contains("ZeptoClaw"));
    }

    #[test]
    fn test_build_messages_empty_input() {
        let builder = ContextBuilder::new();
        let messages = builder.build_messages(vec![], "");

        // Only system message when input is empty
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, Role::System);
    }

    #[test]
    fn test_build_messages_with_input() {
        let builder = ContextBuilder::new();
        let messages = builder.build_messages(vec![], "Hello");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::System);
        assert_eq!(messages[1].role, Role::User);
        assert_eq!(messages[1].content, "Hello");
    }

    #[test]
    fn test_build_messages_with_history() {
        let builder = ContextBuilder::new();
        let history = vec![
            Message::user("Previous message"),
            Message::assistant("Previous response"),
        ];
        let messages = builder.build_messages(history, "New message");

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, Role::System);
        assert_eq!(messages[1].role, Role::User);
        assert_eq!(messages[1].content, "Previous message");
        assert_eq!(messages[2].role, Role::Assistant);
        assert_eq!(messages[3].role, Role::User);
        assert_eq!(messages[3].content, "New message");
    }

    #[test]
    fn test_build_messages_preserves_history_order() {
        let builder = ContextBuilder::new();
        let history = vec![
            Message::user("First"),
            Message::assistant("Second"),
            Message::user("Third"),
            Message::assistant("Fourth"),
        ];
        let messages = builder.build_messages(history, "");

        // System + 4 history messages (no new input since it's empty)
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[1].content, "First");
        assert_eq!(messages[2].content, "Second");
        assert_eq!(messages[3].content, "Third");
        assert_eq!(messages[4].content, "Fourth");
    }

    #[test]
    fn test_context_builder_chaining() {
        let builder = ContextBuilder::new()
            .with_system_prompt("Custom prompt")
            .with_skills("- /skill1: Do something");

        let system = builder.build_system_message();
        assert!(system.content.contains("Custom prompt"));
        assert!(system.content.contains("/skill1"));
    }

    #[test]
    fn test_build_messages_with_tool_calls_in_history() {
        use crate::session::ToolCall;

        let builder = ContextBuilder::new();
        let history = vec![
            Message::user("Search for rust"),
            Message::assistant_with_tools(
                "Let me search for that.",
                vec![ToolCall::new("call_1", "search", r#"{"q": "rust"}"#)],
            ),
            Message::tool_result("call_1", "Found 100 results"),
            Message::assistant("I found 100 results about Rust."),
        ];
        let messages = builder.build_messages(history, "Thanks!");

        // System + 4 history + new user message
        assert_eq!(messages.len(), 6);
        assert!(messages[2].has_tool_calls());
        assert!(messages[3].is_tool_result());
    }
}
