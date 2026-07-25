Status: implemented provider presets, config.toml writer, lib.rs registrations, and a setup_wizard stub.
Tests: red phase observed via missing ProviderModelWrite API; green verification via `cargo test -p xai-grok-pager provider_ -- --nocapture`.
Concerns: `cargo fmt --all` currently fails on an unrelated Rust 2024 `let`-chain parse issue in `crates/codegen/xai-grok-pager/src/acp/mod.rs`; Task 5 files were formatted with targeted `rustfmt --edition 2024`.
Commit: feat: add provider presets and config writer
