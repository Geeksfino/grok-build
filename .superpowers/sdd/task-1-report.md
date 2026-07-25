# Task 1 Report: Empty baked-in model catalog

## Summary

Implemented the provider-neutral default by clearing the embedded
`xai-grok-models` catalog and relaxing the loader so an empty `default` is only
legal when the embedded `models` array is empty.

## TDD Evidence

### RED

1. Added the required tests to `crates/codegen/xai-grok-models/src/lib.rs`:
   - `empty_catalog_allows_blank_default`
   - `embedded_default_model_is_blank`
2. Ran the exact RED command from the brief:

```bash
cargo test -p xai-grok-models embedded_default_model_is_blank -- --nocapture
```

Observed failure:

- test `tests::embedded_default_model_is_blank` failed
- assertion mismatch: left `"grok-build"` vs right `""`

This proved the baked-in catalog still exposed `grok-build` before the fix.

### GREEN

Implemented the minimal production change:

- Replaced `default_models.json` with an empty catalog
- Updated the `LazyLock` validation so blank `default` is only accepted when
  `models` is empty
- Documented that `default_model()` returning `""` means config or setup must
  supply the model

Ran the required verification commands:

```bash
cargo test -p xai-grok-models
```

Result: PASS (`2 passed; 0 failed`)

```bash
cargo test -p xai-grok-shell default_models -- --nocapture
```

Result: PASS

Focused shell tests that ran under the filter included:

- `agent::config::tests::config_models_default_is_not_overwritten_by_default_models_json`
- `agent::config::tests::default_models_dual_endpoint_routing`

No `xai-grok-shell` source or test expectation changes were needed.

## Environment Notes

The focused `xai-grok-shell` verification initially failed before reaching test
assertions because `bin/protoc` required `dotslash` and the machine did not have
it on `PATH`. I installed it with:

```bash
cargo install dotslash
```

After that, the shell verification command passed.

## Files Changed

- `crates/codegen/xai-grok-models/default_models.json`
- `crates/codegen/xai-grok-models/src/lib.rs`

## Self-Review

- Change scope is minimal and matches the brief exactly
- Loader invariant still protects against invalid embedded catalogs
- Tests cover both parsing an explicit empty catalog and the embedded runtime
  default behavior
- Downstream shell verification passed without requiring compatibility shims or
  reintroducing `grok-build`

## Concerns

- No code-level concerns.
- Environment setup for future cloud agents should include `dotslash` so
  `bin/protoc` works without manual installation.

## Review Follow-up: shell tests adapted for empty catalog

Reviewer finding confirmed: `crates/codegen/xai-grok-shell/src/agent/config.rs`
still had unit tests that assumed the embedded catalog provided a non-empty
default model or a bundled `grok-build` donor entry. The fix kept
`default_models.json` empty and updated the shell tests instead.

### Shell test changes

- Replaced `crate::models::default_model()` test assumptions with explicit test
  model ids where the test only needed "some model" rather than a baked-in one.
- Reworked routing tests to pass explicit `ModelEntry` fixtures instead of
  expecting a bundled default model to exist.
- Updated the default-endpoint test to assert the resolved catalog is empty
  under the empty baked-in contract.
- Reworked bundled-`grok-build` visibility/inheritance tests to use explicit
  prefetched donor fixtures when that behavior was the real thing under test.
- Kept global-header and endpoint inheritance coverage by creating explicit
  `[model.<id>]` overrides rather than relying on embedded defaults.

### Verification commands and results

```bash
cargo test -p xai-grok-models
```

PASS: `2 passed; 0 failed`

```bash
cargo test -p xai-grok-shell default_models -- --nocapture
```

PASS: filtered shell tests passed, including:

- `agent::config::tests::config_models_default_is_not_overwritten_by_default_models_json`
- `agent::config::tests::default_models_dual_endpoint_routing`

```bash
cargo test -p xai-grok-shell e2e_default_endpoint_still_injects_defaults -- --nocapture
```

PASS: `1 passed; 0 failed`

```bash
cargo test -p xai-grok-shell e2e_default_model_with_session_routes_to_proxy -- --nocapture
```

PASS: `1 passed; 0 failed`

Additional focused regression checks run while fixing adjacent assumptions:

```bash
cargo test -p xai-grok-shell user_override_adds_api_key_to_default_model -- --nocapture
cargo test -p xai-grok-shell global_extra_headers_apply_to_model_without_override -- --nocapture
cargo test -p xai-grok-shell e2e_enterprise_endpoints_only_no_model_override -- --nocapture
cargo test -p xai-grok-shell resolve_model_list_prunes_bundled_entries_not_in_prefetch -- --nocapture
cargo test -p xai-grok-shell byok_config_overlay_visible_to_api_key_users -- --nocapture
cargo test -p xai-grok-shell plain_config_overlay_preserves_bundled_visibility -- --nocapture
```

PASS: all targeted follow-up tests passed after adapting the remaining empty
catalog and prefetched-donor expectations.
