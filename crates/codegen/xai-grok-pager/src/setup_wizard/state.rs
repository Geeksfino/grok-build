use indexmap::IndexMap;

use super::validate::ValidateRequest;
use crate::provider_config_write::ProviderModelWrite;
use crate::provider_presets::{ProviderPreset, all_presets};

const CUSTOM_PROVIDER_ENV_KEY: &str = "CUSTOM_PROVIDER_API_KEY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupWizardPhase {
    SelectPreset,
    EditFields,
    Validating,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFieldFocus {
    BaseUrl,
    Model,
    Name,
    ApiKey,
    StoreKeyInConfig,
    Submit,
    Back,
}

#[derive(Debug)]
pub struct SetupWizardSubmission {
    pub validate: ValidateRequest,
    pub write: ProviderModelWrite,
    pub post_setup_notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditFieldsState {
    pub preset_id: String,
    pub preset_label: String,
    pub base_url: String,
    pub model: String,
    pub name: String,
    pub api_key_input: String,
    pub store_key_in_config: bool,
    pub api_backend: String,
    pub env_key_name: Option<String>,
    pub auth_not_required: bool,
    pub extra_headers: IndexMap<String, String>,
    pub focus: EditFieldFocus,
}

impl EditFieldsState {
    fn from_preset(preset: &ProviderPreset) -> Self {
        let env_key_name = match (preset.default_env_key, preset.auth_not_required) {
            (_, true) => None,
            (Some(key), false) => Some(key.to_string()),
            (None, false) => Some(derived_env_key_for_preset(preset.id)),
        };
        Self {
            preset_id: preset.id.to_string(),
            preset_label: preset.label.to_string(),
            base_url: preset.base_url.to_string(),
            model: String::new(),
            name: preset.label.to_string(),
            api_key_input: String::new(),
            store_key_in_config: false,
            api_backend: preset.api_backend.to_string(),
            env_key_name,
            auth_not_required: preset.auth_not_required,
            extra_headers: preset.extra_headers.clone(),
            focus: EditFieldFocus::BaseUrl,
        }
    }

    pub fn requires_api_key(&self) -> bool {
        !self.auth_not_required
    }

    pub fn visible_focus_order(&self) -> Vec<EditFieldFocus> {
        let mut order = vec![
            EditFieldFocus::BaseUrl,
            EditFieldFocus::Model,
            EditFieldFocus::Name,
        ];
        if self.requires_api_key() {
            order.push(EditFieldFocus::ApiKey);
            order.push(EditFieldFocus::StoreKeyInConfig);
        }
        order.push(EditFieldFocus::Submit);
        order.push(EditFieldFocus::Back);
        order
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupWizardState {
    preset_cursor: usize,
    phase: SetupWizardPhase,
    edit_fields: Option<EditFieldsState>,
}

impl Default for SetupWizardState {
    fn default() -> Self {
        Self::new()
    }
}

impl SetupWizardState {
    pub fn new() -> Self {
        Self {
            preset_cursor: 0,
            phase: SetupWizardPhase::SelectPreset,
            edit_fields: None,
        }
    }

    pub fn phase(&self) -> &SetupWizardPhase {
        &self.phase
    }

    pub fn preset_cursor(&self) -> usize {
        self.preset_cursor
    }

    pub fn presets(&self) -> &'static [ProviderPreset] {
        all_presets()
    }

    pub fn selected_preset(&self) -> &'static ProviderPreset {
        &all_presets()[self.preset_cursor]
    }

    pub fn edit_fields(&self) -> Option<&EditFieldsState> {
        self.edit_fields.as_ref()
    }

    pub fn edit_fields_mut(&mut self) -> Option<&mut EditFieldsState> {
        self.edit_fields.as_mut()
    }

    pub fn select_next_preset(&mut self) {
        let total = all_presets().len();
        if total == 0 {
            return;
        }
        self.preset_cursor = (self.preset_cursor + 1) % total;
    }

    pub fn select_prev_preset(&mut self) {
        let total = all_presets().len();
        if total == 0 {
            return;
        }
        self.preset_cursor = if self.preset_cursor == 0 {
            total - 1
        } else {
            self.preset_cursor - 1
        };
    }

    pub fn select_preset_by_id(&mut self, preset_id: &str) -> Result<(), String> {
        let Some((idx, _preset)) = all_presets()
            .iter()
            .enumerate()
            .find(|(_, preset)| preset.id == preset_id)
        else {
            return Err(format!("Unknown preset: {preset_id}"));
        };
        self.preset_cursor = idx;
        Ok(())
    }

