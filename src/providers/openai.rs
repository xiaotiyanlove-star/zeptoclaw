//! OpenAI Provider Implementation
//!
//! This module implements the `LLMProvider` trait for OpenAI's Chat Completions API,
//! handling message conversion, tool calls, and response parsing.
//!
//! # Example
//!
//! ```rust,ignore
//! use zeptoclaw::providers::{openai::OpenAIProvider, ChatOptions, LLMProvider};
//! use zeptoclaw::session::Message;
//!
//! async fn example() {
//!     let provider = OpenAIProvider::new("your-api-key");
//!
//!     let messages = vec![
//!         Message::system("You are a helpful assistant."),
//!         Message::user("Hello!"),
//!     ];
//!
//!     let response = provider
//!         .chat(messages, vec![], None, ChatOptions::default())
//!         .await
//!         .unwrap();
//!
//!     println!("OpenAI: {}", response.content);
//! }
//! ```

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{PicoError, Result};
use crate::session::{Message, Role};

use super::{ChatOptions, LLMProvider, LLMResponse, LLMToolCall, ToolDefinition, Usage};

/// The OpenAI API endpoint URL.
const OPENAI_API_URL: &str = "https://api.openai.com/v1";

/// The default OpenAI model to use.
const DEFAULT_MODEL: &str = "gpt-4o";

// ============================================================================
// OpenAI API Request Types
// ============================================================================

/// OpenAI API request body.
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    /// Model identifier
    model: String,
    /// Conversation messages (including system)
    messages: Vec<OpenAIMessage>,
    /// Available tools
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    /// Temperature for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    /// Top-p (nucleus) sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

/// A message in OpenAI's format.
#[derive(Debug, Serialize)]
struct OpenAIMessage {
    /// Role: "system", "user", "assistant", or "tool"
    role: String,
    /// Message content (can be null for assistant with tool_calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    /// Tool calls made by the assistant
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCallRequest>>,
    /// ID of the tool call this message is responding to
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

/// A tool call in a request (assistant requesting tool execution).
#[derive(Debug, Serialize)]
struct OpenAIToolCallRequest {
    /// Unique identifier for this tool call
    id: String,
    /// Type of tool call (always "function")
    r#type: String,
    /// Function details
    function: OpenAIFunctionCall,
}

/// Function call details.
#[derive(Debug, Serialize, Deserialize)]
struct OpenAIFunctionCall {
    /// Name of the function to call
    name: String,
    /// JSON-encoded arguments
    arguments: String,
}

/// OpenAI tool definition.
#[derive(Debug, Serialize)]
struct OpenAITool {
    /// Type of tool (always "function")
    r#type: String,
    /// Function definition
    function: OpenAIFunctionDef,
}

/// OpenAI function definition.
#[derive(Debug, Serialize)]
struct OpenAIFunctionDef {
    /// Function name
    name: String,
    /// Function description
    description: String,
    /// JSON Schema for function parameters
    parameters: serde_json::Value,
}

// ============================================================================
// OpenAI API Response Types
// ============================================================================

/// OpenAI API response body.
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    /// Response choices
    choices: Vec<OpenAIChoice>,
    /// Token usage
    usage: Option<OpenAIUsage>,
}

/// A choice in the response.
#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    /// The message content
    message: OpenAIResponseMessage,
}

/// A message in the response.
#[derive(Debug, Deserialize)]
struct OpenAIResponseMessage {
    /// Text content (may be null if tool_calls present)
    content: Option<String>,
    /// Tool calls made by the model
    tool_calls: Option<Vec<OpenAIToolCallResponse>>,
}

/// A tool call in the response.
#[derive(Debug, Deserialize)]
struct OpenAIToolCallResponse {
    /// Unique identifier for this tool call
    id: String,
    /// Function details
    function: OpenAIFunctionCall,
}

/// OpenAI token usage.
#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    /// Tokens in the prompt
    prompt_tokens: u32,
    /// Tokens in the completion
    completion_tokens: u32,
}

/// OpenAI API error response.
#[derive(Debug, Deserialize)]
struct OpenAIErrorResponse {
    error: OpenAIError,
}

/// OpenAI API error details.
#[derive(Debug, Deserialize)]
struct OpenAIError {
    message: String,
    r#type: String,
}

// ============================================================================
// OpenAI Provider
// ============================================================================

