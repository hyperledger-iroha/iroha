# Integration Tests

This crate hosts cross-component tests for Iroha.

## Running tests
- Default suite: `cargo test -p integration_tests -- --nocapture`
- Grouped harnesses:
  - `core_api`
  - `events_and_triggers`
  - `queries_and_proofs`
  - `network_functional`
  - `privacy_release_network` (requires `zk-stark,privacy-release-evidence`)
  - `consensus_and_da`
  - `nexus_and_streaming`
- Target a harness directly with `cargo test -p integration_tests --test <harness>`.
- Target a single test with `cargo test -p integration_tests --test <harness> <filter> -- --nocapture`.
- Exact test filters are now module-qualified inside the grouped harnesses; for example:
  `cargo test -p integration_tests --test core_api asset::client_add_asset_quantities_should_increase_asset_amounts -- --exact --nocapture`
- Release acceptance must require its network fixtures instead of accepting sandbox-related skips.
  Exact12 release qualification uses the dedicated `privacy_release_network`
  harness. It is the sole Cargo harness that owns all seven Exact12 network
  modules; the general-purpose `network_functional` harness retains only its
  ambient network scenarios. Cargo refuses to build the release harness unless
  both required features are present, and its module inventory is unconditional
  so a feature omission cannot produce a successful zero-test run.
  Run the dynamic-access serialization gate with
  `IROHA_TEST_REQUIRE_NETWORK=1 cargo test -p integration_tests --test core_api contracts::dynamic_and_helper_hidden_contract_writes_serialize_on_four_peers -- --exact --nocapture`.
  The Kotodama/IVM V1 release gate is
  `IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test core_api contracts::contract_v1_executes_and_survives_four_peer_da_rbc_restart -- --exact --nocapture --test-threads=1`.
  It authenticates the signed NPoS/mandatory-DA genesis contract, requires
  cross-peer RBC evidence for the deployment, decodes the canonical `int`
  result and persisted state on all four peers, then repeats both reads through
  a cold-restarted validator.
  The pull-request test job sets this switch; ordinary developer runs keep the existing sandbox-skip behavior.
- Feature flags: `telemetry` (default), `fault_injection`, `norito_streaming_fec`, `js_host_parity`, `zk-stark`, and the non-shipping `privacy-release-evidence` gate. Enable with `cargo test -p integration_tests --features "<feature list>"`.
- Ignored/long cases (e.g., adversarial network, flaky trigger paths): `IROHA_RUN_IGNORED=1 cargo test -p integration_tests -- --ignored --nocapture`.
- The four-peer autoscale A/B/A lifecycle and rotating-validator Native AMX release gates remain in the ordinary, non-ignored Cargo inventory so release automation can detect renames or ignored tests. Plain developer suites take a fast opt-out; set `IROHA_RUN_IGNORED=1` with the exact test filter to execute them locally. Production uses `IROHA_MULTILANE_RELEASE_MODE=1`, requires a real network, and rejects missing completion markers.
- Plain `cargo test` now uses Cargo's native jobserver and libtest's native thread selection; the workspace no longer serializes every developer build or test globally. Memory-constrained and release-evidence wrappers set scoped `--jobs`, `RUST_TEST_THREADS`, debug, and incremental limits only for their own runs.
- High-count integration-test suites in workspace crates use explicit grouped harnesses instead of Cargo's automatic one-file-one-binary discovery, reducing duplicate test binary linking in default workspace runs.
- Test networks run one-at-a-time by default so plain `cargo test` stays stable on WSL and memory-constrained VMs. Increase concurrency with `IROHA_TEST_NETWORK_PARALLELISM=<N>` on high-memory hosts; set `IROHA_TEST_SERIALIZE_NETWORKS=1` to force one-at-a-time startup explicitly.
- `scripts/run_full_tests.sh` now reuses the workspace-built `iroha3d`, `iroha`, and `kagami` binaries when they are available and isolates the integration-test permit directory by default.
- For WSL or memory-constrained VMs, first size WSL memory/swap/host disk headroom appropriately; use `scripts/run_full_tests.sh --wsl-safe --target-dir /tmp/iroha-wsl-tests` only when you still need a conservative local run. This mode runs non-integration workspace tests one package at a time, sets `CARGO_INCREMENTAL=0`, serializes network tests, writes resource snapshots to `<target-dir>/run_full_tests_resources.log`, and refuses to start the next Cargo step when `MemAvailable` is below `4096` MiB unless overridden with `--min-available-mib`.
- For faster local full runs, `scripts/run_full_tests.sh --fast` routes all cargo calls through `scripts/cargo_fast.sh`; add `--fast-zero-debug` and `--no-incremental` when you want the more aggressive local-throughput mode.

