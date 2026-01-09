# Integration Tests

This crate hosts cross-component tests for Iroha.

## Running tests
- Default suite: `cargo test -p integration_tests -- --nocapture`
- Feature flags: `telemetry` (default), `fault_injection`, `norito_streaming_fec`, `js_host_parity`. Enable with `cargo test -p integration_tests --features "<feature list>"`.
- Ignored/long cases (e.g., adversarial network, flaky trigger paths): `IROHA_RUN_IGNORED=1 cargo test -p integration_tests -- --ignored --nocapture`.
- Limit concurrent test networks with `IROHA_TEST_NETWORK_PARALLELISM=<N>` (default scales with CPU/min peers); set `IROHA_TEST_SERIALIZE_NETWORKS=1` to force one-at-a-time startup.
- Target a single test: `cargo test -p integration_tests <test_name> -- --nocapture`.

## Fixtures
- IVM bytecode fixtures refresh automatically via `build.rs` when tests run.
- Regenerate SoraFS gateway fixtures: `cargo run -p integration_tests --bin sorafs-gateway-fixtures -- --out fixtures/sorafs_gateway`.

## Notes
- Pipeline block rejection scaffold lives at `tests/pipeline_block_rejected.rs` and is `#[ignore]` until a deterministic trigger is available.
- SoraNet web deploy + public DNS ALIAS/CNAME + NS/DS delegation placeholders coverage lives at `tests/soranet_web_deploy.rs`.