    pub fn confirm_selected_preset(&mut self) -> Result<(), String> {
        let preset = self.selected_preset();
        self.edit_fields = Some(EditFieldsState::from_preset(preset));
        self.phase = SetupWizardPhase::EditFields;
        Ok(())
    }

    pub fn set_model(&mut self, value: &str) {
        if let Some(fields) = self.edit_fields_mut() {
            fields.model = value.to_string();
            self.clear_error_state();
        }
    }

    pub fn focus(&self) -> Option<EditFieldFocus> {
        self.edit_fields.as_ref().map(|fields| fields.focus)
    }

    pub fn focus_next(&mut self) {
        let Some(fields) = self.edit_fields_mut() else {
            return;
        };
        let order = fields.visible_focus_order();
        let Some(current_idx) = order.iter().position(|focus| *focus == fields.focus) else {
            fields.focus = EditFieldFocus::BaseUrl;
            return;
        };
        fields.focus = order[(current_idx + 1) % order.len()];
        self.clear_error_state();
    }

    pub fn focus_prev(&mut self) {
        let Some(fields) = self.edit_fields_mut() else {
            return;
        };
        let order = fields.visible_focus_order();
        let Some(current_idx) = order.iter().position(|focus| *focus == fields.focus) else {
            fields.focus = EditFieldFocus::BaseUrl;
            return;
        };
        fields.focus = if current_idx == 0 {
            order[order.len() - 1]
        } else {
            order[current_idx - 1]
        };
        self.clear_error_state();
    }

    pub fn return_to_preset_select(&mut self) {
        self.phase = SetupWizardPhase::SelectPreset;
    }

    pub fn start_validating(&mut self) {
        self.phase = SetupWizardPhase::Validating;
    }

    pub fn finish_with_error(&mut self, message: impl Into<String>) {
        self.phase = SetupWizardPhase::Error(message.into());
    }

    pub fn clear_error_state(&mut self) {
        if matches!(self.phase, SetupWizardPhase::Error(_)) {
            self.phase = SetupWizardPhase::EditFields;
        }
    }

    pub fn current_error(&self) -> Option<&str> {
        match &self.phase {
            SetupWizardPhase::Error(message) => Some(message.as_str()),
            _ => None,
        }
    }

    pub fn insert_char(&mut self, ch: char) -> bool {
        let Some(fields) = self.edit_fields_mut() else {
            return false;
        };
        let target = match fields.focus {
            EditFieldFocus::BaseUrl => &mut fields.base_url,
            EditFieldFocus::Model => &mut fields.model,
            EditFieldFocus::Name => &mut fields.name,
            EditFieldFocus::ApiKey => &mut fields.api_key_input,
            _ => return false,
        };
        target.push(ch);
        self.clear_error_state();
        true
    }

    pub fn push_str(&mut self, value: &str) -> bool {
        let Some(fields) = self.edit_fields_mut() else {
            return false;
        };
        let target = match fields.focus {
            EditFieldFocus::BaseUrl => &mut fields.base_url,
            EditFieldFocus::Model => &mut fields.model,
            EditFieldFocus::Name => &mut fields.name,
            EditFieldFocus::ApiKey => &mut fields.api_key_input,
            _ => return false,
        };
        target.push_str(value);
        self.clear_error_state();
        true
    }

    pub fn backspace(&mut self) -> bool {
        let Some(fields) = self.edit_fields_mut() else {
            return false;
        };
        let target = match fields.focus {
            EditFieldFocus::BaseUrl => &mut fields.base_url,
            EditFieldFocus::Model => &mut fields.model,
            EditFieldFocus::Name => &mut fields.name,
            EditFieldFocus::ApiKey => &mut fields.api_key_input,
            _ => return false,
        };
        if target.pop().is_some() {
            self.clear_error_state();
            true
        } else {
            false
        }
    }

    pub fn toggle_store_key_in_config(&mut self) -> bool {
        let Some(fields) = self.edit_fields_mut() else {
            return false;
        };
        if !fields.requires_api_key() {
            return false;
        }
        fields.store_key_in_config = !fields.store_key_in_config;
        self.clear_error_state();
        true
    }

    pub fn build_validate_request(&self) -> Result<ValidateRequest, String> {
        let Some(fields) = self.edit_fields() else {
            return Err("No preset selected".to_string());
        };
        let base_url = fields.base_url.trim();
        if base_url.is_empty() {
            return Err("Provider base URL is required".to_string());
        }
        let model = fields.model.trim();
        if model.is_empty() {
            return Err("Provider model is required".to_string());
        }
        let api_key = resolve_probe_api_key(fields)?;
        Ok(ValidateRequest {
            base_url: base_url.to_string(),
            model: model.to_string(),
            api_key,
            api_backend: fields.api_backend.clone(),
            auth_not_required: fields.auth_not_required,
            extra_headers: fields.extra_headers.clone(),
        })
    }

