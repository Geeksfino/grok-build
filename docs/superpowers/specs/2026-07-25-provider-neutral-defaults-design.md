# Provider-Neutral Defaults (Approach 1)

**Date:** 2026-07-25  
**Status:** Approved for implementation planning  
**Repo:** Geeksfino/grok-build (fork of grok-build)  
**Approach:** Default cutover + dead-code stubs (upstream-merge friendly)

## Problem

A fresh `grok` install defaults to interactive `grok.com` / accounts.x.ai login and the embedded `grok-build` model served via `cli-chat-proxy.grok.com`. That ties the product to fixed xAI identity and models even though the runtime already supports OpenAI-compatible, Anthropic Messages, and Responses backends via `[model.*]` config.

## Goals

1. Remove **default** x.ai login so users are not forced through grok.com OAuth.
2. Support any desirable LLM through existing backends (`chat_completions`, `messages`, `responses`).
3. Provide an interactive **first-run setup wizard** with broad provider presets.
4. Keep CLI name `grok`, home dir `~/.grok/`, and crate names unchanged (behavior-only fork).
5. Minimize divergence from upstream so merges stay cheap.

## Non-goals

- Deleting OAuth / AuthManager / billing / managed-MCP source modules
- Renaming crates, binary, or `~/.grok`
- Making SuperGrok billing or grok.com-managed MCP work with third parties
- Rewriting the credential/session subsystem (Approach 3)
- Hard-deleting first-party surfaces (Approach 2)

## Decisions

| Decision | Choice |
|---|---|
| Outcome | Full provider-neutral empty state (option C), implemented via Approach 1 |
| x.ai first-party path | Completely idle by default — not advertised, not required |
| First launch | Interactive setup wizard (not fail-closed CLI-only, not deferred-until-prompt) |
| Wizard presets | OpenAI, Anthropic, Ollama, OpenRouter, Groq, Together, DeepSeek, Custom OpenAI-compatible |
| Branding | Keep `grok` / `~/.grok` / `xai-grok-*` crate names |
| Upstream strategy | Prefer changing defaults/gates over deleting modules |

## Architecture

Change the empty-state defaults and startup gate. Leave first-party code compiled but unreachable unless deliberately re-enabled later.

```text
┌─────────────────────────────────────────────────────────┐
│  cold start (no usable model credentials)               │
│                                                         │
│  pager: Setup Wizard (presets → write ~/.grok/config)   │
│           │                                             │
│           ▼                                             │
│  shell: auth_methods = [api_key / BYOK only]            │
│         (grok.com NOT advertised by default)            │
│           │                                             │
│           ▼                                             │
│  existing sampler + [model.*] backends                  │
│  (chat_completions / messages / responses)              │
└─────────────────────────────────────────────────────────┘

x.ai OAuth / cli-chat-proxy / billing / managed MCP
  → remain in tree; defaults + gates keep them idle
```

### Merge-friendly seams

1. `crates/codegen/xai-grok-models/default_models.json` — clear embedded `grok-build` catalog
2. Default inference base (`CLI_CHAT_PROXY_BASE_URL_DEFAULT`) — no implicit cli-chat-proxy URL
3. `build_auth_methods` in `xai-grok-shell` — do not append `grok.com` on the unpinned default path
4. Pager startup (`startup_auth_metadata` / event loop) — run setup wizard instead of `Action::Login`
5. New small provider-preset table + wizard UI module in the pager
6. Thin config writer that merges `[auth]`, `[models]`, and `[model.<id>]` into `~/.grok/config.toml`

## First-run setup wizard

### Trigger

Pager startup finds no usable **model** credentials:

- no `XAI_API_KEY` / `GROK_CODE_XAI_API_KEY`
- no resolvable `[model.*] api_key` / `env_key`
- and no wizard-completed / config-defined model that marks the install as set up (e.g. Ollama with local `base_url` and no key)

Cached `~/.grok/auth.json` x.ai / grok.com session tokens do **not** count as cold-start credentials and must not skip the wizard or re-enable interactive `grok.com` login.

Then open the wizard instead of starting grok.com login.

### Flow

1. **Pick provider preset** — OpenAI, Anthropic, Ollama, OpenRouter, Groq, Together, DeepSeek, Custom OpenAI-compatible
2. **Fill fields** — prefilled `base_url`, `api_backend`, and header style; user enters API key (optional for Ollama), model id, display name
3. **Validate** — lightweight `GET {base_url}/models` or a tiny chat ping; on failure show an inline error and stay in the wizard
4. **Persist** — write/merge `~/.grok/config.toml` (see shape below)
5. **Enter normal TUI** — re-init ACP auth with API-key/BYOK first; no interactive `grok.com` method advertised

### Config shape written by wizard

```toml
[auth]
preferred_method = "api_key"

[models]
default = "<chosen-id>"

[model.<chosen-id>]
model = "..."
base_url = "..."
api_backend = "chat_completions"  # or "messages" / "responses"
name = "..."
# Prefer env_key; api_key only if user opts into storing the secret in config
env_key = "OPENAI_API_KEY"
```

### Credential storage

