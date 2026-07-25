# Provider-Neutral Defaults Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a fresh `grok` install provider-neutral: no default x.ai login/proxy/`grok-build` catalog, with a first-run provider wizard that configures any OpenAI/Anthropic-compatible LLM.

**Architecture:** Approach 1 from the design spec — cut over empty-state defaults and startup gates; leave OAuth/billing/managed-MCP modules in-tree but idle. Auth advertisement omits `grok.com`/`cached_token` by default; the pager opens a setup wizard instead of `Action::Login`.

**Tech Stack:** Rust (Cargo workspace), `xai-grok-shell`, `xai-grok-pager`, `xai-grok-models`, `toml_edit`, ACP auth methods, existing sampler backends (`chat_completions` / `messages` / `responses`).

**Spec:** `docs/superpowers/specs/2026-07-25-provider-neutral-defaults-design.md`

## Global Constraints

- Keep CLI name `grok`, home dir `~/.grok/`, and crate names `xai-grok-*` (behavior-only fork).
- Do **not** delete OAuth / AuthManager / billing / managed-MCP source modules.
- Do **not** open accounts.x.ai / grok.com browser login on cold start.
- Prefer localized seam changes for easy upstream merges.
- CLI for the wizard is `grok provider` (not `grok setup` — that already means managed team config).
- Slash re-entry is `/provider`.
- Method id `xai.api_key` is kept for merge ease even though it means “API key / BYOK”.
- TDD: write/adjust failing tests before production changes; commit after each task.
- Always run targeted crate tests: `cargo test -p <crate> <filter>` (never whole workspace).

---

## File structure

| Path | Responsibility |
|---|---|
| `crates/codegen/xai-grok-models/default_models.json` | Empty baked-in catalog |
| `crates/codegen/xai-grok-models/src/lib.rs` | Allow empty default; `default_model()` may be `""` |
| `crates/codegen/xai-grok-shell/src/agent/config.rs` | No implicit proxy; `auth_not_required` on model entries; credential helpers |
| `crates/codegen/xai-grok-shell/src/agent/auth_method.rs` | Unpinned path: no `grok.com`, no `cached_token` |
| `crates/codegen/xai-grok-shell/src/agent/mvp_agent/acp_agent.rs` | Pass flags consistent with fork auth rules |
| `crates/codegen/xai-grok-pager/src/provider_presets.rs` | Preset table (URLs, backends, env keys) |
| `crates/codegen/xai-grok-pager/src/provider_config_write.rs` | Merge-write `[auth]` / `[models]` / `[model.*]` via `toml_edit` |
| `crates/codegen/xai-grok-pager/src/setup_wizard/` | Wizard state machine + validation + render |
| `crates/codegen/xai-grok-pager/src/app/event_loop.rs` | Cold-start → wizard instead of `Action::Login` |
| `crates/codegen/xai-grok-pager/src/app/actions.rs` | `Action::SetupWizard` (or equiv.) |
| `crates/codegen/xai-grok-pager/src/app/cli.rs` + `pager-bin/src/main.rs` | `Command::Provider`; shim `Command::Login` |
| `crates/codegen/xai-grok-pager/src/slash/commands/` | `/provider` command |
| `crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md` (+ shell README auth section) | First-run docs |

---

### Task 1: Empty baked-in model catalog

**Files:**
- Modify: `crates/codegen/xai-grok-models/default_models.json`
- Modify: `crates/codegen/xai-grok-models/src/lib.rs`
- Test: `crates/codegen/xai-grok-models/src/lib.rs` (add `#[cfg(test)]` module) and any shell tests that assume `grok-build` in defaults

**Interfaces:**
- Consumes: none
- Produces: `default_model() -> &'static str` returns `""` when catalog empty; `DEFAULT_MODELS_JSON` embeds empty catalog; `default_models()` in shell yields empty `IndexMap`

- [ ] **Step 1: Write the failing test**

