//! Setup wizard startup gating.
//!
//! Task 6 adds the cold-start state and gate. Task 7 will fill in the
//! interactive flow and rendering.

use std::fmt;

use agent_client_protocol as acp;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};
use unicode_width::UnicodeWidthStr;

pub mod state;
pub mod validate;

pub use state::{
    EditFieldFocus, EditFieldsState, SetupWizardPhase, SetupWizardState, SetupWizardSubmission,
};

use crate::app::actions::Action;
use crate::app::app_view::{AppView, AuthState};
use crate::theme::Theme;

pub const FORCE_PROVIDER_SETUP_ENV: &str = "GROK_FORCE_PROVIDER_SETUP";

pub struct SetupWizardCompletion {
    pub connection: Option<crate::acp::AcpConnection>,
    pub post_setup_notice: Option<String>,
}

impl fmt::Debug for SetupWizardCompletion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetupWizardCompletion")
            .field("post_setup_notice", &self.post_setup_notice)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum SetupWizardInputOutcome {
    Unchanged,
    Changed,
    Action(Action),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetupWizardRenderResult {
    pub cursor_pos: Option<(u16, u16)>,
}

/// Fresh users can start with no advertised auth methods after provider-neutral
/// defaults land. At cold start, that should open provider setup instead of the
/// interactive login flow.
pub fn cold_start_needs_provider_setup(
    auth_methods: &[acp::AuthMethod],
    force_login: bool,
) -> bool {
    auth_methods.is_empty() && !force_login
}

pub fn login_shim_message() -> &'static str {
    "grok login is disabled in this build. Run `grok provider` to configure an LLM, \
or add a [model.*] entry in ~/.grok/config.toml. See: grok provider --help"
}

pub fn handle_setup_wizard_input(
    ev: &Event,
    wizard: &mut SetupWizardState,
) -> SetupWizardInputOutcome {
    match ev {
        Event::Resize(_, _) => return SetupWizardInputOutcome::Changed,
        Event::Paste(text) => {
            return if wizard.push_str(&text.replace(['\n', '\r'], "")) {
                SetupWizardInputOutcome::Changed
            } else {
                SetupWizardInputOutcome::Unchanged
            };
        }
        _ => {}
    }

    let Event::Key(key) = ev else {
        return SetupWizardInputOutcome::Unchanged;
    };
    if key.kind == KeyEventKind::Release {
        return SetupWizardInputOutcome::Unchanged;
    }

    match wizard.phase() {
        SetupWizardPhase::SelectPreset => {
            if is_quit_key(key) {
                SetupWizardInputOutcome::Action(Action::Quit)
            } else {
                handle_select_preset_key(key, wizard)
            }
        }
        SetupWizardPhase::Validating => {
            if is_quit_key(key) {
                SetupWizardInputOutcome::Action(Action::Quit)
            } else {
                SetupWizardInputOutcome::Unchanged
            }
        }
        SetupWizardPhase::EditFields | SetupWizardPhase::Error(_) => {
            handle_edit_fields_key(key, wizard)
        }
    }
}

pub fn render_setup_wizard(
    area: Rect,
    buf: &mut Buffer,
    wizard: &SetupWizardState,
) -> SetupWizardRenderResult {
    let theme = Theme::current();
    Clear.render(area, buf);

    let block = Block::default()
        .title(" Provider setup ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.gray_dim));
    let inner = block.inner(area);
    block.render(area, buf);

    if inner.width < 20 || inner.height < 8 {
        return SetupWizardRenderResult::default();
    }

    let sections = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(4),
        Constraint::Length(2),
    ])
    .split(inner);

    render_header(sections[0], buf, wizard, &theme);
    let cursor_pos = match wizard.phase() {
        SetupWizardPhase::SelectPreset => {
            render_preset_picker(sections[1], buf, wizard, &theme);
            None
        }
        SetupWizardPhase::EditFields
        | SetupWizardPhase::Error(_)
        | SetupWizardPhase::Validating => render_edit_fields(sections[1], buf, wizard, &theme),
    };
    render_footer(sections[2], buf, wizard, &theme);

    SetupWizardRenderResult { cursor_pos }
}

