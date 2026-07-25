# Task 7 report: setup wizard UI, validation, persist, re-enter TUI

## Scope completed

- Replaced the setup wizard stub with a real state machine and keyboard-driven TUI flow.
- Added provider endpoint validation with HTTP probes and human-readable failures.
- Added provider config persistence through `write_provider_model_config`.
- Wired async submit handling, ACP reconnect, wizard teardown, and deferred session-startup replay.
- Guarded retry-state 401 handling so custom/BYOK providers without an interactive method do not fall back to grok.com re-auth.

## Files changed

- `crates/codegen/xai-grok-pager/src/setup_wizard/mod.rs`
- `crates/codegen/xai-grok-pager/src/setup_wizard/state.rs`
- `crates/codegen/xai-grok-pager/src/setup_wizard/validate.rs`
- `crates/codegen/xai-grok-pager/src/provider_config_write.rs`
- `crates/codegen/xai-grok-pager/src/app/actions.rs`
- `crates/codegen/xai-grok-pager/src/app/app_view.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/router.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/task_result.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/event_loop.rs`
- `crates/codegen/xai-grok-pager/src/app/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/acp_handler/session_notification.rs`
- `crates/codegen/xai-grok-pager/src/app/acp_handler/tests/session_events.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/mod.rs`
- `crates/codegen/xai-grok-pager/src/app/dispatch/tests/task_result.rs`
- `crates/codegen/xai-grok-pager/Cargo.toml`
- `Cargo.lock`

## Verification run

### Focused verification that passed

1. `cargo test -p xai-grok-pager validate_rejects_empty_model_id -- --nocapture`
2. `cargo test -p xai-grok-pager setup_wizard -- --nocapture`
3. `cargo test -p xai-grok-pager apply_retry_state_custom_provider_401_without_interactive_login_keeps_retry_failed -- --nocapture`
4. `cargo test -p xai-grok-pager setup_wizard_submit_complete -- --nocapture`

### Broader verification

- `cargo test -p xai-grok-pager --lib`
- Result: failed with 8 unrelated pre-existing failures outside the Task 7 surface.

Failing tests from the broader library run:

- `app::modals::command_palette_vim_input_tests::command_palette_search_bar_cursor_only_when_focused`
- `scrollback::blocks::user::tests::collapsed_truncation_keeps_teal_on_straddling_token`
- `scrollback::blocks::user::tests::collapsed_truncation_keeps_teal_on_token_within_last_line`
- `scrollback::blocks::user::tests::invalid_token_ranges_are_dropped`
- `scrollback::blocks::user::tests::mid_text_multiple_tokens_each_teal`
- `scrollback::blocks::user::tests::mid_text_token_on_second_logical_line`
- `scrollback::blocks::user::tests::narrow_wrap_keeps_teal_on_both_rows_of_split_token`
- `views::picker::tests::search_bar_cursor_visible_only_when_search_active`

These failures are in untouched command-palette, picker, and scrollback rendering tests rather than the wizard/auth/reconnect paths changed for Task 7.

## Notes / concerns

- The reconnect path now swaps the ACP transport in the running event loop after wizard success; this is covered by focused dispatch/task-result tests, but it was not exercised through a full interactive smoke test in this environment.
- Full-library verification is not green on the branch because of the unrelated failures listed above.

## Review follow-up: q/space input handling

- Scoped plain `q` quitting so text-entry fields in setup-wizard edit mode insert `q` instead of quitting; forced quit remains on `Ctrl+C`/`Ctrl+D`.
- Changed space handling so text fields insert spaces, while the store-key-in-config checkbox still toggles on space.
- Removed the unused `SetupWizardPhase::Done` state and `mark_done` helper as dead code.

### Added focused regression tests

- `q_in_model_and_api_key_fields_inserts_text_without_quitting`
- `space_in_display_name_field_inserts_space`
- `space_on_store_key_checkbox_toggles_value`

### Focused verification that passed

- `cargo test -p xai-grok-pager setup_wizard::tests::`

## Review follow-up: success-path safety and reconnect logging

- Removed the `Effect::SubmitSetupWizard` process-env mutation path; the effect now validates, writes config, reconnects, and returns a post-setup notice only.
- Changed setup-wizard submission building so a typed key with `store_key_in_config = false` still persists `api_key` when the configured `env_key` is currently unset, preserving immediate reconnect without `std::env::set_var`.
- Renamed the success notice field from `transient_env_notice` to `post_setup_notice` to match the new behavior.
- Reworked `crates/codegen/xai-grok-pager/src/unified_log.rs` to keep the sender in `Mutex<Option<AcpAgentTx>>`, spawn its flush loop once, and allow `set_sender(...)` rebinding after reconnect.
- Rebound the logger sender in `app/event_loop.rs` immediately before swapping `app.acp_tx` to the new ACP connection.

### Added focused regression tests

- `setup_wizard::tests::typed_api_key_without_exported_env_is_persisted_for_reconnect`
- `unified_log::tests::set_sender_rebinds_future_flushes`

### Focused verification that passed

- `cargo test -p xai-grok-pager typed_api_key_without_exported_env_is_persisted_for_reconnect`
- `cargo test -p xai-grok-pager setup_wizard_submit_complete_success_clears_wizard_and_replays_startup`
- `cargo test -p xai-grok-pager setup_wizard_submit_complete_error_keeps_wizard_open`
- `cargo test -p xai-grok-pager set_sender_rebinds_future_flushes`
