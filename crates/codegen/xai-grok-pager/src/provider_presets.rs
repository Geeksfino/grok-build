use indexmap::IndexMap;
use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub base_url: &'static str,
    pub api_backend: &'static str,
    pub default_env_key: Option<&'static str>,
    pub auth_not_required: bool,
    pub extra_headers: IndexMap<String, String>,
}

static PROVIDER_PRESETS: LazyLock<Vec<ProviderPreset>> = LazyLock::new(|| {
    vec![
        ProviderPreset {
            id: "openai",
            label: "OpenAI",
            base_url: "https://api.openai.com/v1",
            api_backend: "chat_completions",
            default_env_key: Some("OPENAI_API_KEY"),
            auth_not_required: false,
            extra_headers: IndexMap::new(),
        },
        ProviderPreset {
            id: "anthropic",
            label: "Anthropic",
            base_url: "https://api.anthropic.com/v1",
            api_backend: "messages",
            default_env_key: Some("ANTHROPIC_API_KEY"),
            auth_not_required: false,
            extra_headers: IndexMap::from([(
                "anthropic-version".to_string(),
                "2023-06-01".to_string(),
            )]),
        },
        ProviderPreset {
            id: "ollama",
            label: "Ollama",
            base_url: "http://localhost:11434/v1",
            api_backend: "chat_completions",
            default_env_key: None,
            auth_not_required: true,
            extra_headers: IndexMap::new(),
        },
        ProviderPreset {
            id: "openrouter",
            label: "OpenRouter",
            base_url: "https://openrouter.ai/api/v1",
            api_backend: "chat_completions",
            default_env_key: Some("OPENROUTER_API_KEY"),
            auth_not_required: false,
            extra_headers: IndexMap::new(),
        },
        ProviderPreset {
            id: "groq",
            label: "Groq",
            base_url: "https://api.groq.com/openai/v1",
            api_backend: "chat_completions",
            default_env_key: Some("GROQ_API_KEY"),
            auth_not_required: false,
            extra_headers: IndexMap::new(),
        },
        ProviderPreset {
            id: "together",
            label: "Together",
            base_url: "https://api.together.xyz/v1",
            api_backend: "chat_completions",
            default_env_key: Some("TOGETHER_API_KEY"),
            auth_not_required: false,
            extra_headers: IndexMap::new(),
        },
        ProviderPreset {
            id: "deepseek",
            label: "DeepSeek",
            base_url: "https://api.deepseek.com/v1",
            api_backend: "chat_completions",
            default_env_key: Some("DEEPSEEK_API_KEY"),
            auth_not_required: false,
            extra_headers: IndexMap::new(),
        },
        ProviderPreset {
            id: "custom",
            label: "Custom OpenAI-compatible",
            base_url: "",
            api_backend: "chat_completions",
            default_env_key: None,
            auth_not_required: false,
            extra_headers: IndexMap::new(),
        },
    ]
});

pub fn all_presets() -> &'static [ProviderPreset] {
    PROVIDER_PRESETS.as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_includes_expected_ids() {
        let ids: Vec<_> = all_presets().iter().map(|preset| preset.id).collect();

        for expected in [
            "openai",
            "anthropic",
            "ollama",
            "openrouter",
            "groq",
            "together",
            "deepseek",
            "custom",
        ] {
            assert!(
                ids.contains(&expected),
                "expected preset list to contain {expected:?}, got {ids:?}"
            );
        }
    }

    #[test]
    fn anthropic_and_ollama_presets_capture_special_auth_behavior() {
        let anthropic = all_presets()
            .iter()
            .find(|preset| preset.id == "anthropic")
            .expect("anthropic preset");
        assert_eq!(anthropic.api_backend, "messages");
        assert_eq!(
            anthropic
                .extra_headers
                .get("anthropic-version")
                .map(String::as_str),
            Some("2023-06-01")
        );

        let ollama = all_presets()
            .iter()
            .find(|preset| preset.id == "ollama")
            .expect("ollama preset");
        assert!(ollama.auth_not_required);
        assert_eq!(ollama.default_env_key, None);
    }
}
