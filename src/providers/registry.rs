//! Provider registry and resolution helpers.
//!
//! This module centralizes provider metadata and the mapping from configuration
//! to runtime provider selection.

use crate::config::{Config, ProviderConfig};

/// Metadata describing an LLM provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderSpec {
    /// Config key / provider id (e.g. "openai").
    pub name: &'static str,
    /// Model keywords commonly associated with this provider.
    pub model_keywords: &'static [&'static str],
    /// Whether this provider is currently wired for runtime execution.
    pub runtime_supported: bool,
}

/// Runtime-ready provider selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProviderSelection {
    /// Selected provider id.
    pub name: &'static str,
    /// API key used for provider auth.
    pub api_key: String,
    /// Optional provider base URL.
    pub api_base: Option<String>,
}

/// Provider registry in priority order.
///
/// Runtime selection follows this order for runtime-supported providers.
pub const PROVIDER_REGISTRY: &[ProviderSpec] = &[
    ProviderSpec {
        name: "anthropic",
        model_keywords: &["anthropic", "claude"],
        runtime_supported: true,
    },
    ProviderSpec {
        name: "openai",
        model_keywords: &["openai", "gpt"],
        runtime_supported: true,
    },
    ProviderSpec {
        name: "openrouter",
        model_keywords: &["openrouter"],
        runtime_supported: false,
    },
    ProviderSpec {
        name: "groq",
        model_keywords: &["groq"],
        runtime_supported: false,
    },
    ProviderSpec {
        name: "zhipu",
        model_keywords: &["zhipu", "glm", "zai"],
        runtime_supported: false,
    },
    ProviderSpec {
        name: "vllm",
        model_keywords: &["vllm"],
        runtime_supported: false,
    },
    ProviderSpec {
        name: "gemini",
        model_keywords: &["gemini"],
        runtime_supported: false,
    },
];

fn provider_config_by_name<'a>(config: &'a Config, name: &str) -> Option<&'a ProviderConfig> {
    match name {
        "anthropic" => config.providers.anthropic.as_ref(),
        "openai" => config.providers.openai.as_ref(),
        "openrouter" => config.providers.openrouter.as_ref(),
        "groq" => config.providers.groq.as_ref(),
        "zhipu" => config.providers.zhipu.as_ref(),
        "vllm" => config.providers.vllm.as_ref(),
        "gemini" => config.providers.gemini.as_ref(),
        _ => None,
    }
}

fn configured_api_key(provider: Option<&ProviderConfig>) -> Option<&str> {
    provider
        .and_then(|p| p.api_key.as_deref())
        .and_then(|k| if k.is_empty() { None } else { Some(k) })
}

/// Returns all configured provider ids in registry order.
pub fn configured_provider_names(config: &Config) -> Vec<&'static str> {
    PROVIDER_REGISTRY
        .iter()
        .filter_map(|spec| {
            configured_api_key(provider_config_by_name(config, spec.name)).map(|_| spec.name)
        })
        .collect()
}

/// Returns configured provider ids that are not yet runtime-supported.
pub fn configured_unsupported_provider_names(config: &Config) -> Vec<&'static str> {
    PROVIDER_REGISTRY
        .iter()
        .filter_map(|spec| {
            if spec.runtime_supported {
                None
            } else {
                configured_api_key(provider_config_by_name(config, spec.name)).map(|_| spec.name)
            }
        })
        .collect()
}

/// Resolve the provider currently used by runtime execution.
///
/// Priority follows `PROVIDER_REGISTRY` order for `runtime_supported` providers.
pub fn resolve_runtime_provider(config: &Config) -> Option<RuntimeProviderSelection> {
    for spec in PROVIDER_REGISTRY
        .iter()
        .filter(|spec| spec.runtime_supported)
    {
        let provider = provider_config_by_name(config, spec.name);
        let Some(api_key) = configured_api_key(provider) else {
            continue;
        };

        let api_base = provider.and_then(|p| p.api_base.clone()).and_then(|base| {
            if base.is_empty() {
                None
            } else {
                Some(base)
            }
        });

        return Some(RuntimeProviderSelection {
            name: spec.name,
            api_key: api_key.to_string(),
            api_base,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configured_provider_names_registry_order() {
        let mut config = Config::default();
        config.providers.openai = Some(ProviderConfig {
            api_key: Some("sk-openai".to_string()),
            ..Default::default()
        });
        config.providers.anthropic = Some(ProviderConfig {
            api_key: Some("sk-ant".to_string()),
            ..Default::default()
        });

        let names = configured_provider_names(&config);
        assert_eq!(names, vec!["anthropic", "openai"]);
    }

    #[test]
    fn test_configured_unsupported_provider_names() {
        let mut config = Config::default();
        config.providers.openrouter = Some(ProviderConfig {
            api_key: Some("sk-or".to_string()),
            ..Default::default()
        });
        config.providers.groq = Some(ProviderConfig {
            api_key: Some("sk-groq".to_string()),
            ..Default::default()
        });

        let names = configured_unsupported_provider_names(&config);
        assert_eq!(names, vec!["openrouter", "groq"]);
    }

    #[test]
    fn test_resolve_runtime_provider_priority() {
        let mut config = Config::default();
        config.providers.openai = Some(ProviderConfig {
            api_key: Some("sk-openai".to_string()),
            api_base: Some("https://example.com/v1".to_string()),
            ..Default::default()
        });
        config.providers.anthropic = Some(ProviderConfig {
            api_key: Some("sk-ant".to_string()),
            ..Default::default()
        });

        let selected = resolve_runtime_provider(&config).expect("provider should resolve");
        assert_eq!(selected.name, "anthropic");
        assert_eq!(selected.api_key, "sk-ant");
        assert_eq!(selected.api_base, None);
    }

    #[test]
    fn test_resolve_runtime_provider_openai_base_url() {
        let mut config = Config::default();
        config.providers.openai = Some(ProviderConfig {
            api_key: Some("sk-openai".to_string()),
            api_base: Some("https://example.com/v1".to_string()),
            ..Default::default()
        });

        let selected = resolve_runtime_provider(&config).expect("provider should resolve");
        assert_eq!(selected.name, "openai");
        assert_eq!(selected.api_key, "sk-openai");
        assert_eq!(selected.api_base.as_deref(), Some("https://example.com/v1"));
    }

    #[test]
    fn test_runtime_supported_constant_stays_in_sync() {
        let runtime_supported: Vec<&str> = PROVIDER_REGISTRY
            .iter()
            .filter(|spec| spec.runtime_supported)
            .map(|spec| spec.name)
            .collect();

        assert_eq!(
            runtime_supported,
            crate::providers::RUNTIME_SUPPORTED_PROVIDERS
        );
    }
}