/// OpenAI LLM provider.
///
/// Implements the `LLMProvider` trait for OpenAI's Chat Completions API.
/// Handles message format conversion, tool calling, and response parsing.
pub struct OpenAIProvider {
    /// API key for authentication
    api_key: String,
    /// API base URL
    api_base: String,
    /// HTTP client for making requests
    client: Client,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider with the given API key.
    ///
    /// Uses the default OpenAI API endpoint.
    ///
    /// # Arguments
    /// * `api_key` - OpenAI API key
    ///
    /// # Example
    /// ```
    /// use zeptoclaw::providers::openai::OpenAIProvider;
    /// use zeptoclaw::providers::LLMProvider;
    ///
    /// let provider = OpenAIProvider::new("sk-xxx");
    /// assert_eq!(provider.name(), "openai");
    /// ```
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            api_base: OPENAI_API_URL.to_string(),
            client: Client::new(),
        }
    }

    /// Create a new OpenAI provider with a custom base URL.
    ///
    /// This is useful for OpenAI-compatible APIs (Azure, local models, etc.).
    ///
    /// # Arguments
    /// * `api_key` - API key
    /// * `api_base` - Base URL for the API (trailing slash will be removed)
    ///
    /// # Example
    /// ```
    /// use zeptoclaw::providers::openai::OpenAIProvider;
    ///
    /// let provider = OpenAIProvider::with_base_url("sk-xxx", "https://my-api.com/v1/");
    /// ```
    pub fn with_base_url(api_key: &str, api_base: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            api_base: api_base.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    /// Create a new OpenAI provider with a custom HTTP client.
    ///
    /// This is useful for testing or when you need custom client configuration
    /// (e.g., custom timeouts, proxies).
    ///
    /// # Arguments
    /// * `api_key` - API key
    /// * `api_base` - Base URL for the API
    /// * `client` - Custom reqwest client
    pub fn with_client(api_key: &str, api_base: &str, client: Client) -> Self {
        Self {
            api_key: api_key.to_string(),
            api_base: api_base.trim_end_matches('/').to_string(),
            client,
        }
    }
}

// ============================================================================
// Conversion Functions
// ============================================================================

/// Convert ZeptoClaw messages to OpenAI API format.
fn convert_messages(messages: Vec<Message>) -> Vec<OpenAIMessage> {
    messages
        .into_iter()
        .map(|msg| {
            let role = match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            }
            .to_string();

            let tool_calls = msg.tool_calls.map(|tcs| {
                tcs.into_iter()
                    .map(|tc| OpenAIToolCallRequest {
                        id: tc.id,
                        r#type: "function".to_string(),
                        function: OpenAIFunctionCall {
                            name: tc.name,
                            arguments: tc.arguments,
                        },
                    })
                    .collect()
            });

            OpenAIMessage {
                role,
                content: if msg.content.is_empty() && tool_calls.is_some() {
                    None
                } else {
                    Some(msg.content)
                },
                tool_calls,
                tool_call_id: msg.tool_call_id,
            }
        })
        .collect()
}

/// Convert ZeptoClaw tool definitions to OpenAI API format.
fn convert_tools(tools: Vec<ToolDefinition>) -> Vec<OpenAITool> {
    tools
        .into_iter()
        .map(|t| OpenAITool {
            r#type: "function".to_string(),
            function: OpenAIFunctionDef {
                name: t.name,
                description: t.description,
                parameters: t.parameters,
            },
        })
        .collect()
}

/// Convert OpenAI API response to ZeptoClaw LLMResponse.
fn convert_response(response: OpenAIResponse) -> LLMResponse {
    let choice = response.choices.into_iter().next();

    let (content, tool_calls) = match choice {
        Some(c) => {
            let content = c.message.content.unwrap_or_default();
            let tool_calls = c
                .message
                .tool_calls
                .map(|tcs| {
                    tcs.into_iter()
                        .map(|tc| {
                            LLMToolCall::new(&tc.id, &tc.function.name, &tc.function.arguments)
                        })
                        .collect()
                })
                .unwrap_or_default();
            (content, tool_calls)
        }
        None => (String::new(), Vec::new()),
    };

    let mut llm_response = if tool_calls.is_empty() {
        LLMResponse::text(&content)
    } else {
        LLMResponse::with_tools(&content, tool_calls)
    };

    if let Some(usage) = response.usage {
        llm_response =
            llm_response.with_usage(Usage::new(usage.prompt_tokens, usage.completion_tokens));
    }

    llm_response
}

