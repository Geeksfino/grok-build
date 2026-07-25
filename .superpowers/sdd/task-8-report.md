## Task 8 report

- Added `Command::Provider` to clap and normalized `grok provider` into an interactive startup that sets `GROK_FORCE_PROVIDER_SETUP=1`, reusing the existing startup wizard path.
- Replaced the `grok login` CLI flow with a hard shim: `login_shim_message()` is printed to stderr and the process exits with code 1.
- Added `/provider` as a builtin slash command that dispatches `Action::OpenSetupWizard`.
- Updated headless no-credentials guidance to point users at `grok provider` or `[model.*]` config instead of browser login.
- Left `Command::Setup` untouched for managed team configuration.

### Verification

- Red: `cargo test -p xai-grok-pager login_shim_message -- --nocapture` failed before implementation because `login_shim_message()` and `Command::Provider` did not exist.
- Green: `cargo test -p xai-grok-pager login_shim_message_points_at_grok_provider_not_xai_account_login -- --nocapture`
- Green: `cargo test -p xai-grok-pager provider_subcommand_parses -- --nocapture`
- Green: `cargo test -p xai-grok-pager provider_command_is_builtin_and_opens_setup_wizard -- --nocapture`
- Green: `cargo test -p xai-grok-pager auth_required_message_points_headless_users_to_grok_provider -- --nocapture`
- Green: `cargo check -p xai-grok-pager-bin`

### Notes

- `grok provider` now forces the setup wizard even if auth methods are advertised, without changing the existing cold-start and managed-setup flows.

## Task 8 follow-up: Medium review findings

- Updated `Command::Login` clap docs to present `login` as a disabled compatibility shim that points users to `grok provider` or manual `~/.grok/config.toml` configuration.
- Hid `--oauth` and `--device-auth` from clap help while keeping both flags parseable for backwards compatibility.
- Replaced pager/pager-bin remediation text that still said `grok login` with `grok provider` or direct `config.toml` / deployment-key guidance, preserving the managed `grok setup` deployment-key flow.

### Verification

- Red: `cargo test -p xai-grok-pager login_shim_message` failed before the edit because `grok login` was still documented and the deprecated flags were still visible in `grok login --help`.
- Green: `cargo test -p xai-grok-pager login_shim_message`
- Green: `cargo check -p xai-grok-pager-bin`

## Task 8 follow-up: Messaging accuracy review fixes

- Replaced `grok workspace` first-party auth failures with deployment-key/provider-neutral guidance instead of suggesting `grok provider`.
- Restored `grok setup` missing-principal guidance so it keeps both valid principals: deployment key and existing team sign-in.
- Updated trace upload auth failures to require a deployment key or explicit upload endpoint/auth configuration, not `grok provider`.
- Confirmed the remaining `grok provider` copy is limited to the login shim and headless/model-missing paths.

### Verification

- Red: `cargo test -p xai-grok-pager-bin workspace_messages_do_not_point_first_party_failures_at_provider_setup -- --nocapture ; cargo test -p xai-grok-pager missing_upload_credentials_message_requires_first_party_upload_config -- --nocapture` failed before implementation because the new helper-backed assertions had no production message helpers yet.
- Green: `cargo test -p xai-grok-pager-bin workspace_messages_do_not_point_first_party_failures_at_provider_setup -- --nocapture`
- Green: `cargo test -p xai-grok-pager-bin setup_missing_principal_message_mentions_team_sign_in_and_deployment_key -- --nocapture`
- Green: `cargo test -p xai-grok-pager login_shim_message_login_help_points_at_provider_setup -- --nocapture`
- Green: `cargo test -p xai-grok-pager login_shim_message_points_at_grok_provider_not_xai_account_login -- --nocapture`
- Green: `cargo test -p xai-grok-pager auth_required_message_points_headless_users_to_grok_provider -- --nocapture`
- Green: `cargo test -p xai-grok-pager missing_upload_credentials_message_requires_first_party_upload_config -- --nocapture`
- Green: `cargo check -p xai-grok-pager-bin`