pub fn apply_setup_wizard_success(app: &mut AppView, post_setup_notice: Option<String>) {
    app.setup_wizard = None;
    app.auth_state = AuthState::Done;
    app.recompute_usage_visibility();
    app.welcome_prompt_focused = !app.is_access_blocked();
    if let Some(message) = post_setup_notice {
        app.startup_warnings.push(crate::startup::StartupWarning {
            severity: crate::startup::WarningSeverity::Info,
            message,
            action: None,
        });
    }
}

fn handle_select_preset_key(
    key: &crossterm::event::KeyEvent,
    wizard: &mut SetupWizardState,
) -> SetupWizardInputOutcome {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            wizard.select_prev_preset();
            SetupWizardInputOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            wizard.select_next_preset();
            SetupWizardInputOutcome::Changed
        }
        KeyCode::Enter => match wizard.confirm_selected_preset() {
            Ok(()) => SetupWizardInputOutcome::Changed,
            Err(message) => {
                wizard.finish_with_error(message);
                SetupWizardInputOutcome::Changed
            }
        },
        _ => SetupWizardInputOutcome::Unchanged,
    }
}

fn handle_edit_fields_key(
    key: &crossterm::event::KeyEvent,
    wizard: &mut SetupWizardState,
) -> SetupWizardInputOutcome {
    if is_forced_quit_key(key) || (is_quit_key(key) && !is_text_entry_focus(wizard.focus())) {
        return SetupWizardInputOutcome::Action(Action::Quit);
    }
    if crate::input::key::is_shift_tab(key) {
        wizard.focus_prev();
        return SetupWizardInputOutcome::Changed;
    }
    match key.code {
        KeyCode::Esc => {
            wizard.return_to_preset_select();
            SetupWizardInputOutcome::Changed
        }
        KeyCode::Tab | KeyCode::Down => {
            wizard.focus_next();
            SetupWizardInputOutcome::Changed
        }
        KeyCode::Up => {
            wizard.focus_prev();
            SetupWizardInputOutcome::Changed
        }
        KeyCode::Backspace => {
            if wizard.backspace() {
                SetupWizardInputOutcome::Changed
            } else {
                SetupWizardInputOutcome::Unchanged
            }
        }
        KeyCode::Char(' ') => {
            if matches!(wizard.focus(), Some(EditFieldFocus::StoreKeyInConfig))
                && wizard.toggle_store_key_in_config()
            {
                SetupWizardInputOutcome::Changed
            } else if matches!(wizard.focus(), Some(EditFieldFocus::ApiBackend))
                && wizard.cycle_api_backend_next()
            {
                SetupWizardInputOutcome::Changed
            } else if wizard.insert_char(' ') {
                SetupWizardInputOutcome::Changed
            } else {
                SetupWizardInputOutcome::Unchanged
            }
        }
        KeyCode::Left => {
            if matches!(wizard.focus(), Some(EditFieldFocus::ApiBackend))
                && wizard.cycle_api_backend_prev()
            {
                SetupWizardInputOutcome::Changed
            } else {
                SetupWizardInputOutcome::Unchanged
            }
        }
        KeyCode::Right => {
            if matches!(wizard.focus(), Some(EditFieldFocus::ApiBackend))
                && wizard.cycle_api_backend_next()
            {
                SetupWizardInputOutcome::Changed
            } else {
                SetupWizardInputOutcome::Unchanged
            }
        }
        KeyCode::Enter => match wizard.focus() {
            Some(EditFieldFocus::Submit) => match wizard.build_submission() {
                Ok(submission) => {
                    wizard.start_validating();
                    SetupWizardInputOutcome::Action(Action::SubmitSetupWizard(submission))
                }
                Err(message) => {
                    wizard.finish_with_error(message);
                    SetupWizardInputOutcome::Changed
                }
            },
            Some(EditFieldFocus::Back) => {
                wizard.return_to_preset_select();
                SetupWizardInputOutcome::Changed
            }
            Some(EditFieldFocus::StoreKeyInConfig) => {
                if wizard.toggle_store_key_in_config() {
                    SetupWizardInputOutcome::Changed
                } else {
                    SetupWizardInputOutcome::Unchanged
                }
            }
            Some(EditFieldFocus::ApiBackend) => {
                if wizard.cycle_api_backend_next() {
                    SetupWizardInputOutcome::Changed
                } else {
                    SetupWizardInputOutcome::Unchanged
                }
            }
            Some(_) => {
                wizard.focus_next();
                SetupWizardInputOutcome::Changed
            }
            None => SetupWizardInputOutcome::Unchanged,
        },
        KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            if wizard.insert_char(ch) {
                SetupWizardInputOutcome::Changed
            } else {
                SetupWizardInputOutcome::Unchanged
            }
        }
        _ => SetupWizardInputOutcome::Unchanged,
    }
}

