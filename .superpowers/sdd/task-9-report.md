# Task 9 Report: Idle first-party UX + docs

## Status

Completed on branch `cursor/provider-neutral-defaults-ea70`.

## What changed

- Added a focused pager regression test: `cold_start_without_auth_meta_hides_usage_upsell`.
- Gated `usage_visible` behind authenticated session metadata, plus existing team/API-key checks.
- Recomputed the gate consistently across startup, reconnect, settings updates, and setup-wizard open/close.
- Updated docs to point first run at `grok provider`, with `grok setup` called out as managed team configuration.
- Removed shell/root README guidance that implied fresh installs should log in with `grok login`/grok.com first.

## Verification

TDD:

1. `cargo test -p xai-grok-pager cold_start_without_auth_meta_hides_usage_upsell -- --nocapture` (failed first, then passed)

Focused follow-up:

- `cargo test -p xai-grok-pager settings_non_api_key_tier_clears_stale_api_key_flag -- --nocapture`
- `cargo test -p xai-grok-pager open_setup_wizard_sets_cold_start_state -- --nocapture`

Brief Step 4 suite:

- `cargo test -p xai-grok-models`
- `cargo test -p xai-grok-shell auth_method`
- `cargo test -p xai-grok-shell proxy_url_has_no_hardcoded_default`
- `cargo test -p xai-grok-shell auth_not_required`
- `cargo test -p xai-grok-pager startup_auth`
- `cargo test -p xai-grok-pager provider_`
- `cargo test -p xai-grok-pager setup_wizard`
- `cargo test -p xai-grok-pager login_shim`
- `cargo check -p xai-grok-pager-bin`

All passed.

## Concerns

- The shell README still documents `auth.json` for advanced first-party session/proxy use; that remains intentional, but it is no longer described as the default fresh-install path.

## Review follow-up

- Tightened `apply_tier_restrictions()` so slash-command / voice SuperGrok upsells only apply when a first-party authenticated session exists.
- Added pager regression test `restricted_command_without_authenticated_session_does_not_open_upsell` to keep cold-start and BYOK users out of the restricted-command modal path.
- Updated `01-getting-started.md` and `02-authentication.md` so first run points to `grok provider` / `config.toml`, `grok login` is described as disabled, and `grok setup` stays reserved for managed team config.
- `TaskResult::LogoutComplete` now clears `has_authenticated_session`, recomputes `usage_visible`, and reapplies tier restrictions so stale SuperGrok CTAs disappear immediately after logout.
- Added pager regression test `logout_clears_authenticated_billing_state` to lock the logout reset behavior.
- Updated `14-headless-mode.md` and `docs/user-guide/README.md` so headless/default auth guidance points to `grok provider`, `~/.grok/config.toml`, and API keys; `grok setup` remains the managed team bootstrap path.

Additional focused verification:

- `cargo test -p xai-grok-pager logout_ -- --nocapture`
- `cargo test -p xai-grok-pager restricted_command_ -- --nocapture`
