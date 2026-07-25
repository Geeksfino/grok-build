# Task 2 Report: Remove implicit cli-chat-proxy default URL

## Summary

Removed the implicit fallback to
`https://cli-chat-proxy.grok.com/v1` from endpoint resolution so shell and
pager flows stay idle unless a proxy is explicitly configured. The
`CLI_CHAT_PROXY_BASE_URL_DEFAULT` constant remains defined for opt-in usage and
documentation, but `EndpointsConfig::proxy_url()` no longer uses it as a
runtime default.

## TDD Evidence

### RED

Added the required regression tests to
`crates/codegen/xai-grok-shell/src/agent/config.rs`:

- `proxy_url_has_no_hardcoded_default`
- `resolve_inference_base_url_empty_without_models_base_url`

Ran the exact RED command from the brief:

```bash
cargo test -p xai-grok-shell proxy_url_has_no_hardcoded_default -- --nocapture
```

Observed failure:

- compile failed because `EndpointsConfig::proxy_url_configured()` did not exist
- the new test therefore proved the new empty-default contract was not
  implemented yet

### GREEN

Implemented the minimal production change:

- added `EndpointsConfig::proxy_url_configured() -> bool`
- changed `EndpointsConfig::proxy_url()` to return `""` when unset
- made proxy-derived resolvers return `""` instead of `"/models"`,
  `"/deployment/config"`, or `"/traces"` when no proxy is configured
- updated pager `RefreshGate` settings fetch to skip the network call when the
  resolved proxy URL is empty
- updated stale shell tests that still pinned the old implicit proxy default

Ran the focused verification commands:

```bash
cargo test -p xai-grok-shell proxy_url_has_no_hardcoded_default
cargo test -p xai-grok-shell resolve_inference_base_url_empty_without_models_base_url
cargo test -p xai-grok-shell aux_endpoints_resolve_to_proxy
cargo test -p xai-grok-shell loader_managed_config_url
cargo test -p xai-grok-shell models_fetch_endpoint_matches_auth_mode
cargo test -p xai-grok-shell deployment_config_url_stays_empty_when_proxy_not_overridden
```

Result: PASS (all focused tests passed).

## Files Changed

- `crates/codegen/xai-grok-shell/src/agent/config.rs`
- `crates/codegen/xai-grok-shell/src/remote/client.rs`
- `crates/codegen/xai-grok-pager/src/app/effects/mod.rs`

## Self-Review

- `CLI_CHAT_PROXY_BASE_URL_DEFAULT` remains defined but no longer drives runtime
  fallback behavior
- inference and auxiliary resolvers now stay empty unless explicitly configured,
  which matches the provider-neutral default contract from the brief
- explicit per-endpoint and explicit proxy overrides still win unchanged
- pager gate refresh now no-ops instead of silently contacting the public proxy
- I found two additional focused tests in `remote/client.rs` that still assumed
  the old implicit proxy default and updated them to the new idle/empty behavior

## Concerns

- No code-level concerns.
- Verification was intentionally limited to targeted cargo tests per the brief;
  I did not run the full crate or workspace test suite.

---

## Review Fix Follow-up: shell stays idle when proxy URL is unset

### Fixes applied

- `crates/codegen/xai-grok-shell/src/managed_config.rs`
  - `fetch_for_principal()` now returns early with an empty result when the
    resolved managed-config URL is empty.
  - `fetch_managed_config_once()` now no-ops on an empty URL as a second guard.
- `crates/codegen/xai-grok-shell/src/agent/models.rs`
  - startup models/settings prefetch now resolves explicit startup URLs first
    and skips both fetches when proxy/models endpoints are empty.
  - startup prefetch only uses explicit `models_base_url` / `models_list_url`
    or the configured proxy; it does not fall through to an implicit proxy URL.
- `crates/codegen/xai-grok-shell/src/auth/flow.rs`
  - device-flow remote probing is now gated behind a non-empty proxy URL.
- `crates/codegen/xai-grok-shell/src/remote/client.rs`
  - `fetch_settings_blocking()` and `fetch_login_device_flow()` now return
    immediately when called with an empty proxy URL.

### Added regression coverage

- `managed_config::tests::fetch_for_principal_skips_when_managed_config_url_unset`
- `auth::flow::tests::login_device_flow_probe_url_skips_empty_proxy`
- `agent::models::tests::startup_prefetch_urls_skip_empty_proxy_but_keep_explicit_models_endpoints`
  - includes the ambient-API-key/no-proxy case to prove startup prefetch still
    stays idle without an explicit models endpoint

### Verification evidence

```bash
$ cargo test -p xai-grok-shell proxy_url_has_no_hardcoded_default
test agent::config::tests::proxy_url_has_no_hardcoded_default ... ok
test result: ok. 1 passed; 0 failed

$ cargo test -p xai-grok-shell aux_endpoints_resolve_to_proxy
test agent::config::tests::aux_endpoints_resolve_to_proxy_never_inference ... ok
test result: ok. 1 passed; 0 failed

$ cargo test -p xai-grok-shell loader_managed_config_url
test agent::config::tests::loader_managed_config_url_never_follows_inference_endpoint ... ok
test result: ok. 1 passed; 0 failed

$ cargo test -p xai-grok-shell fetch_for_principal_skips_when_managed_config_url_unset
test managed_config::tests::fetch_for_principal_skips_when_managed_config_url_unset ... ok
test result: ok. 1 passed; 0 failed

$ cargo test -p xai-grok-shell login_device_flow_probe_url_skips_empty_proxy
test auth::flow::tests::login_device_flow_probe_url_skips_empty_proxy ... ok
test result: ok. 1 passed; 0 failed

$ cargo test -p xai-grok-shell startup_prefetch_urls_skip_empty_proxy_but_keep_explicit_models_endpoints
test agent::models::tests::startup_prefetch_urls_skip_empty_proxy_but_keep_explicit_models_endpoints ... ok
test result: ok. 1 passed; 0 failed
```