fn render_header(area: Rect, buf: &mut Buffer, wizard: &SetupWizardState, theme: &Theme) {
    let status = match wizard.phase() {
        SetupWizardPhase::SelectPreset => "1/2  Choose a provider preset",
        SetupWizardPhase::EditFields => "2/2  Enter provider details and save",
        SetupWizardPhase::Error(_) => "2/2  Fix the highlighted problem and retry",
        SetupWizardPhase::Validating => "2/2  Validating and saving provider settings",
    };
    Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Set up a provider before starting your first session.",
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(status, Style::default().fg(theme.gray))]),
    ])
    .wrap(Wrap { trim: false })
    .render(area, buf);
}

fn render_preset_picker(area: Rect, buf: &mut Buffer, wizard: &SetupWizardState, theme: &Theme) {
    let mut lines = Vec::with_capacity(wizard.presets().len() + 1);
    lines.push(Line::from(vec![Span::styled(
        "Choose a preset:",
        Style::default().fg(theme.text_primary),
    )]));
    for (idx, preset) in wizard.presets().iter().enumerate() {
        let selected = idx == wizard.preset_cursor();
        let pointer = if selected { ">" } else { " " };
        let base = if preset.base_url.is_empty() {
            "enter manually"
        } else {
            preset.base_url
        };
        let mut style = Style::default().fg(theme.text_primary);
        if selected {
            style = style.fg(theme.accent_user).add_modifier(Modifier::BOLD);
        }
        lines.push(Line::from(vec![
            Span::styled(format!("{pointer} {:<24}", preset.label), style),
            Span::styled(base, Style::default().fg(theme.gray)),
        ]));
    }
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .render(area, buf);
}

fn render_edit_fields(
    area: Rect,
    buf: &mut Buffer,
    wizard: &SetupWizardState,
    theme: &Theme,
) -> Option<(u16, u16)> {
    let fields = wizard.edit_fields()?;
    let status_line = match wizard.phase() {
        SetupWizardPhase::Validating => Some((
            "Validating endpoint and reconnecting...",
            Style::default().fg(theme.accent_user),
        )),
        SetupWizardPhase::Error(message) => {
            Some((message.as_str(), Style::default().fg(theme.accent_error)))
        }
        _ => None,
    };

    let mut lines: Vec<(String, Style, Option<(bool, usize)>)> = Vec::new();
    lines.push(field_line(
        "Preset",
        &fields.preset_label,
        false,
        None,
        theme,
    ));
    lines.push(field_line(
        "Base URL",
        &fields.base_url,
        matches!(fields.focus, EditFieldFocus::BaseUrl),
        Some((true, fields.base_url.len())),
        theme,
    ));
    if fields.shows_api_backend_selector() {
        lines.push(field_line(
            "API backend",
            &fields.api_backend,
            matches!(fields.focus, EditFieldFocus::ApiBackend),
            None,
            theme,
        ));
    }
    lines.push(field_line(
        "Model",
        &fields.model,
        matches!(fields.focus, EditFieldFocus::Model),
        Some((true, fields.model.len())),
        theme,
    ));
    lines.push(field_line(
        "Display name",
        &fields.name,
        matches!(fields.focus, EditFieldFocus::Name),
        Some((true, fields.name.len())),
        theme,
    ));
    if fields.requires_api_key() {
        let masked = if fields.api_key_input.is_empty() {
            String::new()
        } else {
            "*".repeat(fields.api_key_input.chars().count())
        };
        lines.push(field_line(
            "API key",
            &masked,
            matches!(fields.focus, EditFieldFocus::ApiKey),
            Some((true, masked.len())),
            theme,
        ));
        let checkbox = if fields.store_key_in_config {
            "[x]"
        } else {
            "[ ]"
        };
        let note = fields
            .env_key_name
            .as_deref()
            .map(|key| format!("{checkbox} Store in config.toml  (default: {key})"))
            .unwrap_or_else(|| format!("{checkbox} Store in config.toml"));
        lines.push(field_line(
            "Credential",
            &note,
            matches!(fields.focus, EditFieldFocus::StoreKeyInConfig),
            None,
            theme,
        ));
    } else {
        lines.push(field_line(
            "Credential",
            "No API key needed for this provider.",
            false,
            None,
            theme,
        ));
    }
    lines.push(button_line(
        "Validate and save",
        matches!(fields.focus, EditFieldFocus::Submit),
        theme,
    ));
    lines.push(button_line(
        "Back to presets",
        matches!(fields.focus, EditFieldFocus::Back),
        theme,
    ));

    if let Some((message, style)) = status_line {
        let status_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 2.min(area.height),
        };
        Paragraph::new(vec![Line::from(vec![Span::styled(message, style)])])
            .wrap(Wrap { trim: false })
            .render(status_area, buf);
    }

    let top_offset = if status_line.is_some() { 2 } else { 0 };
    let mut cursor = None;
    for (idx, (text, style, cursor_hint)) in lines.iter().enumerate() {
        let y = area.y.saturating_add(top_offset).saturating_add(idx as u16);
        if y >= area.y + area.height {
            break;
        }
        buf.set_line(
            area.x,
            y,
            &Line::from(text.clone()).style(*style),
            area.width,
        );
        if cursor.is_none()
            && !matches!(wizard.phase(), SetupWizardPhase::Validating)
            && let Some((active, cursor_chars)) = cursor_hint
            && *active
        {
            let max_x = area.x + area.width.saturating_sub(1);
            let cursor_x = area.x.saturating_add(text.width() as u16).min(max_x);
            let _ = cursor_chars;
            cursor = Some((cursor_x, y));
        }
    }
    cursor
}