// ============================================================================
// LLMProvider Implementation
// ============================================================================

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
        model: Option<&str>,
        options: ChatOptions,
    ) -> Result<LLMResponse> {
        let model = model.unwrap_or(DEFAULT_MODEL);
        let openai_messages = convert_messages(messages);
        let openai_tools = if tools.is_empty() {
            None
        } else {
            Some(convert_tools(tools))
        };

        let request = OpenAIRequest {
            model: model.to_string(),
            messages: openai_messages,
            tools: openai_tools,
            max_tokens: options.max_tokens,
            temperature: options.temperature,
            top_p: options.top_p,
            stop: options.stop,
        };

        debug!("OpenAI request to model {}", model);

        let response = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| PicoError::Provider(format!("OpenAI request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse as OpenAI error response
            if let Ok(error_response) = serde_json::from_str::<OpenAIErrorResponse>(&error_text) {
                return Err(PicoError::Provider(format!(
                    "OpenAI API error ({}): {} - {}",
                    status, error_response.error.r#type, error_response.error.message
                )));
            }

            return Err(PicoError::Provider(format!(
                "OpenAI API error ({}): {}",
                status, error_text
            )));
        }

        let openai_response: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| PicoError::Provider(format!("Failed to parse OpenAI response: {}", e)))?;

        info!("OpenAI response received");
        Ok(convert_response(openai_response))
    }

    fn default_model(&self) -> &str {
        DEFAULT_MODEL
    }

    fn name(&self) -> &str {
        "openai"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Message, ToolCall};

    #[test]
    fn test_openai_provider_creation() {
        let provider = OpenAIProvider::new("test-key");
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.default_model(), "gpt-4o");
        assert_eq!(provider.api_base, "https://api.openai.com/v1");
    }

    #[test]
    fn test_openai_provider_with_base_url() {
        let provider = OpenAIProvider::with_base_url("test-key", "https://custom.api/v1/");
        assert_eq!(provider.api_base, "https://custom.api/v1");
    }

    #[test]
    fn test_openai_provider_with_client() {
        let client = Client::new();
        let provider = OpenAIProvider::with_client("test-key", "https://api.openai.com/v1", client);
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_convert_messages_simple() {
        let messages = vec![
            Message::system("You are helpful"),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];
        let converted = convert_messages(messages);

        assert_eq!(converted.len(), 3);
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[0].content, Some("You are helpful".to_string()));
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[1].content, Some("Hello".to_string()));
        assert_eq!(converted[2].role, "assistant");
        assert_eq!(converted[2].content, Some("Hi there!".to_string()));
    }

    #[test]
    fn test_convert_messages_with_tool_calls() {
        let tool_call = ToolCall::new("call_1", "search", r#"{"q": "rust"}"#);
        let messages = vec![
            Message::assistant_with_tools("Let me search", vec![tool_call]),
            Message::tool_result("call_1", "Found results"),
        ];
        let converted = convert_messages(messages);

        assert_eq!(converted.len(), 2);

        // First message: assistant with tool calls
        assert_eq!(converted[0].role, "assistant");
        assert!(converted[0].tool_calls.is_some());
        let tool_calls = converted[0].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].r#type, "function");
        assert_eq!(tool_calls[0].function.name, "search");

        // Second message: tool result
        assert_eq!(converted[1].role, "tool");
        assert_eq!(converted[1].tool_call_id, Some("call_1".to_string()));
        assert_eq!(converted[1].content, Some("Found results".to_string()));
    }

    #[test]
    fn test_convert_messages_empty_content_with_tool_calls() {
        let tool_call = ToolCall::new("call_1", "search", r#"{"q": "test"}"#);
        let mut msg = Message::assistant_with_tools("", vec![tool_call]);
        msg.content = String::new(); // Ensure content is empty

        let messages = vec![msg];
        let converted = convert_messages(messages);

        // Content should be None when empty and tool_calls present
        assert!(converted[0].content.is_none());
        assert!(converted[0].tool_calls.is_some());
    }

    #[test]
    fn test_convert_tools() {
        let tools = vec![ToolDefinition::new(
            "search",
            "Search the web",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            }),
        )];
        let converted = convert_tools(tools);

        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].r#type, "function");
        assert_eq!(converted[0].function.name, "search");
        assert_eq!(converted[0].function.description, "Search the web");
    }

    #[test]
    fn test_convert_response_text_only() {
        let response = OpenAIResponse {
            choices: vec![OpenAIChoice {
                message: OpenAIResponseMessage {
                    content: Some("Hello!".to_string()),
                    tool_calls: None,
                },
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
            }),
        };
        let converted = convert_response(response);

        assert_eq!(converted.content, "Hello!");
        assert!(!converted.has_tool_calls());
        assert!(converted.usage.is_some());

        let usage = converted.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn test_convert_response_with_tool_calls() {
        let response = OpenAIResponse {
            choices: vec![OpenAIChoice {
                message: OpenAIResponseMessage {
                    content: Some("".to_string()),
                    tool_calls: Some(vec![OpenAIToolCallResponse {
                        id: "call_123".to_string(),
                        function: OpenAIFunctionCall {
                            name: "search".to_string(),
                            arguments: r#"{"q":"test"}"#.to_string(),
                        },
                    }]),
                },
            }],
            usage: None,
        };
        let converted = convert_response(response);

        assert!(converted.has_tool_calls());
        assert_eq!(converted.tool_calls.len(), 1);
        assert_eq!(converted.tool_calls[0].id, "call_123");
        assert_eq!(converted.tool_calls[0].name, "search");
        assert_eq!(converted.tool_calls[0].arguments, r#"{"q":"test"}"#);
    }

    #[test]
    fn test_convert_response_empty_choices() {
        let response = OpenAIResponse {
            choices: vec![],
            usage: None,
        };
        let converted = convert_response(response);

        assert_eq!(converted.content, "");
        assert!(!converted.has_tool_calls());
    }

    #[test]
    fn test_convert_response_null_content() {
        let response = OpenAIResponse {
            choices: vec![OpenAIChoice {
                message: OpenAIResponseMessage {
                    content: None,
                    tool_calls: Some(vec![OpenAIToolCallResponse {
                        id: "call_1".to_string(),
                        function: OpenAIFunctionCall {
                            name: "test".to_string(),
                            arguments: "{}".to_string(),
                        },
                    }]),
                },
            }],
            usage: None,
        };
        let converted = convert_response(response);

        // Content should be empty string when null
        assert_eq!(converted.content, "");
        assert!(converted.has_tool_calls());
    }

    #[test]
    fn test_openai_request_serialization() {
        let request = OpenAIRequest {
            model: "gpt-4o".to_string(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: Some("Hello".to_string()),
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            max_tokens: Some(1000),
            temperature: Some(0.7),
            top_p: None,
            stop: None,
        };

        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("gpt-4o"));
        assert!(json.contains("max_tokens"));
        assert!(json.contains("Hello"));
        assert!(json.contains("temperature"));
        // Optional fields that are None should not be present
        assert!(!json.contains("top_p"));
        assert!(!json.contains("stop"));
        assert!(!json.contains("tools"));
    }

    #[test]
    fn test_openai_request_with_tools() {
        let request = OpenAIRequest {
            model: "gpt-4o".to_string(),
            messages: vec![],
            tools: Some(vec![OpenAITool {
                r#type: "function".to_string(),
                function: OpenAIFunctionDef {
                    name: "search".to_string(),
                    description: "Search the web".to_string(),
                    parameters: serde_json::json!({"type": "object"}),
                },
            }]),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
        };

        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("tools"));
        assert!(json.contains(r#""type":"function""#));
        assert!(json.contains("search"));
    }

    #[test]
    fn test_openai_message_with_tool_call_id() {
        let msg = OpenAIMessage {
            role: "tool".to_string(),
            content: Some("Tool result".to_string()),
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains("tool_call_id"));
        assert!(json.contains("call_123"));
    }

    #[test]
    fn test_multiple_tool_calls_conversion() {
        let tc1 = ToolCall::new("call_1", "tool_a", r#"{"arg": "a"}"#);
        let tc2 = ToolCall::new("call_2", "tool_b", r#"{"arg": "b"}"#);

        let messages = vec![Message::assistant_with_tools(
            "Running both",
            vec![tc1, tc2],
        )];
        let converted = convert_messages(messages);

        assert_eq!(converted.len(), 1);
        let tool_calls = converted[0].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0].function.name, "tool_a");
        assert_eq!(tool_calls[1].function.name, "tool_b");
    }
}