- Default: `env_key` (prompt user which env var to use / export)
- Opt-in: store `api_key` in `config.toml` with an explicit warning
- Wizard never contacts accounts.x.ai or cli-chat-proxy

### Re-entry

- `grok setup` CLI subcommand and/or `/setup` slash command re-runs the same flow to add or switch providers

### Placement

- New pager module (e.g. `setup_wizard`) + thin shell/config merge helper
- Do not rewrite `auth/flow.rs` OAuth implementation

## Provider presets

| Preset | Default base URL | api_backend | Auth style |
|---|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `chat_completions` | Bearer via `env_key` / `api_key` |
| Anthropic | `https://api.anthropic.com/v1` | `messages` | `extra_headers` (`x-api-key`, `anthropic-version`) |
| Ollama | `http://localhost:11434/v1` | `chat_completions` | No key required |
| OpenRouter | `https://openrouter.ai/api/v1` | `chat_completions` | Bearer |
| Groq | `https://api.groq.com/openai/v1` | `chat_completions` | Bearer |
| Together | `https://api.together.xyz/v1` | `chat_completions` | Bearer |
| DeepSeek | `https://api.deepseek.com/v1` | `chat_completions` | Bearer |
| Custom OpenAI-compatible | user-supplied | user-selected | Bearer (default) |

Exact URLs may be adjusted to match each provider’s current OpenAI-compatible docs at implementation time; the table is the product contract, not a frozen constant file forever.

## What we disable (without deleting)

| Surface | New empty-state behavior | Code stays in tree |
|---|---|---|
| `grok.com` / accounts.x.ai OAuth | Not advertised; wizard replaces login | Yes |
| Default `cli-chat-proxy.grok.com` | No default base URL; models need explicit `base_url` / `models_base_url` | Yes |
| Embedded `grok-build` catalog | `default_models.json` empty (no default model id) | Yes (file kept) |
| SuperGrok / billing upsells | Hidden when not first-party-authenticated | Yes |
| Managed MCP from proxy | No fetch without proxy URL + session | Yes |
| `grok login` | Shim: message pointing to `grok setup` / config.toml; does not open x.ai browser login | Yes |
| Headless `grok -p` | Requires configured model credentials; clear error if missing | Yes |
| Telemetry to x.ai | Remains off unless explicitly configured | Yes |

### Auth advertisement rule (fork default)

- If BYOK / API key / setup-complete local model present → advertise `xai.api_key` only (method id kept for upstream merge ease)
- Else → no auth methods that imply interactive login; pager runs wizard
- Never append `grok.com` (or OIDC interactive login) in the default unpinned path
- Do not advertise `cached_token` from legacy x.ai `auth.json` as a reason to skip setup

### Power-user escape (no wizard)

Unchanged documented path:

- `[model.*]` with `api_key` / `env_key`, and/or
- `GROK_MODELS_BASE_URL` + API key

## Data flow

```text
wizard inputs → validate endpoint → write ~/.grok/config.toml
      → pager reloads effective config / re-inits ACP
      → shell resolve_credentials(model) → sampler Bearer / extra_headers
```

## Error handling

| Case | Behavior |
|---|---|
| Wizard validation failure | Inline error; keep form state; do not commit a partial default model |
| Config write failure | Show path + error; do not enter session |
| Mid-session 401 on custom provider | Surface provider auth error; **do not** fall back to grok.com re-auth |
| Headless with no model creds | Exit with message pointing at `grok setup` / config example |
| `grok login` | Shim to `grok setup`; no browser OAuth |

## Testing

1. Auth methods: unpinned default with no creds → no `grok.com` in list
2. Startup: no creds → wizard trigger, not `Action::Login`
3. Wizard persist: preset → expected `[model.*]` + `preferred_method = "api_key"`
4. Smoke: configured custom model starts without interactive login metadata
5. Regression: existing BYOK / `models_base_url` paths still skip wizard

## Implementation sketch (for planning)

Ordered work packages (detail belongs in the implementation plan):

1. **Defaults cutover** — empty catalog, no default proxy URL, auth methods omit `grok.com`
2. **Startup gate** — replace auto-login with wizard entry condition
3. **Provider presets + config writer**
4. **Wizard UI + `grok setup` / `/setup`**
5. **Shims & hide first-party UX** — `grok login`, billing CTAs, managed MCP idle behavior
6. **Tests + docs** — user-guide updates for custom models / first-run

## Risks

| Risk | Mitigation |
|---|---|
| Upstream reintroduces grok.com as first auth method | Keep fork change localized in `build_auth_methods` / startup metadata with a clear comment + tests |
| Empty catalog breaks code assuming a default model | Ensure wizard or explicit config is required before session create; add guards with clear errors |
| Ollama-without-key still looks “unauthenticated” today | Treat Ollama preset as credentialed when `base_url` is local and key optional; teach `has_own_credentials` / startup gate accordingly |
| Residual first-party calls (settings, MCP) | Idle when proxy URL unset; do not invent a default proxy |

## Success criteria

- Fresh install never opens accounts.x.ai / grok.com login
- User can complete wizard and chat with OpenAI, Anthropic, or Ollama without any x.ai account
- `git merge` from upstream typically conflicts only at the small seam files above, not across deleted auth modules
