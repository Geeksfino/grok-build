//! Default model IDs loaded from `default_models.json` at runtime.
//! Edit that JSON file to change them.
//!
//! At runtime each model is resolved via:
//!   CLI flag > ENV var > config.toml > remote settings > these defaults

use std::sync::LazyLock;

/// The raw JSON, embedded at compile time. Re-exported through the
/// `xai_grok_shell::models` facade and consumed by `agent::config`, so it must
/// be `pub` (was `pub(crate)` when this lived inside the shell crate).
pub const DEFAULT_MODELS_JSON: &str = include_str!("../default_models.json");

#[derive(serde::Deserialize)]
struct DefaultModels {
    default: String,
    /// Falls back to `default` if not specified in JSON.
    web_search: Option<String>,
    /// Falls back to `default` if not specified in JSON.
    image_description: Option<String>,
    /// Falls back to `default` if not specified in JSON.
    session_summary: Option<String>,
    models: Vec<DefaultModelEntry>,
}

#[derive(serde::Deserialize)]
struct DefaultModelEntry {
    model: String,
}

static DEFAULTS: LazyLock<DefaultModels> = LazyLock::new(|| {
    let defaults: DefaultModels = serde_json::from_str(DEFAULT_MODELS_JSON)
        .expect("default_models.json: invalid JSON or missing 'default' field");

    // Baked-in JSON — a mismatch here is a developer error, not a runtime condition.
    if defaults.default.is_empty() {
        assert!(
            defaults.models.is_empty(),
            "default_models.json: blank 'default' requires empty 'models' array"
        );
    } else {
        let model_ids: Vec<&str> = defaults.models.iter().map(|m| m.model.as_str()).collect();
        assert!(
            model_ids.contains(&defaults.default.as_str()),
            "default_models.json: 'default' is '{}' but 'models' array only has {model_ids:?}",
            defaults.default,
        );
    }

    defaults
});

/// Primary model for coding tasks and general fallback.
///
/// An empty string means there is no baked-in default; config or setup flows
/// must supply one.
pub fn default_model() -> &'static str {
    &DEFAULTS.default
}

/// Model for web search tool synthesis. Falls back to default model.
pub fn default_web_search_model() -> &'static str {
    DEFAULTS.web_search.as_deref().unwrap_or(&DEFAULTS.default)
}

/// Model for image describe. Falls back to default model.
pub fn default_image_description_model() -> &'static str {
    DEFAULTS
        .image_description
        .as_deref()
        .unwrap_or(&DEFAULTS.default)
}

/// Model for session title generation. Falls back to default model.
pub fn default_session_summary_model() -> &'static str {
    DEFAULTS
        .session_summary
        .as_deref()
        .unwrap_or(&DEFAULTS.default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_catalog_allows_blank_default() {
        let parsed: DefaultModels =
            serde_json::from_str(r#"{"default":"","models":[]}"#).expect("parse");
        assert_eq!(parsed.default, "");
        assert!(parsed.models.is_empty());
    }

    #[test]
    fn embedded_default_model_is_blank() {
        assert_eq!(default_model(), "");
        assert!(DEFAULTS.models.is_empty());
    }
}
