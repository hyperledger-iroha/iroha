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
- Release acceptance must require its network fixtures instead of accepting sandbox-related skips.
  Run the dynamic-access serialization gate with
  `IROHA_TEST_REQUIRE_NETWORK=1 cargo test -p integration_tests --test core_api contracts::dynamic_and_helper_hidden_contract_writes_serialize_on_four_peers -- --exact --nocapture`.
  The pull-request test job sets this switch; ordinary developer runs keep the existing sandbox-skip behavior.
- Feature flags: `telemetry` (default), `fault_injection`, `norito_streaming_fec`, `js_host_parity`. Enable with `cargo test -p integration_tests --features "<feature list>"`.
- Ignored/long cases (e.g., adversarial network, flaky trigger paths): `IROHA_RUN_IGNORED=1 cargo test -p integration_tests -- --ignored --nocapture`.
- The four-peer autoscale A/B/A lifecycle and rotating-validator Native AMX release gates remain in the ordinary, non-ignored Cargo inventory so release automation can detect renames or ignored tests. Plain developer suites take a fast opt-out; set `IROHA_RUN_IGNORED=1` with the exact test filter to execute them locally. Production uses `IROHA_MULTILANE_RELEASE_MODE=1`, requires a real network, and rejects missing completion markers.
- The workspace Cargo config and dev/test profiles keep plain `cargo test` conservative by default (`build.jobs=1`, `profile.{dev,test}.debug=0`, `profile.test.incremental=false`, `RUST_TEST_THREADS=1`) so WSL and memory-constrained VMs do not fan out across every logical CPU, emit debug-heavy Linux artifacts, or retain large incremental-cache working sets. Override with `cargo test --jobs <N>`, `CARGO_PROFILE_TEST_DEBUG=line-tables-only`, `CARGO_INCREMENTAL=1`, and/or `RUST_TEST_THREADS=<N>` on high-memory hosts.
- High-count integration-test suites in workspace crates use explicit grouped harnesses instead of Cargo's automatic one-file-one-binary discovery, reducing duplicate test binary linking in default workspace runs.
- Test networks run one-at-a-time by default so plain `cargo test` stays stable on WSL and memory-constrained VMs. Increase concurrency with `IROHA_TEST_NETWORK_PARALLELISM=<N>` on high-memory hosts; set `IROHA_TEST_SERIALIZE_NETWORKS=1` to force one-at-a-time startup explicitly.
- `scripts/run_full_tests.sh` now reuses the workspace-built `iroha3d`, `iroha`, and `kagami` binaries when they are available and isolates the integration-test permit directory by default.
- For WSL or memory-constrained VMs, first size WSL memory/swap/host disk headroom appropriately; use `scripts/run_full_tests.sh --wsl-safe --target-dir /tmp/iroha-wsl-tests` only when you still need a conservative local run. This mode runs non-integration workspace tests one package at a time, sets `CARGO_INCREMENTAL=0`, serializes network tests, writes resource snapshots to `<target-dir>/run_full_tests_resources.log`, and refuses to start the next Cargo step when `MemAvailable` is below `4096` MiB unless overridden with `--min-available-mib`.
- For faster local full runs, `scripts/run_full_tests.sh --fast` routes all cargo calls through `scripts/cargo_fast.sh`; add `--fast-zero-debug` and `--no-incremental` when you want the more aggressive local-throughput mode.

## Fixtures
- IVM bytecode fixtures refresh automatically via `build.rs` when tests run.
- Regenerate SoraFS gateway fixtures: `cargo run -p integration_tests --bin sorafs-gateway-fixtures -- --out fixtures/sorafs_gateway`.
- Regenerate grouped `nexus_and_streaming` Norito instruction + streaming goldens:
  `cargo run -p integration_tests --bin refresh_nexus_streaming_fixtures`.

## Notes
- Pipeline block rejection scaffold lives at `tests/pipeline_block_rejected.rs` inside the `core_api` harness and is `#[ignore]` until a deterministic trigger is available.
- Canonical Jindo activation, pre-activation rejection, exact replay, and
  restarted-peer catch-up coverage lives in
  `tests/privacy_exact12_jindo_network.rs` inside the `network_functional`
  harness.
- Kagemusha lifecycle coverage lives in the dedicated four-wallet driver and
  exercises top-up, offline multihop split/change, restart recovery, and exact
  redemption under Halo2 verification.
- SoraNet web deploy + public DNS ALIAS/CNAME + NS/DS delegation placeholders coverage lives at `tests/soranet_web_deploy.rs`.