fn render_footer(area: Rect, buf: &mut Buffer, wizard: &SetupWizardState, theme: &Theme) {
    let footer = match wizard.phase() {
        SetupWizardPhase::SelectPreset => "Up/Down choose  Enter continue  Ctrl+C quit",
        SetupWizardPhase::Validating => "Waiting for validation to finish...",
        _ => "Tab move  Enter activate  Esc presets  Ctrl+C quit",
    };
    Paragraph::new(vec![Line::from(vec![Span::styled(
        footer,
        Style::default().fg(theme.gray),
    )])])
    .wrap(Wrap { trim: false })
    .render(area, buf);
}

fn field_line(
    label: &str,
    value: &str,
    active: bool,
    cursor_hint: Option<(bool, usize)>,
    theme: &Theme,
) -> (String, Style, Option<(bool, usize)>) {
    let prefix = if active { ">" } else { " " };
    let style = if active {
        Style::default()
            .fg(theme.accent_user)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_primary)
    };
    (
        format!("{prefix} {label:<13}: {value}"),
        style,
        cursor_hint.map(|(_, pos)| (active, pos)),
    )
}

fn button_line(label: &str, active: bool, theme: &Theme) -> (String, Style, Option<(bool, usize)>) {
    let prefix = if active { ">" } else { " " };
    let style = if active {
        Style::default()
            .fg(theme.accent_user)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_primary)
    };
    (format!("{prefix} {label}"), style, None)
}

fn is_quit_key(key: &crossterm::event::KeyEvent) -> bool {
    is_plain_quit_key(key) || is_forced_quit_key(key)
}

fn is_plain_quit_key(key: &crossterm::event::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q')) && key.modifiers.is_empty()
}

