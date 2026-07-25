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
