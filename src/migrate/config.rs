//! OpenClaw config → ZeptoClaw config conversion.

use serde_json::Value;

use crate::config::{
    CompactionConfig, Config, DiscordConfig, ProviderConfig, SlackConfig, TelegramConfig,
};

/// Result of a config conversion: lists of migrated / skipped / not-portable fields.
pub struct MigrationConfigResult {
    pub migrated: Vec<String>,
    pub skipped: Vec<(String, String)>,
    pub not_portable: Vec<String>,
}

/// Convert an OpenClaw config (parsed JSON `Value`) into a ZeptoClaw `Config`,
/// merging into `existing`. Returns what was migrated, skipped, and not portable.
pub fn convert_config(openclaw: &Value, existing: &mut Config) -> MigrationConfigResult {
    let mut migrated = Vec::new();
    let mut skipped = Vec::new();
    let mut not_portable = Vec::new();

    // ── Providers ──────────────────────────────────────────────────────
    migrate_provider(openclaw, existing, "anthropic", &mut migrated);
    migrate_provider(openclaw, existing, "openai", &mut migrated);
    migrate_provider_key_only(openclaw, existing, "openrouter", &mut migrated);
    migrate_provider_key_only(openclaw, existing, "groq", &mut migrated);

    // ── Agent defaults ────────────────────────────────────────────────

    // Model (primary from nested object or plain string)
    if let Some(model_val) = pointer(openclaw, &["agents", "defaults", "model"]) {
        if let Some(primary) = model_val.get("primary").and_then(|v| v.as_str()) {
            existing.agents.defaults.model = primary.to_string();
            migrated.push("agents.defaults.model".into());
        } else if let Some(s) = model_val.as_str() {
            existing.agents.defaults.model = s.to_string();
            migrated.push("agents.defaults.model".into());
        }
    }

    // Workspace
    if let Some(ws) = str_at(openclaw, &["agents", "defaults", "workspace"]) {
        existing.agents.defaults.workspace = ws.to_string();
        migrated.push("agents.defaults.workspace".into());
    }

    // contextTokens → compaction
    if let Some(ct) = pointer(openclaw, &["agents", "defaults", "contextTokens"]) {
        if let Some(n) = ct.as_u64() {
            existing.compaction = CompactionConfig {
                enabled: true,
                context_limit: n as usize,
                ..existing.compaction.clone()
            };
            migrated.push("agents.defaults.contextTokens -> compaction".into());
        }
    }

    // ── Channels ──────────────────────────────────────────────────────

    // Telegram
    if let Some(token) = str_at(openclaw, &["channels", "telegram", "token"]) {
        let tg = existing
            .channels
            .telegram
            .get_or_insert_with(TelegramConfig::default);
        tg.token = token.to_string();
        tg.enabled = true;
        migrated.push("channels.telegram.token".into());
    }

    // Discord
    if let Some(token) = str_at(openclaw, &["channels", "discord", "token"]) {
        let dc = existing
            .channels
            .discord
            .get_or_insert_with(DiscordConfig::default);
        dc.token = token.to_string();
        dc.enabled = true;
        migrated.push("channels.discord.token".into());
    }

    // Slack — OpenClaw uses "token", ZeptoClaw uses "bot_token"
    if let Some(token) = str_at(openclaw, &["channels", "slack", "token"]) {
        let sl = existing
            .channels
            .slack
            .get_or_insert_with(SlackConfig::default);
        sl.bot_token = token.to_string();
        sl.enabled = true;
        migrated.push("channels.slack.token -> bot_token".into());
    }

    // ── Tools ─────────────────────────────────────────────────────────

    // Web search API key
    if let Some(key) = str_at(openclaw, &["tools", "web", "search", "apiKey"]) {
        existing.tools.web.search.api_key = Some(key.to_string());
        migrated.push("tools.web.search.apiKey".into());
    }

    // Web search maxResults
    if let Some(n) =
        pointer(openclaw, &["tools", "web", "search", "maxResults"]).and_then(|v| v.as_u64())
    {
        existing.tools.web.search.max_results = n as u32;
        migrated.push("tools.web.search.maxResults".into());
    }

    // ── Approvals ─────────────────────────────────────────────────────
    if let Some(approvals) = openclaw.get("approvals") {
        existing.approval.enabled = true;

        if let Some(deny) = approvals.get("deny").and_then(|v| v.as_array()) {
            existing.approval.require_for = deny
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect();
        }
        migrated.push("approvals -> approval".into());
    }

    // ── Gateway ───────────────────────────────────────────────────────
    if let Some(port) = pointer(openclaw, &["gateway", "port"]).and_then(|v| v.as_u64()) {
        existing.gateway.port = port as u16;
        migrated.push("gateway.port".into());
    }

    // ── Not-portable fields ───────────────────────────────────────────
    check_not_portable(openclaw, &mut not_portable, &mut skipped);

    MigrationConfigResult {
        migrated,
        skipped,
        not_portable,
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Migrate a provider (api_key + api_base) from OpenClaw's
/// `models.providers.<name>.{apiKey,baseUrl}`.
fn migrate_provider(oc: &Value, config: &mut Config, name: &str, migrated: &mut Vec<String>) {
    let api_key = str_at(oc, &["models", "providers", name, "apiKey"]);
    let api_base = str_at(oc, &["models", "providers", name, "baseUrl"]);

    if api_key.is_none() && api_base.is_none() {
        return;
    }

    let provider = match get_provider_mut(&mut config.providers, name) {
        Some(p) => p,
        None => return,
    };

    if let Some(key) = api_key {
        provider.api_key = Some(key.to_string());
        migrated.push(format!("providers.{}.api_key", name));
    }
    if let Some(base) = api_base {
        provider.api_base = Some(base.to_string());
        migrated.push(format!("providers.{}.api_base", name));
    }
}

/// Migrate a provider with only api_key (no baseUrl).
fn migrate_provider_key_only(
    oc: &Value,
    config: &mut Config,
    name: &str,
    migrated: &mut Vec<String>,
) {
    if let Some(key) = str_at(oc, &["models", "providers", name, "apiKey"]) {
        if let Some(provider) = get_provider_mut(&mut config.providers, name) {
            provider.api_key = Some(key.to_string());
            migrated.push(format!("providers.{}.api_key", name));
        }
    }
}

/// Get a mutable reference to a provider entry by name, creating it if absent.
/// Returns `None` for unrecognised provider names.
fn get_provider_mut<'a>(
    providers: &'a mut crate::config::ProvidersConfig,
    name: &str,
) -> Option<&'a mut ProviderConfig> {
    match name {
        "anthropic" => Some(
            providers
                .anthropic
                .get_or_insert_with(ProviderConfig::default),
        ),
        "openai" => Some(providers.openai.get_or_insert_with(ProviderConfig::default)),
        "openrouter" => Some(
            providers
                .openrouter
                .get_or_insert_with(ProviderConfig::default),
        ),
        "groq" => Some(providers.groq.get_or_insert_with(ProviderConfig::default)),
        "zhipu" => Some(providers.zhipu.get_or_insert_with(ProviderConfig::default)),
        "vllm" => Some(providers.vllm.get_or_insert_with(ProviderConfig::default)),
        "gemini" => Some(providers.gemini.get_or_insert_with(ProviderConfig::default)),
        "ollama" => Some(providers.ollama.get_or_insert_with(ProviderConfig::default)),
        _ => None,
    }
}

