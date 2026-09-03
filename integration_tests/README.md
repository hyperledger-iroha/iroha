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
- Native BPNG alias bootstrap retained-Kura coverage lives in
  `tests/alias_registry_bootstrap_network.rs` in `network_functional`. It requires
  four real NPoS validators, native paid SNS quotes/leases, future-height routing
  activation and an exact-owner bootstrap grant, then restarts the same peers
  with snapshots disabled, Strict Kura and only an additive BPNG dataspace
  catalog entry. It checks exact leases, domains, parameters, balances and
  transaction results, plus original stored SignedBlock execution plans and
  genuine three-of-four CommitQCs before/after replay. Every peer must also
  demonstrate the live static-catalog addition through the SNS account-alias
  parser and retain its exact validated lane catalog and incarnation roots.
  After the second Strict restart it commits a new BPNG successor, proves the
  predecessor hash/height and lane-incarnation link on all four peers, and
  checks the stopped Kura/CommitQC evidence for exactly one appended certified
  BPNG lane block. Missing binaries, networking or persisted evidence fail; no
  success-by-skip is accepted.
  Its pre-activation universal-domain alias is a historical control, **not**
  qualification of a private-to-universal routing transition; genuine historical
  private-lane replay remains a separate release prerequisite. Ordinary
  transaction fees are zero from the original test genesis to isolate real SNS
  lease charges, so this is not production-fee qualification. Existing-file-only
  Fast Kura inspection verifies finality without starting a writer; only the
  actual Strict daemon restart qualifies replay.
  This scenario is release-only: the authenticated release bootstrap launches
  the sealed child in `scripts/run_sumeragi_v2_release_gates.sh --release`, which
  publishes the source/lock/toolchain-bound prebuilt bundle before re-discovering
  the exact non-ignored test and running its cooperative gate with one network at
  a time and one startup attempt. Do not substitute a standalone Cargo command
  or hand-supplied executable paths; they do not provide the required sealed
  identity, invocation root, source manifest or prebuilt-binary attestation.
  Source wiring is not runtime qualification: a clean signed release candidate
  still has to complete that gate. The scenario neither builds child binaries
  nor substitutes a fresh chain/store for retained replay.
- Pipeline block rejection scaffold lives at `tests/pipeline_block_rejected.rs` inside the `core_api` harness and is `#[ignore]` until a deterministic trigger is available.
- Canonical Jindo activation, pre-activation rejection, exact replay, and
  restarted-peer catch-up coverage lives in
  `tests/privacy_exact12_jindo_network.rs` inside the `network_functional`
  harness.
- Canonical native Orchard and PQ-MASP proving, four-peer DA/RBC convergence,
  pre-activation and corrupted-proof rejection, stable-nullifier and exact
  transaction replay, failure atomicity, and a fresh nullifier replay through
  the restarted peer to authenticate recovered PQ state live in
  `tests/privacy_exact12_orchard_pq_masp_network.rs`. The fixture builders are
  non-shipping and require the explicit feature. Run the exact release gate
  with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test network_functional --features 'zk-stark privacy-release-evidence' privacy_exact12_orchard_pq_masp_network::canonical_orchard_and_pq_masp_actions_survive_four_peer_da_replay_and_restart -- --exact --nocapture --test-threads=1`.
- Canonical native Anonymous PGC, VeRange, Bootle/Lantern, FCMP++, and
  private-IVM proving, exact governed activation, independently corrupted
  proofs, cross-profile proof substitution, wrong statement binding,
  pre-activation rejection, exact and stable-state replay, public-state
  atomicity, four-peer DA/RBC finality, and restarted-validator recovery live
  in `tests/privacy_exact12_retained_network.rs`. The same suite proves that
  ZK-ACE remains unavailable on every peer and that its production builder
  fails closed without changing public state. Every available-engine setup
  action and proof traverses the production native executor path; the fixture
  builders are non-shipping and require the explicit feature. Run the
  enforced release gate with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test network_functional --features 'zk-stark privacy-release-evidence' privacy_exact12_retained_network::canonical_retained_exact12_actions_survive_four_peer_adversarial_replay_and_restart -- --exact --nocapture --test-threads=1`.
- The retained positive ZK-AMS/Vega acceptance suite in
  `tests/privacy_exact12_zk_ams_vega_network.rs` covers canonical native
  proving, governed activation, corrupted statement/proof rejection, exact
  replay, four-validator finality, and restarted-validator recovery once both
  compiled profiles are releasable. It remains an enforced release-evidence
  gate (not ignored): while either production profile is unavailable, the
  required-network command must fail at the compiled-profile boundary before
  activation or a passing evidence marker. Run that gate with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test network_functional --features 'zk-stark privacy-release-evidence' privacy_exact12_zk_ams_vega_network::canonical_zk_ams_and_vega_actions_survive_four_validator_activation_replay_and_restart -- --exact --nocapture --test-threads=1`.
- Governed ZK-X509 trust-anchor, certificate-policy, and signed-CRL dependency
  ordering, unavailable-profile activation refusal, candidate-action refusal,
  exact four-peer convergence of every rejection, substituted
  anchor/policy/CRL candidate references at the outer unavailable-protocol
  boundary, and cold-restart persistence live in
  `tests/privacy_exact12_zk_x509_network.rs`. This gate intentionally asserts
  that the profile remains unavailable until real KAT/resource evidence is
  pinned; it does not claim reference-specific native proof verification and
  must be replaced by canonical acceptance and nullifier-replay coverage when
  the native network action builder is released. Run it with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test network_functional --features 'zk-stark privacy-release-evidence' privacy_exact12_zk_x509_network::zk_x509_governance_and_unreleased_actions_fail_closed_across_four_peer_restart -- --exact --nocapture --test-threads=1`.
- The complete canonical exact-12 privacy-registry release gate lives in
  `tests/privacy_exact12_activation_network.rs` inside the
  `network_functional` harness. It covers unauthorized and malformed
  registrations, exact activation lead time, three-of-four activation while a
  validator is stopped, cold Proposed-state recovery, metadata-distinct
  duplicate rejection, exact replay rejection, and final restart/catch-up.
  Run it with
  `TEST_NETWORK_IROHAD_FEATURES=zk-stark IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_SERIALIZE_NETWORKS=1 cargo test --locked -p integration_tests --test network_functional --features zk-stark privacy_exact12_activation_network::canonical_exact12_governance_survives_four_peer_activation_replay_and_restart -- --exact --nocapture --test-threads=1`.
- KAGEMUSHA V1 lifecycle coverage exercises pooled-reserve top-up,
  aggregate-balance receipt folding, restart recovery, unrestricted subsequent
  payment, and full or partial redemption under paired-Pasta verification.
- SoraNet web deploy + public DNS ALIAS/CNAME + NS/DS delegation placeholders coverage lives at `tests/soranet_web_deploy.rs`.
