# Final Fix Report

## Provider-neutral defaults review follow-up

- Fixed setup-wizard credential persistence so typed API keys are only written to `config.toml` when `Store in config.toml` is enabled.
- When a user opts out of storing the key, submit now requires the preset/custom `env_key` to already be exported for reconnect and surfaces a wizard error that points to `ENV_KEY=...` or `Store in config.toml`.
- Kept validation using the typed key in-memory, while preventing reconnect from silently depending on a disk-persisted fallback secret.
- Added a custom-preset API-backend selector in the wizard and persisted the selected backend through the existing provider config writer.
- Updated provider validation probing so `responses` backends validate against `/responses` instead of `/chat/completions`.
- Swapped the empty-auth login-method error copy to the provider-setup guidance (`grok provider`).

## Verification

- `cargo test -p xai-grok-pager setup_wizard -- --nocapture`
- `cargo test -p xai-grok-pager provider_ -- --nocapture`