Add to `crates/codegen/xai-grok-models/src/lib.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p xai-grok-models embedded_default_model_is_blank -- --nocapture`  
Expected: FAIL (`default_model()` still `"grok-build"`) or panic on assert if JSON emptied first without relaxing loader.

- [ ] **Step 3: Implement empty catalog + relax loader**

Replace `default_models.json` with:

```json
{
  "default": "",
  "web_search": "",
  "image_description": "",
  "session_summary": "",
  "models": []
}
```

Update `LazyLock` in `lib.rs` so blank default is legal when `models` is empty:

```rust
static DEFAULTS: LazyLock<DefaultModels> = LazyLock::new(|| {
    let defaults: DefaultModels = serde_json::from_str(DEFAULT_MODELS_JSON)
        .expect("default_models.json: invalid JSON or missing 'default' field");

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
```

Document on `default_model()`: empty string means “no baked-in default; config/wizard must supply one.”

- [ ] **Step 4: Run tests**

Run: `cargo test -p xai-grok-models`  
Expected: PASS. Then `cargo test -p xai-grok-shell default_models -- --nocapture` and fix any tests that hard-require embedded `grok-build` (update expectations to empty catalog / config-supplied models only).

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-models crates/codegen/xai-grok-shell
git commit -m "feat: clear baked-in grok-build default model catalog"
```

---

### Task 2: Remove implicit cli-chat-proxy default URL

**Files:**
- Modify: `crates/codegen/xai-grok-shell/src/agent/config.rs` (`CLI_CHAT_PROXY_BASE_URL_DEFAULT`, `EndpointsConfig::proxy_url`, callers that must no-op when empty)
- Modify: `crates/codegen/xai-grok-pager/src/app/effects/mod.rs` (settings fetch fallback)
- Test: existing `aux_endpoints_resolve_to_proxy_*` tests in `config.rs` — rewrite expectations

**Interfaces:**
- Consumes: Task 1 empty catalog
- Produces: `EndpointsConfig::proxy_url() -> String` returns `""` when unset; `proxy_url_configured() -> bool` helper; inference requires explicit `models_base_url` or per-model `base_url`

- [ ] **Step 1: Write the failing test**

In `config.rs` tests module, add:

```rust
#[test]
fn proxy_url_has_no_hardcoded_default() {
    let endpoints = EndpointsConfig::default();
    assert_eq!(endpoints.proxy_url(), "");
    assert!(!endpoints.proxy_url_configured());
}