    pub fn build_provider_write(&self) -> Result<ProviderModelWrite, String> {
        let Some(fields) = self.edit_fields() else {
            return Err("No preset selected".to_string());
        };
        let base_url = fields.base_url.trim();
        if base_url.is_empty() {
            return Err("Provider base URL is required".to_string());
        }
        let model = fields.model.trim();
        if model.is_empty() {
            return Err("Provider model is required".to_string());
        }
        let name = if fields.name.trim().is_empty() {
            default_display_name(&fields.preset_label, model)
        } else {
            fields.name.trim().to_string()
        };
        let stored_api_key = stored_api_key(fields);
        Ok(ProviderModelWrite {
            catalog_id: format!("{}-{}", fields.preset_id, sanitize_catalog_component(model)),
            model: model.to_string(),
            name,
            base_url: base_url.to_string(),
            api_backend: fields.api_backend.clone(),
            env_key: if fields.auth_not_required {
                None
            } else {
                fields.env_key_name.clone()
            },
            api_key: stored_api_key,
            auth_not_required: fields.auth_not_required,
            extra_headers: fields.extra_headers.clone(),
        })
    }

    pub fn build_submission(&self) -> Result<SetupWizardSubmission, String> {
        let Some(fields) = self.edit_fields() else {
            return Err("No preset selected".to_string());
        };
        let validate = self.build_validate_request()?;
        let write = self.build_provider_write()?;
        let post_setup_notice = env_fallback_notice(fields);
        Ok(SetupWizardSubmission {
            validate,
            write,
            post_setup_notice,
        })
    }
}

fn stored_api_key(fields: &EditFieldsState) -> Option<String> {
    let entered = non_empty(fields.api_key_input.trim()).map(str::to_string)?;
    if fields.store_key_in_config || should_persist_typed_key_for_env_fallback(fields) {
        Some(entered)
    } else {
        None
    }
}

fn env_fallback_notice(fields: &EditFieldsState) -> Option<String> {
    let env_key = fields.env_key_name.as_deref()?;
    should_persist_typed_key_for_env_fallback(fields).then(|| {
        format!(
            "Saved provider config. {env_key} is not exported in this environment, so the typed API key was saved in config for reconnect."
        )
    })
}

fn should_persist_typed_key_for_env_fallback(fields: &EditFieldsState) -> bool {
    !fields.auth_not_required
        && !fields.store_key_in_config
        && non_empty(fields.api_key_input.trim()).is_some()
        && fields
            .env_key_name
            .as_ref()
            .is_some_and(|key_name| std::env::var_os(key_name).is_none())
}

fn resolve_probe_api_key(fields: &EditFieldsState) -> Result<Option<String>, String> {
    if fields.auth_not_required {
        return Ok(None);
    }
    if let Some(entered) = non_empty(fields.api_key_input.trim()) {
        return Ok(Some(entered.to_string()));
    }
    if let Some(env_key) = fields.env_key_name.as_ref()
        && let Some(env_value) = std::env::var_os(env_key)
    {
        let value = env_value.to_string_lossy();
        if let Some(non_empty) = non_empty(value.trim()) {
            return Ok(Some(non_empty.to_string()));
        }
    }
    let missing = fields
        .env_key_name
        .as_deref()
        .unwrap_or("provider API key environment variable");
    Err(format!(
        "API key required (enter one now or export {missing})"
    ))
}

fn sanitize_catalog_component(model: &str) -> String {
    let mut sanitized = String::with_capacity(model.len());
    let mut last_dash = false;
    for ch in model.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            sanitized.push('-');
            last_dash = true;
        }
    }
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "model".to_string()
    } else {
        sanitized.to_string()
    }
}

fn default_display_name(preset_label: &str, model: &str) -> String {
    format!("{preset_label} {model}")
}

fn derived_env_key_for_preset(preset_id: &str) -> String {
    if preset_id.eq_ignore_ascii_case("custom") {
        return CUSTOM_PROVIDER_ENV_KEY.to_string();
    }
    let mut env = String::new();
    for ch in preset_id.chars() {
        if ch.is_ascii_alphanumeric() {
            env.push(ch.to_ascii_uppercase());
        } else {
            env.push('_');
        }
    }
    if env.is_empty() {
        CUSTOM_PROVIDER_ENV_KEY.to_string()
    } else {
        format!("{env}_API_KEY")
    }
}

fn non_empty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}