## Fixtures
- IVM bytecode fixtures refresh automatically via `build.rs` when tests run.
- Regenerate SoraFS gateway fixtures: `cargo run -p integration_tests --features dev-tools --bin sorafs-gateway-fixtures -- --out fixtures/sorafs_gateway`.
- Regenerate grouped `nexus_and_streaming` Norito instruction + streaming goldens:
  `cargo run -p integration_tests --features dev-tools --bin refresh_nexus_streaming_fixtures`.

## Notes
- Pipeline block rejection scaffold lives at `tests/pipeline_block_rejected.rs` inside the `core_api` harness and is `#[ignore]` until a deterministic trigger is available.
- Canonical Jindo activation, pre-activation rejection, exact replay, and
  restarted-peer catch-up coverage lives in
  `tests/privacy_exact12_jindo_network.rs` inside the
  `privacy_release_network` harness.
- Canonical native Orchard and PQ-MASP proving, four-peer DA/RBC convergence,
  pre-activation and corrupted-proof rejection, stable-nullifier and exact
  transaction replay, failure atomicity, and a fresh nullifier replay through
  the restarted peer to authenticate recovered PQ state live in
  `tests/privacy_exact12_orchard_pq_masp_network.rs`. The fixture builders are
  non-shipping and require the explicit feature. Run the exact release gate
  with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test privacy_release_network --features 'zk-stark privacy-release-evidence' privacy_exact12_orchard_pq_masp_network::canonical_orchard_and_pq_masp_actions_survive_four_peer_da_replay_and_restart -- --exact --nocapture --test-threads=1`.
- Canonical native Anonymous PGC, VeRange, Bootle/Lantern, FCMP++, and
  private-IVM proving, exact governed activation, independently corrupted
  proofs, cross-profile proof substitution, wrong statement binding,
  pre-activation rejection, exact and stable-state replay, public-state
  atomicity, four-peer DA/RBC finality, and restarted-validator recovery live
  in `tests/privacy_exact12_retained_network.rs`. The same suite binds its
  ZK-ACE expectation to the native-stage availability constant, leaves that
  separately qualified protocol unactivated, and proves its probes do not
  change public state. Every retained-engine setup action and proof traverses
  the production native executor path; the fixture builders are non-shipping
  and require the explicit feature. Run the
  enforced release gate with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test privacy_release_network --features 'zk-stark privacy-release-evidence' privacy_exact12_retained_network::canonical_retained_exact12_actions_survive_four_peer_adversarial_replay_and_restart -- --exact --nocapture --test-threads=1`.
- Canonical native ZK-AMS and Vega proving, governed activation, corrupted
  statement/proof rejection, exact replay, four-validator finality, and
  restarted-validator recovery live in
  `tests/privacy_exact12_zk_ams_vega_network.rs`. The test is an enforced
  release-evidence gate (not ignored) and runs with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test privacy_release_network --features 'zk-stark privacy-release-evidence' privacy_exact12_zk_ams_vega_network::canonical_zk_ams_and_vega_actions_survive_four_validator_activation_replay_and_restart -- --exact --nocapture --test-threads=1`.
- Governed ZK-X509 trust-anchor, certificate-policy, signed-CRL, native
  certificate-presentation, certificate-nullifier replay, four-peer finality,
  and cold-restart coverage live in
  `tests/privacy_exact12_zk_x509_network.rs`. This production-action gate has
  no unavailable-as-success branch: it refuses before network startup until
  the authenticated KAT/resource evidence and immutable worker inputs make the
  compiled profile available. Run it with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test privacy_release_network --features 'zk-stark privacy-release-evidence' privacy_exact12_zk_x509_network::canonical_zk_x509_action_survives_four_peer_activation_replay_and_restart -- --exact --nocapture --test-threads=1`.
- The canonical exact-12 privacy-registry governance gate lives in
  `tests/privacy_exact12_activation_network.rs` inside the
  `privacy_release_network` harness. It covers compiled-ready profiles plus
  explicit fail-closed rows for evidence-gated profiles, unauthorized and
  malformed registrations, exact activation lead time, three-of-four
  activation while a validator is stopped, cold Proposed-state recovery,
  metadata-distinct duplicate rejection, exact replay rejection, and final
  restart/catch-up. It is not an all-12 release pass while any profile remains
  unavailable. Run it with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test privacy_release_network --features 'zk-stark privacy-release-evidence' privacy_exact12_activation_network::canonical_exact12_governance_survives_four_peer_activation_replay_and_restart -- --exact --nocapture --test-threads=1`.
- Kagemusha local four-wallet driver coverage exercises top-up, offline
  multihop split/change, restart recovery, and exact redemption under Halo2
  verification. It does not replace the still-required four-validator native
  top-up/redemption plus peer-handoff/replay/policy integration gate.
- SoraNet web deploy + public DNS ALIAS/CNAME + NS/DS delegation placeholders coverage lives at `tests/soranet_web_deploy.rs`.
