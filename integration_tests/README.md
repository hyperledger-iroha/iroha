# Integration Tests

This crate hosts cross-component tests for Iroha.

## Running tests
- Default suite: `cargo test -p integration_tests -- --nocapture`
- Grouped harnesses:
  - `core_api`
  - `events_and_triggers`
  - `queries_and_proofs`
  - `network_functional`
  - `consensus_and_da`
  - `nexus_and_streaming`
- Target a harness directly with `cargo test -p integration_tests --test <harness>`.
- Target a single test with `cargo test -p integration_tests --test <harness> <filter> -- --nocapture`.
- Exact test filters are now module-qualified inside the grouped harnesses; for example:
  `cargo test -p integration_tests --test core_api asset::client_add_asset_quantities_should_increase_asset_amounts -- --exact --nocapture`
- Feature flags: `telemetry` (default), `fault_injection`, `norito_streaming_fec`, `js_host_parity`. Enable with `cargo test -p integration_tests --features "<feature list>"`.
- Ignored/long cases (e.g., adversarial network, flaky trigger paths): `IROHA_RUN_IGNORED=1 cargo test -p integration_tests -- --ignored --nocapture`.
- Limit concurrent test networks with `IROHA_TEST_NETWORK_PARALLELISM=<N>` (default scales with CPU/min peers); set `IROHA_TEST_SERIALIZE_NETWORKS=1` to force one-at-a-time startup.
- `scripts/run_full_tests.sh` now reuses the workspace-built `iroha3d`, `iroha`, and `kagami` binaries when they are available and isolates the integration-test permit directory by default.
- For faster local full runs, `scripts/run_full_tests.sh --fast` routes all cargo calls through `scripts/cargo_fast.sh`; add `--fast-zero-debug` and `--fast-no-incremental` when you want the more aggressive local-throughput mode.

## Fixtures
- IVM bytecode fixtures refresh automatically via `build.rs` when tests run.
- Regenerate SoraFS gateway fixtures: `cargo run -p integration_tests --bin sorafs-gateway-fixtures -- --out fixtures/sorafs_gateway`.
- Regenerate grouped `nexus_and_streaming` Norito instruction + streaming goldens:
  `cargo run -p integration_tests --bin refresh_nexus_streaming_fixtures`.

## Notes
- Pipeline block rejection scaffold lives at `tests/pipeline_block_rejected.rs` inside the `core_api` harness and is `#[ignore]` until a deterministic trigger is available.
- Offline Note V2 four-peer issue/audit/redeem coverage lives at `tests/extra_functional/offline_note_v2.rs` inside the `network_functional` harness and enables Halo2 verification through test-network config.
- SoraNet web deploy + public DNS ALIAS/CNAME + NS/DS delegation placeholders coverage lives at `tests/soranet_web_deploy.rs`.
- SoraFS reconciliation divergence reports are exercised in `tests/sorafs_reconciliation.rs`.