#[test]
fn resolve_inference_base_url_empty_without_models_base_url() {
    let endpoints = EndpointsConfig::default();
    assert_eq!(endpoints.resolve_inference_base_url(), "");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p xai-grok-shell proxy_url_has_no_hardcoded_default -- --nocapture`  
Expected: FAIL (still returns `https://cli-chat-proxy.grok.com/v1`).

- [ ] **Step 3: Minimal implementation**

Keep the constant for explicit opt-in / docs, but stop using it as fallback:

```rust
pub const CLI_CHAT_PROXY_BASE_URL_DEFAULT: &str = "https://cli-chat-proxy.grok.com/v1";

impl EndpointsConfig {
    pub fn proxy_url_configured(&self) -> bool {
        blank_as_unset(&self.cli_chat_proxy_base_url).is_some()
    }

    pub fn proxy_url(&self) -> String {
        blank_as_unset(&self.cli_chat_proxy_base_url)
            .unwrap_or_default()
    }
}
```

Add a short comment: fork default — no implicit cli-chat-proxy; set `GROK_CLI_CHAT_PROXY_BASE_URL` or `[endpoints] cli_chat_proxy_base_url` to re-enable.

Update pager settings fetch (`effects/mod.rs` ~3917) to skip network when proxy URL is empty (no fallback to `CLI_CHAT_PROXY_BASE_URL_DEFAULT`).

Update shell tests that asserted the old default URL to expect `""` / skipped fetch instead.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p xai-grok-shell proxy_url_has_no_hardcoded_default
cargo test -p xai-grok-shell aux_endpoints_resolve_to_proxy
cargo test -p xai-grok-shell loader_managed_config_url
```

Expected: PASS after expectation updates.

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-shell/src/agent/config.rs crates/codegen/xai-grok-pager/src/app/effects/mod.rs
git commit -m "feat: remove implicit cli-chat-proxy default URL"
```

---

### Task 3: Stop advertising grok.com / cached_token by default

**Files:**
- Modify: `crates/codegen/xai-grok-shell/src/agent/auth_method.rs` (`build_unpinned`)
- Modify tests in the same file (`fresh_user_*`, `session_only_*`, `no_legacy_token_*`, `auth_provider_command_*`, etc.)
- Modify: `crates/codegen/xai-grok-pager/src/acp/mod.rs` tests that assume fresh user → `needs_login`

**Interfaces:**
- Consumes: none (auth list only)
- Produces: `build_unpinned` returns only `xai.api_key` when `has_external_api_key`, else empty methods; no `grok.com`, no `cached_token` in unpinned path

- [ ] **Step 1: Rewrite the failing/updated contract test**

Replace `fresh_user_only_advertises_grok_com_and_requires_login` with:

```rust
#[test]
fn fresh_user_advertises_no_interactive_login() {
    let built = build_auth_methods(default_inputs());
    assert!(built.methods.is_empty(), "expected no auth methods, got {:?}", built.methods);
    assert!(built.default_auth_method_id.is_none());
    assert!(
        !built
            .methods
            .iter()
            .any(|m| AuthMethodKind::from_id(m.id()).needs_interactive_login())
    );
}
```

Add:

```rust
#[test]
fn unpinned_ignores_cached_token_flag() {
    let built = build_auth_methods(AuthMethodsBuildInputs {
        has_cached_token: true,
        ..default_inputs()
    });
    assert!(
        !built
            .methods
            .iter()
            .any(|m| AuthMethodKind::from_id(m.id()) == AuthMethodKind::CachedToken)
    );
    assert!(
        !built
            .methods
            .iter()
            .any(|m| AuthMethodKind::from_id(m.id()) == AuthMethodKind::GrokCom)
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p xai-grok-shell fresh_user_advertises_no_interactive_login -- --nocapture`  
Expected: FAIL (still advertises `grok.com`).

- [ ] **Step 3: Change `build_unpinned`**

```rust
fn build_unpinned(
    has_external_api_key: bool,
    _has_cached_token: bool,
    _has_enterprise_oidc: bool,
    _enterprise_oidc_issuer: Option<&str>,
    _login_label: Option<&str>,
    _has_auth_provider_command: bool,
) -> BuiltAuthMethods {
    // Fork default: provider-neutral empty state. Do not advertise grok.com,
    // OIDC, or legacy cached_token here — pager runs the provider wizard instead.
    // preferred_method=oidc / api_key pins still use the pinned builders.
    if has_external_api_key {
        BuiltAuthMethods {
            methods: vec![xai_api_key_auth_method()],
            default_auth_method_id: Some(acp::AuthMethodId::new(XAI_API_KEY_METHOD_ID)),
        }
    } else {
        BuiltAuthMethods {
            methods: Vec::new(),
            default_auth_method_id: None,
        }
    }
}
```

Leave `push_interactive_login` and `build_pinned_oidc` intact for upstream merge / explicit `preferred_method = "oidc"`.

Update other unpinned tests that expected `grok.com` or `cached_token` first.

Update pager test expectations for `startup_auth_*` / BYOK still skip login when `xai.api_key` is first.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p xai-grok-shell auth_method
cargo test -p xai-grok-pager startup_auth
cargo test -p xai-grok-pager shell_built_auth_methods_for_byok
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-shell/src/agent/auth_method.rs crates/codegen/xai-grok-pager/src/acp/mod.rs
git commit -m "feat: omit grok.com and cached_token from default auth methods"
```

---

### Task 4: `auth_not_required` for keyless local models (Ollama)

**Files:**
- Modify: `crates/codegen/xai-grok-shell/src/agent/config.rs` (`ModelEntryConfig`, `ConfigModelOverride`, `ModelEntry`, `has_own_credentials`, override apply)
- Modify: `crates/codegen/xai-grok-shell/src/agent/auth_method.rs` (`should_advertise_xai_api_key` stays on `has_own_credentials`)
- Test: `has_own_credentials_*` in `config.rs`

**Interfaces:**
- Consumes: Task 3 auth advertisement
- Produces: `ModelEntry.auth_not_required: bool` (default false); `has_own_credentials()` true when `auth_not_required` **or** resolvable key; wizard can mark Ollama ready without a secret

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn auth_not_required_counts_as_own_credentials() {
    let mut entry = ModelEntry::fallback("llama3", &EndpointsConfig::default());
    entry.info.base_url = "http://localhost:11434/v1".into();
    entry.auth_not_required = true;
    assert!(entry.has_own_credentials());
    assert!(should_advertise_xai_api_key(false, std::iter::once(&entry)));
}
```

(Import `should_advertise_xai_api_key` in the test module or keep the assert local to `has_own_credentials` and cover advertise in `auth_method` tests.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p xai-grok-shell auth_not_required_counts_as_own_credentials -- --nocapture`  
Expected: FAIL (field missing / false).

- [ ] **Step 3: Minimal implementation**

Add to `ModelEntryConfig`, `ConfigModelOverride`, and `ModelEntry`:

```rust
/// When true, the model is usable without api_key/env_key (e.g. local Ollama).
#[serde(default, skip_serializing_if = "is_false")]
pub auth_not_required: bool,
```

Wire through `from_config_entry`, `ConfigModelOverride::apply`, and:

```rust
pub fn has_own_credentials(&self) -> bool {
    self.auth_not_required || self.own_credential().is_some()
}
```

Ensure `resolve_credentials` still works with no key (Bearer omitted or empty — sampler already supports open endpoints; if not, send no `Authorization` when `auth_not_required && own_credential().is_none()`).

- [ ] **Step 4: Run tests**

Run: `cargo test -p xai-grok-shell auth_not_required_counts_as_own_credentials has_own_credentials`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-shell/src/agent/config.rs crates/codegen/xai-grok-shell/src/agent/auth_method.rs
git commit -m "feat: support auth_not_required for keyless local models"
```

---

### Task 5: Provider presets + config.toml writer

**Files:**
- Create: `crates/codegen/xai-grok-pager/src/provider_presets.rs`
- Create: `crates/codegen/xai-grok-pager/src/provider_config_write.rs`
- Modify: `crates/codegen/xai-grok-pager/src/lib.rs` (register modules)
- Reuse patterns from: `crates/codegen/xai-grok-pager/src/config_toml_edit.rs`

**Interfaces:**
- Consumes: Task 4 `auth_not_required` field name
- Produces:
  - `ProviderPreset { id, label, base_url, api_backend, default_env_key, auth_not_required, extra_headers }`
  - `fn all_presets() -> &'static [ProviderPreset]`
  - `pub struct ProviderModelWrite { pub catalog_id: String, pub model: String, pub name: String, pub base_url: String, pub api_backend: String, pub env_key: Option<String>, pub api_key: Option<String>, pub auth_not_required: bool, pub extra_headers: IndexMap<String, String> }`
  - `pub fn write_provider_model_config(path: &Path, write: &ProviderModelWrite) -> std::io::Result<()>`

- [ ] **Step 1: Write failing tests for writer + presets**

In `provider_config_write.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_auth_models_and_model_table() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_provider_model_config(
            &path,
            &ProviderModelWrite {
                catalog_id: "openai-gpt-4o".into(),
                model: "gpt-4o".into(),
                name: "GPT-4o".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_backend: "chat_completions".into(),
                env_key: Some("OPENAI_API_KEY".into()),
                api_key: None,
                auth_not_required: false,
                extra_headers: Default::default(),
            },
        )
        .unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("preferred_method") && body.contains("api_key"));
        assert!(body.contains("[models]"));
        assert!(body.contains("default = \"openai-gpt-4o\""));
        assert!(body.contains("[model.openai-gpt-4o]"));
        assert!(body.contains("base_url = \"https://api.openai.com/v1\""));
        assert!(body.contains("env_key = \"OPENAI_API_KEY\""));
    }

    #[test]
    fn merge_preserves_unrelated_tables() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[ui]\nvim_mode = true\n").unwrap();
        write_provider_model_config(
            &path,
            &ProviderModelWrite {
                catalog_id: "ollama-llama3".into(),
                model: "llama3".into(),
                name: "Llama 3".into(),
                base_url: "http://localhost:11434/v1".into(),
                api_backend: "chat_completions".into(),
                env_key: None,
                api_key: None,
                auth_not_required: true,
                extra_headers: Default::default(),
            },
        )
        .unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("vim_mode = true"));
        assert!(body.contains("auth_not_required = true"));
    }
}
```

In `provider_presets.rs` test that `all_presets()` includes ids: `openai`, `anthropic`, `ollama`, `openrouter`, `groq`, `together`, `deepseek`, `custom`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p xai-grok-pager writes_auth_models_and_model_table -- --nocapture`  
Expected: FAIL (module missing).

- [ ] **Step 3: Implement presets + writer**

Preset defaults (adjust only if a provider’s public docs differ at impl time):

| id | base_url | api_backend | env_key | notes |
|---|---|---|---|---|
| openai | `https://api.openai.com/v1` | chat_completions | OPENAI_API_KEY | |
| anthropic | `https://api.anthropic.com/v1` | messages | ANTHROPIC_API_KEY | also set `extra_headers` anthropic-version; auth via `x-api-key` header in wizard write |
| ollama | `http://localhost:11434/v1` | chat_completions | — | `auth_not_required = true` |
| openrouter | `https://openrouter.ai/api/v1` | chat_completions | OPENROUTER_API_KEY | |
| groq | `https://api.groq.com/openai/v1` | chat_completions | GROQ_API_KEY | |
| together | `https://api.together.xyz/v1` | chat_completions | TOGETHER_API_KEY | |
| deepseek | `https://api.deepseek.com/v1` | chat_completions | DEEPSEEK_API_KEY | |
| custom | (user) | user-selected | user | |

Writer algorithm (mirror `config_toml_edit::read_config_document_for_edit`):

1. `create_dir_all` parent
2. Parse or new `DocumentMut`; if unparseable non-empty → return error (do not clobber)
3. Set `doc["auth"]["preferred_method"] = "api_key"`
4. Set `doc["models"]["default"] = catalog_id`
5. Set `doc["model"][catalog_id]` fields (`model`, `base_url`, `name`, `api_backend`, optional `env_key`/`api_key`/`auth_not_required`/`extra_headers`)
6. Atomic write (temp file + rename), matching shell `atomic_write_string` if accessible; otherwise `std::fs::write` acceptable for v1 with a TODO comment only if atomic helper is awkward to call — prefer calling shell’s atomic helper when public.

Register modules in `lib.rs`:

```rust
pub mod provider_presets;
pub mod provider_config_write;
pub mod setup_wizard; // stub mod.rs ok until Task 6/7
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p xai-grok-pager provider_config_write provider_presets`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-pager/src/provider_presets.rs \
  crates/codegen/xai-grok-pager/src/provider_config_write.rs \
  crates/codegen/xai-grok-pager/src/lib.rs \
  crates/codegen/xai-grok-pager/src/setup_wizard
git commit -m "feat: add provider presets and config.toml writer"
```

---

### Task 6: Cold-start gate opens setup wizard (not Login)

**Files:**
- Modify: `crates/codegen/xai-grok-pager/src/app/actions.rs` — add `Action::OpenSetupWizard`
- Modify: `crates/codegen/xai-grok-pager/src/app/event_loop.rs` (~635–707)
- Modify: `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs` — route new action
- Modify: `crates/codegen/xai-grok-pager/src/app/app_view.rs` — `setup_wizard: Option<SetupWizardState>` field (state type from Task 7; use minimal stub enum here if needed)
- Test: unit test around a pure helper extracted from event_loop logic

**Interfaces:**
- Consumes: Task 3 empty auth methods; Task 5 writer available later
- Produces: `fn cold_start_needs_provider_setup(auth_methods: &[AuthMethod], force_login: bool) -> bool` — true when methods empty and not force-login-with-methods; event_loop dispatches `OpenSetupWizard` instead of `Action::Login`

- [ ] **Step 1: Write the failing test**

Prefer a pure helper in `acp/mod.rs` or `setup_wizard/mod.rs`:

```rust
#[test]
fn empty_auth_methods_request_provider_setup_not_login() {
    assert!(cold_start_needs_provider_setup(&[], false));
    assert!(!cold_start_needs_provider_setup(
        &[/* mock xai.api_key method */],
        false
    ));
}
```

(Construct `acp::AuthMethod` the same way existing `startup_auth_*` tests do.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p xai-grok-pager empty_auth_methods_request_provider_setup_not_login -- --nocapture`  
Expected: FAIL.

- [ ] **Step 3: Wire event_loop**

Replace the auto-login branch conceptually with:

```rust
let needs_provider_setup =
    crate::setup_wizard::cold_start_needs_provider_setup(&connection.auth_methods, force_login);

let mut post_render_effects = if needs_provider_setup {
    app.welcome_prompt_focused = false;
    app.setup_wizard = Some(crate::setup_wizard::SetupWizardState::new());
    dispatch::dispatch(Action::OpenSetupWizard, &mut app)
} else if needs_interactive_login {
    // Retain for preferred_method=oidc pin / explicit interactive methods only.
    if connection.auth_methods.is_empty() {
        app.auth_state = AuthState::Pending {
            error: Some(/* clear message pointing at `grok provider` */.into()),
        };
        vec![]
    } else {
        dispatch::dispatch(Action::Login, &mut app)
    }
} else {
    vec![]
};
```

Important: with Task 3, unpinned fresh users have **empty** methods and `needs_login == false` from `startup_auth_metadata`. Do **not** rely on `needs_login`; key off empty methods / helper.

Update empty-methods fail-closed copy from `PREFERRED_API_KEY_UNAVAILABLE` to a provider-setup message when appropriate.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p xai-grok-pager empty_auth_methods_request_provider_setup
cargo test -p xai-grok-pager startup_auth
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-pager/src/app crates/codegen/xai-grok-pager/src/setup_wizard crates/codegen/xai-grok-pager/src/acp/mod.rs
git commit -m "feat: open provider setup wizard on cold start"
```

---

### Task 7: Setup wizard UI, validation, persist, re-enter TUI

**Files:**
- Create: `crates/codegen/xai-grok-pager/src/setup_wizard/mod.rs`
- Create: `crates/codegen/xai-grok-pager/src/setup_wizard/state.rs`
- Create: `crates/codegen/xai-grok-pager/src/setup_wizard/validate.rs`
- Modify: pager render/input path to show wizard when `app.setup_wizard.is_some()`
- Modify: dispatch handler for `Action::OpenSetupWizard` / wizard key events

**Interfaces:**
- Consumes: `provider_presets::all_presets`, `provider_config_write::write_provider_model_config`, `SetupWizardState`
- Produces: completed wizard clears `app.setup_wizard`, writes config, sets env if needed, triggers ACP re-init / session ready with `xai.api_key` advertised

**Wizard steps (state machine):**
1. `SelectPreset`
2. `EditFields { preset_id, base_url, model, name, api_key_input, store_key_in_config, api_backend }`
3. `Validating`
4. `Error { message }` (return to EditFields)
5. `Done` (persist + exit)

- [ ] **Step 1: Write failing validation + state tests**

```rust
#[tokio::test]
async fn validate_rejects_empty_model_id() {
    let err = validate_provider_endpoint(&ValidateRequest {
        base_url: "http://localhost:11434/v1".into(),
        model: "".into(),
        api_key: None,
        api_backend: "chat_completions".into(),
        extra_headers: Default::default(),
    })
    .await
    .expect_err("empty model");
    assert!(err.to_string().contains("model"));
}
```

Add a state test: selecting `ollama` prefills base_url and sets `auth_not_required` on the eventual write struct.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p xai-grok-pager validate_rejects_empty_model_id -- --nocapture`  
Expected: FAIL.

- [ ] **Step 3: Implement wizard**

`validate_provider_endpoint`:
1. Reject empty `base_url` / `model`
2. `GET {base_url.trim_end_matches('/')}/models` with Bearer or anthropic headers when key present; for `auth_not_required`, omit auth
3. On non-success, return human-readable error (status + body snippet)
4. Optional: if `/models` 404, try a 1-token chat/completions or messages probe
5. Timeout ~10s

On success persist via `write_provider_model_config` to `grok_home().join("config.toml")`.

Credential policy:
- Default: write `env_key`; if key was entered and env var unset, offer to export in-process with `std::env::set_var` for the current run **and** instruct user to export persistently
- Opt-in checkbox: write `api_key` into config.toml

After persist:
- Clear wizard state
- Re-run connection/auth initialize path used after login success (reuse existing post-login session start; do **not** call grok.com login)
- Ensure mid-session 401 on custom providers does not dispatch `Action::Login` for `grok.com` (guard in re-auth handler: if no interactive method advertised, show provider error toast only)

Render: full-screen simple form in TUI (list presets, then fields). Keep visuals minimal; no SuperGrok CTAs on this surface.

- [ ] **Step 4: Run tests**

Run: `cargo test -p xai-grok-pager setup_wizard provider_config_write`  
Expected: PASS. Manually smoke with `cargo run -p xai-grok-pager-bin` in a temp `HOME` when possible.

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-pager/src/setup_wizard crates/codegen/xai-grok-pager/src/app
git commit -m "feat: implement provider setup wizard UI and validation"
```

---

### Task 8: CLI `grok provider`, login shim, `/provider` slash

**Files:**
- Modify: `crates/codegen/xai-grok-pager/src/app/cli.rs` — add `Command::Provider`
- Modify: `crates/codegen/xai-grok-pager-bin/src/main.rs` — dispatch Provider; shim Login
- Create: `crates/codegen/xai-grok-pager/src/slash/commands/provider.rs`
- Register slash command in the slash command table (same pattern as `login.rs`)
- Modify headless path error when no model credentials (point at `grok provider`)

**Interfaces:**
- Consumes: wizard entrypoint from Task 7 (shared function `run_provider_setup(app)` / headless non-TUI variant)
- Produces: `grok provider` launches wizard; `grok login` prints shim and exits 1; `/provider` opens wizard

- [ ] **Step 1: Write failing CLI/unit tests where practical**

For login shim, extract:

```rust
pub fn login_shim_message() -> &'static str {
    "grok login is disabled in this build. Run `grok provider` to configure an LLM, \
or add a [model.*] entry in ~/.grok/config.toml. See: grok provider --help"
}
```

Test the string contains `grok provider` and does not mention accounts.x.ai as required.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p xai-grok-pager login_shim_message -- --nocapture`  
Expected: FAIL until helper exists.

- [ ] **Step 3: Implement**

`Command::Provider` in clap (about: “Configure an LLM provider interactively”).

In `main.rs` Login arm, replace `run_cli_login` with:

```rust
eprintln!("{}", xai_grok_pager::setup_wizard::login_shim_message());
xai_grok_shell::instrumentation::finalize_and_exit(1);
```

Provider arm: launch TUI with a flag/env that forces wizard open, **or** run a dedicated wizard-only event loop. Prefer env `GROK_FORCE_PROVIDER_SETUP=1` read by event_loop to avoid large CLI plumbing — document it.

Headless (`grok -p`) with no credentials: exit with the same guidance (no wizard in pure headless unless stdin is a TTY and you choose to support it in v1 — **v1: fail with message only**).

- [ ] **Step 4: Run tests**

Run: `cargo test -p xai-grok-pager login_shim_message`  
`cargo check -p xai-grok-pager-bin`  
Expected: PASS / clean check.

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-pager crates/codegen/xai-grok-pager-bin
git commit -m "feat: add grok provider CLI and disable grok login shim"
```

---

### Task 9: Idle first-party UX + docs

**Files:**
- Modify: `crates/codegen/xai-grok-pager/src/app/app_view.rs` / `dispatch/billing.rs` — ensure SuperGrok CTAs hidden when `is_api_key_auth` or no first-party session (strengthen cold-start before wizard completion: treat as API-key/neutral, not free-tier x.ai)
- Modify: `crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md`
- Modify: `crates/codegen/xai-grok-shell/README.md` Authentication section (first-run → `grok provider`)
- Modify: root `README.md` briefly if it tells users to log in with grok.com first

**Interfaces:**
- Consumes: Tasks 1–8 behavior
- Produces: docs match fork behavior; billing upsells not shown without first-party auth

- [ ] **Step 1: Add a focused billing-gate test**

```rust
#[test]
fn cold_start_without_auth_meta_hides_usage_upsell() {
    // Build AppView with auth_methods empty / is_api_key_auth false / no team
    // Assert usage_visible == false when setup_wizard active OR no first-party session.
}
```

Align with existing `usage_visible` logic: for this fork, also require first-party session before showing upgrade CTAs (not merely `!is_api_key_auth`).

- [ ] **Step 2: Run test to verify it fails if gate still shows upsell**

Run: `cargo test -p xai-grok-pager cold_start_without_auth_meta_hides_usage_upsell -- --nocapture`

- [ ] **Step 3: Implement gate + docs**

Docs updates (concrete):
- Replace “Authenticate with `grok login`” cold-start guidance with `grok provider` / config.toml examples
- Note `grok setup` remains **managed team config**, unrelated to provider wizard
- Keep custom model sections; add “First run” section pointing at presets

- [ ] **Step 4: Run verification suite**

```bash
cargo test -p xai-grok-models
cargo test -p xai-grok-shell auth_method
cargo test -p xai-grok-shell proxy_url_has_no_hardcoded_default
cargo test -p xai-grok-shell auth_not_required
cargo test -p xai-grok-pager startup_auth
cargo test -p xai-grok-pager provider_
cargo test -p xai-grok-pager setup_wizard
cargo test -p xai-grok-pager login_shim
cargo check -p xai-grok-pager-bin
```

Expected: all PASS / clean check.

- [ ] **Step 5: Commit**

```bash
git add crates/codegen/xai-grok-pager crates/codegen/xai-grok-shell/README.md README.md
git commit -m "docs: document provider-neutral first run; idle SuperGrok CTAs"
```

---

## Spec coverage checklist

| Spec requirement | Task |
|---|---|
| Empty `grok-build` catalog | Task 1 |
| No default cli-chat-proxy URL | Task 2 |
| Never advertise `grok.com` by default | Task 3 |
| Ignore legacy cached x.ai token for cold start | Task 3 |
| Ollama / keyless local models | Task 4 |
| Presets (OpenAI…DeepSeek+custom) | Task 5 |
| Config writer `[auth]`/`[models]`/`[model.*]` | Task 5 |
| Wizard instead of Login | Tasks 6–7 |
| Validate endpoint before persist | Task 7 |
| No grok.com fallback on provider 401 | Task 7 |
| `grok provider` + login shim + `/provider` | Task 8 |
| Headless clear error | Task 8 |
| Hide billing/upsells when not first-party | Task 9 |
| Docs | Task 9 |
| Keep crates/CLI names; don’t delete OAuth modules | Global / all tasks |

**Naming delta vs design wording:** design said `grok setup` / `/setup`; plan uses `grok provider` / `/provider` because `Command::Setup` already means managed configuration.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-25-provider-neutral-defaults.md`. Two execution options:

**1. Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