/// Check for OpenClaw features that are not portable and add to report.
fn check_not_portable(
    oc: &Value,
    not_portable: &mut Vec<String>,
    skipped: &mut Vec<(String, String)>,
) {
    // session.scope / dmScope
    if pointer(oc, &["session", "scope"]).is_some() {
        not_portable
            .push("session.scope — ZeptoClaw uses container-per-request isolation instead".into());
    }
    if pointer(oc, &["session", "dmScope"]).is_some() {
        not_portable.push("session.dmScope — use allow_from allowlists per channel".into());
    }

    // tools.profile
    if pointer(oc, &["tools", "profile"]).is_some() {
        not_portable.push("tools.profile — use ZeptoClaw's approval gate instead".into());
    }

    // tools.exec.host: "sandbox"
    if str_at(oc, &["tools", "exec", "host"]) == Some("sandbox") {
        not_portable.push("tools.exec.host: \"sandbox\" — use runtime config instead".into());
    }

    // Non-Brave search provider
    if let Some(provider) = str_at(oc, &["tools", "web", "search", "provider"]) {
        if provider != "brave" {
            skipped.push((
                format!("tools.web.search.provider: {}", provider),
                "ZeptoClaw supports Brave Search only".into(),
            ));
        }
    }

    // talk.*
    if oc.get("talk").is_some() {
        not_portable.push("talk.* — voice features not supported".into());
    }

    // auth.profiles (OAuth)
    if pointer(oc, &["auth", "profiles"]).is_some() {
        not_portable.push("auth.profiles — ZeptoClaw uses API key authentication only".into());
    }

    // browser.*
    if oc.get("browser").is_some() {
        not_portable.push("browser.* — use an MCP server instead".into());
    }

    // Unsupported channels
    for ch in &[
        "signal",
        "imessage",
        "matrix",
        "line",
        "irc",
        "mattermost",
        "teams",
    ] {
        if pointer(oc, &["channels", ch]).is_some() {
            not_portable.push(format!("channels.{} — use the webhook adapter pattern", ch));
        }
    }

    // agents.list (multi-agent definitions)
    if pointer(oc, &["agents", "list"]).is_some() {
        not_portable
            .push("agents.list — multi-agent definitions have no direct equivalent yet".into());
    }

    // plugins.installs
    if pointer(oc, &["plugins", "installs"]).is_some() {
        not_portable.push("plugins.installs — ZeptoClaw uses a different plugin system".into());
    }
}