fn is_forced_quit_key(key: &crossterm::event::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_text_entry_focus(focus: Option<EditFieldFocus>) -> bool {
    matches!(
        focus,
        Some(
            EditFieldFocus::BaseUrl
                | EditFieldFocus::Model
                | EditFieldFocus::Name
                | EditFieldFocus::ApiKey
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_config_write::ProviderModelWrite;
    use crossterm::event::KeyEvent;
    use std::ffi::OsString;

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn unset(key: &'static str) -> Self {
            let original = std::env::var_os(key);
            unsafe { std::env::remove_var(key) };
            Self { key, original }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var_os(key);
            unsafe { std::env::set_var(key, value) };
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                unsafe { std::env::set_var(self.key, value) };
            } else {
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }

    fn make_auth_method(id: &str, name: &str) -> acp::AuthMethod {
        acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
            acp::AuthMethodId::new(id),
            name.to_string(),
        ))
    }

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn edit_fields_wizard(preset_id: &str) -> SetupWizardState {
        let mut wizard = SetupWizardState::new();
        wizard
            .select_preset_by_id(preset_id)
            .expect("preset exists");
        wizard
            .confirm_selected_preset()
            .expect("advance to edit fields");
        wizard
    }

    #[test]
    fn empty_auth_methods_request_provider_setup_not_login() {
        assert!(cold_start_needs_provider_setup(&[], false));
    }

    #[test]
    fn advertised_auth_methods_skip_provider_setup() {
        let methods = vec![make_auth_method("grok.com", "grok.com")];
        assert!(!cold_start_needs_provider_setup(&methods, false));
    }

    #[test]
    fn force_login_disables_provider_setup_gate() {
        assert!(!cold_start_needs_provider_setup(&[], true));
    }

    #[test]
    fn login_shim_message_points_at_grok_provider_not_xai_account_login() {
        let message = login_shim_message();
        assert!(
            message.contains("grok provider"),
            "shim must direct users to provider setup: {message}"
        );
        assert!(
            !message.contains("accounts.x.ai"),
            "shim must stay provider-neutral: {message}"
        );
    }

    #[tokio::test]
    async fn validate_rejects_empty_model_id() {
        let err = validate::validate_provider_endpoint(&validate::ValidateRequest {
            base_url: "http://localhost:11434/v1".into(),
            model: "".into(),
            api_key: None,
            api_backend: "chat_completions".into(),
            auth_not_required: true,
            extra_headers: Default::default(),
        })
        .await
        .expect_err("empty model must fail validation");

        assert!(
            err.to_string().contains("model"),
            "expected model-specific error, got {err:?}"
        );
    }

    #[test]
    fn ollama_preset_prefills_edit_fields_and_write_shape() {
        let mut wizard = SetupWizardState::new();
        wizard
            .select_preset_by_id("ollama")
            .expect("ollama preset exists");
        wizard
            .confirm_selected_preset()
            .expect("advance to edit fields");
        wizard.set_model("llama3.1");

        let fields = wizard.edit_fields().expect("edit fields should be active");
        assert_eq!(fields.base_url, "http://localhost:11434/v1");
        assert_eq!(fields.api_backend, "chat_completions");
        assert_eq!(fields.env_key_name, None);
        assert!(!fields.requires_api_key());

        let write: ProviderModelWrite = wizard
            .build_provider_write()
            .expect("ollama config should be writable");
        assert_eq!(write.base_url, "http://localhost:11434/v1");
        assert_eq!(write.model, "llama3.1");
        assert!(write.auth_not_required, "ollama should skip auth");
        assert_eq!(write.env_key, None);
        assert_eq!(write.api_key, None);
    }

    #[test]
    #[serial_test::serial(SETUP_WIZARD_ENV)]
    fn typed_api_key_without_exported_env_requires_opt_in_to_persist() {
        let _env_guard = EnvVarGuard::unset("OPENAI_API_KEY");
        let mut wizard = edit_fields_wizard("openai");
        wizard.set_model("gpt-4.1");
        let fields = wizard
            .edit_fields_mut()
            .expect("edit fields should be active");
        fields.api_key_input = "sk-test-value".into();
        fields.store_key_in_config = false;

        let validate = wizard
            .build_validate_request()
            .expect("typed key should still validate in-memory");
        assert_eq!(validate.api_key.as_deref(), Some("sk-test-value"));

        let write = wizard
            .build_provider_write()
            .expect("config write should still be shapeable");
        assert_eq!(write.env_key.as_deref(), Some("OPENAI_API_KEY"));
        assert_eq!(
            write.api_key, None,
            "typed key must not be persisted unless the checkbox is enabled"
        );

        let err = wizard
            .build_submission()
            .expect_err("submit must stop before reconnect when env export is missing");
        assert!(
            err.contains("OPENAI_API_KEY"),
            "missing-export error should name the env var: {err}"
        );
        assert!(
            err.contains("Store in config.toml"),
            "missing-export error should point at the opt-in checkbox: {err}"
        );
    }

    #[test]
    #[serial_test::serial(SETUP_WIZARD_ENV)]
    fn typed_api_key_with_exported_env_can_submit_without_persisting_secret() {
        let _env_guard = EnvVarGuard::set("OPENAI_API_KEY", "env-test-value");
        let mut wizard = edit_fields_wizard("openai");
        wizard.set_model("gpt-4.1");
        let fields = wizard
            .edit_fields_mut()
            .expect("edit fields should be active");
        fields.api_key_input = "sk-test-value".into();
        fields.store_key_in_config = false;

        let submission = wizard
            .build_submission()
            .expect("exported env should allow reconnect without persisting the typed key");

        assert_eq!(submission.write.env_key.as_deref(), Some("OPENAI_API_KEY"));
        assert_eq!(submission.write.api_key, None);
        assert_eq!(submission.post_setup_notice, None);
    }

    #[test]
    fn custom_preset_allows_cycling_api_backend_and_persisting_selection() {
        let mut wizard = edit_fields_wizard("custom");
        wizard.set_model("proxy-model");
        wizard.edit_fields_mut().expect("edit fields").base_url = "https://proxy.example/v1".into();

        let outcome =
            handle_setup_wizard_input(&key_event(KeyCode::Tab, KeyModifiers::NONE), &mut wizard);
        assert!(matches!(outcome, SetupWizardInputOutcome::Changed));
        let outcome =
            handle_setup_wizard_input(&key_event(KeyCode::Enter, KeyModifiers::NONE), &mut wizard);
        assert!(matches!(outcome, SetupWizardInputOutcome::Changed));
        let outcome = handle_setup_wizard_input(
            &key_event(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut wizard,
        );
        assert!(matches!(outcome, SetupWizardInputOutcome::Changed));

        let fields = wizard
            .edit_fields()
            .expect("edit fields should remain active");
        assert_eq!(fields.api_backend, "responses");

        let write = wizard
            .build_provider_write()
            .expect("custom preset should persist the chosen backend");
        assert_eq!(write.api_backend, "responses");
    }

    #[test]
    fn q_in_model_and_api_key_fields_inserts_text_without_quitting() {
        let mut wizard = edit_fields_wizard("openai");
        wizard.edit_fields_mut().expect("edit fields").focus = EditFieldFocus::Model;

        let outcome = handle_setup_wizard_input(
            &key_event(KeyCode::Char('q'), KeyModifiers::NONE),
            &mut wizard,
        );

        assert!(matches!(outcome, SetupWizardInputOutcome::Changed));
        let fields = wizard
            .edit_fields()
            .expect("edit fields should remain active");
        assert_eq!(fields.model, "q");
        assert!(matches!(wizard.phase(), SetupWizardPhase::EditFields));

        wizard.edit_fields_mut().expect("edit fields").focus = EditFieldFocus::ApiKey;
        let outcome = handle_setup_wizard_input(
            &key_event(KeyCode::Char('q'), KeyModifiers::NONE),
            &mut wizard,
        );

        assert!(matches!(outcome, SetupWizardInputOutcome::Changed));
        let fields = wizard
            .edit_fields()
            .expect("edit fields should remain active");
        assert_eq!(fields.api_key_input, "q");
        assert!(matches!(wizard.phase(), SetupWizardPhase::EditFields));
    }

    #[test]
    fn space_in_display_name_field_inserts_space() {
        let mut wizard = edit_fields_wizard("openai");
        wizard.edit_fields_mut().expect("edit fields").focus = EditFieldFocus::Name;

        let outcome = handle_setup_wizard_input(
            &key_event(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut wizard,
        );

        assert!(matches!(outcome, SetupWizardInputOutcome::Changed));
        let fields = wizard
            .edit_fields()
            .expect("edit fields should remain active");
        assert_eq!(fields.name, "OpenAI ");
        assert!(!fields.store_key_in_config);
    }

    #[test]
    fn space_on_store_key_checkbox_toggles_value() {
        let mut wizard = edit_fields_wizard("openai");
        wizard.edit_fields_mut().expect("edit fields").focus = EditFieldFocus::StoreKeyInConfig;

        let outcome = handle_setup_wizard_input(
            &key_event(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut wizard,
        );

        assert!(matches!(outcome, SetupWizardInputOutcome::Changed));
        let fields = wizard
            .edit_fields()
            .expect("edit fields should remain active");
        assert!(fields.store_key_in_config);
    }
}
