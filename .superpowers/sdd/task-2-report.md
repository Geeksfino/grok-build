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