/// Navigate nested JSON by a sequence of keys.
fn pointer<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in keys {
        current = current.get(*key)?;
    }
    if current.is_null() {
        None
    } else {
        Some(current)
    }
}

/// Get a string value at a nested path.
fn str_at<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    pointer(value, keys).and_then(|v| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_openclaw_config() -> Value {
        serde_json::json!({
            "models": {
                "providers": {
                    "anthropic": {
                        "apiKey": "sk-ant-test123",
                        "baseUrl": "https://api.anthropic.com"
                    },
                    "openai": {
                        "apiKey": "sk-openai-test",
                        "baseUrl": "https://api.openai.com/v1"
                    },
                    "groq": {
                        "apiKey": "gsk-test"
                    }
                }
            },
            "agents": {
                "defaults": {
                    "model": { "primary": "claude-sonnet-4-5-20250929" },
                    "workspace": "~/projects",
                    "contextTokens": 80000
                }
            },
            "channels": {
                "telegram": { "token": "123456:ABC" },
                "discord": { "token": "MTIz-discord" },
                "slack": { "token": "xoxb-slack-token" }
            },
            "tools": {
                "web": {
                    "search": {
                        "apiKey": "brave-key-123",
                        "maxResults": 8
                    }
                }
            },
            "gateway": { "port": 9090 },
            "session": { "scope": "per-sender" },
            "talk": { "enabled": true },
            "browser": { "headless": true }
        })
    }

    #[test]
    fn test_convert_providers() {
        let oc = sample_openclaw_config();
        let mut config = Config::default();
        let result = convert_config(&oc, &mut config);

        let anthropic = config.providers.anthropic.as_ref().unwrap();
        assert_eq!(anthropic.api_key.as_deref(), Some("sk-ant-test123"));
        assert_eq!(
            anthropic.api_base.as_deref(),
            Some("https://api.anthropic.com")
        );

        let openai = config.providers.openai.as_ref().unwrap();
        assert_eq!(openai.api_key.as_deref(), Some("sk-openai-test"));

        let groq = config.providers.groq.as_ref().unwrap();
        assert_eq!(groq.api_key.as_deref(), Some("gsk-test"));

        assert!(result
            .migrated
            .contains(&"providers.anthropic.api_key".to_string()));
        assert!(result
            .migrated
            .contains(&"providers.openai.api_key".to_string()));
        assert!(result
            .migrated
            .contains(&"providers.groq.api_key".to_string()));
    }

    #[test]
    fn test_convert_agent_defaults() {
        let oc = sample_openclaw_config();
        let mut config = Config::default();
        convert_config(&oc, &mut config);

        assert_eq!(config.agents.defaults.model, "claude-sonnet-4-5-20250929");
        assert_eq!(config.agents.defaults.workspace, "~/projects");
    }

    #[test]
    fn test_convert_context_tokens_to_compaction() {
        let oc = sample_openclaw_config();
        let mut config = Config::default();
        convert_config(&oc, &mut config);

        assert!(config.compaction.enabled);
        assert_eq!(config.compaction.context_limit, 80000);
    }

    #[test]
    fn test_convert_channels() {
        let oc = sample_openclaw_config();
        let mut config = Config::default();
        convert_config(&oc, &mut config);

        let tg = config.channels.telegram.as_ref().unwrap();
        assert!(tg.enabled);
        assert_eq!(tg.token, "123456:ABC");

        let dc = config.channels.discord.as_ref().unwrap();
        assert!(dc.enabled);
        assert_eq!(dc.token, "MTIz-discord");

        let sl = config.channels.slack.as_ref().unwrap();
        assert!(sl.enabled);
        assert_eq!(sl.bot_token, "xoxb-slack-token");
    }

    #[test]
    fn test_convert_tools() {
        let oc = sample_openclaw_config();
        let mut config = Config::default();
        convert_config(&oc, &mut config);

        assert_eq!(
            config.tools.web.search.api_key.as_deref(),
            Some("brave-key-123")
        );
        assert_eq!(config.tools.web.search.max_results, 8);
    }

    #[test]
    fn test_convert_gateway() {
        let oc = sample_openclaw_config();
        let mut config = Config::default();
        convert_config(&oc, &mut config);

        assert_eq!(config.gateway.port, 9090);
    }

    #[test]
    fn test_not_portable_detection() {
        let oc = sample_openclaw_config();
        let mut config = Config::default();
        let result = convert_config(&oc, &mut config);

        assert!(result
            .not_portable
            .iter()
            .any(|s| s.contains("session.scope")));
        assert!(result.not_portable.iter().any(|s| s.contains("talk.")));
        assert!(result.not_portable.iter().any(|s| s.contains("browser.")));
    }

    #[test]
    fn test_convert_empty_config() {
        let oc = serde_json::json!({});
        let mut config = Config::default();
        let result = convert_config(&oc, &mut config);

        assert!(result.migrated.is_empty());
        assert!(result.not_portable.is_empty());
    }

    #[test]
    fn test_convert_model_string() {
        let oc = serde_json::json!({
            "agents": {
                "defaults": {
                    "model": "gpt-4"
                }
            }
        });
        let mut config = Config::default();
        convert_config(&oc, &mut config);

        assert_eq!(config.agents.defaults.model, "gpt-4");
    }

    #[test]
    fn test_convert_preserves_existing_config() {
        let oc = serde_json::json!({
            "models": {
                "providers": {
                    "anthropic": { "apiKey": "new-key" }
                }
            }
        });
        let mut config = Config::default();
        config.gateway.port = 1234;
        config.agents.defaults.max_tokens = 4096;

        convert_config(&oc, &mut config);

        // New value applied
        assert_eq!(
            config
                .providers
                .anthropic
                .as_ref()
                .unwrap()
                .api_key
                .as_deref(),
            Some("new-key")
        );
        // Existing values preserved
        assert_eq!(config.gateway.port, 1234);
        assert_eq!(config.agents.defaults.max_tokens, 4096);
    }
}
