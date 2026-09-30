# Sumeragi handoff implementation and validation, 2026-09-30

This records completion of the interrupted local implementation candidate on
macOS arm64. Existing changes were preserved. No release was sealed, no live
network was reset, and no release or settlement readiness is inferred.

## Completed source changes

- Simulator sweeps release each completed World before constructing the next.
  F35's four known failing seeds remain pinned. F17's catch-up window includes
  the actual two-round-trip execution, durable-write and apply costs; its
  10,000-height catch-up assertion is unchanged.
- Beacon producer/replay fixtures admit a canonical Parliament attempt instead
  of directly populating its derived pulse index. Early positive pulse heights
  use a valid positive predecessor request height. Source-contract mutations
  require the producer and replay fixtures to retain canonical admission.
- The multilane scaling gate charges status requests and activation transactions
  to one deadline and rejects malformed lane readiness records.
- Native Evidence, Defect and canonical execution-result layouts participate in
  the compiled release wire identity. Composite enum schemas are derived from
  their actual fields through schema-only variant identities; evidence wire
  bytes are unchanged. Closed-root and field/discriminant mutation tests bind
  the hash.
- Canonical execution results encode each full current/boundary context once
  and derive Ready successors from that authorized context. The former repeated
  graph exceeded the 64 KiB ceiling for a valid 31-validator boundary with frozen
  preparation. The sole first-release encoding keeps the ceiling and complete
  credentials, rejects contradictory successors, and charges reconstructed
  storage to the inherited decoder allocation budget. There is no old decoder.
  Both active shared finality checkpoints were regenerated in three identical
  captures with retained exporter/log hashes; historical fixture originals were
  preserved.
- Public reset creates the one-use fresh-key token with its authorization-marked
  fresh State. Start and recovery never rearm it. Process attestation, successor
  process transitions, signed predecessor rollback and the exact State-root
  closure understand the native runtime artifacts. Predecessor capture and
  admission bind the exact signed active configuration, including beacon.toml
  after beacon activation; arbitrary configuration paths are rejected.
- Retired telemetry test calls are replaced with current per-instance latency
  samples. The telemetry specification names the registered native series.
  A test-only allocation accessor keeps private-lane fixtures on their original
  State pool; production State visibility is unchanged.
- Mochi validates overlays against the node's configuration schema. Its generated
  genesis test stub binds execution-derived policy through the existing generation
  boundary, then uses strict execution and bundle validation. The prepared signer
  still rejects mismatched commitments and resolved identities cannot be rebound.
- Current status distinguishes the implemented World accumulator and signed RS16
  integration from authenticated accelerated restoration and network qualification.

## Local validation

Completed results are scoped to these commands, rather than the full workspace:

| Check | Result |
| --- | --- |
| Schema and schema-derive suites, including trybuild | 21 integration tests, 10 unit tests and the UI suite passed |
| Strict Clippy for schema and schema-derive, all targets, no dependencies | Passed |
| Full release-mode Sumeragi library with `sim` | 408 passed, 2 existing stress/report tests ignored |
| Evidence and Defect schema tags against the actual codec | Passed |
| Core commitment, release identity, beacon/replay, World root and allocation fixtures | 69 tests passed |
| Data-model finality, compact result, caller decode budgets and wire-schema controls | 60 passed, 1 explicit exporter ignored |
| Recompiled shared-fixture consumers plus finality/schema controls | 62 native-bridge and 85 data-model tests passed; 1 explicit exporter ignored |
| Public-reset first boot, occupied predecessor, capture and admission | 30 tests passed |
| Shared canonical finality checkpoints | 3 byte-identical captures; H1 27,575 bytes, H2 31,792 bytes |
| Mochi configuration/supervision with the GUI feature enabled | 30 core tests and 29 GUI configuration tests passed |
| Kagami native launcher consumption order | 2 tests passed |
| Generated-genesis preparation and unchanged strict signing boundaries | 4 tests passed |
| F35 local-queue asymmetry | 1,000 seeds passed; original four regression seeds retained |
| F17 corrected catch-up scenario | All 20 default seeds passed |
| Sumeragi instance telemetry | 7 unit tests passed |
| Scaling gate | 67 tests passed |
| Parliament source guards and soak CLI/fault/log/run controls | 341 tests passed |
| Settlement release runner | 38 tests and 8,567 subtests passed |
| Python preflight and native status/lane DTO controls | 110 tests passed with the existing local ABI-25 wheel |
| Public-reset release-check/source-inventory controls | 286 tests and 4,522 subtests passed |
| Grafana alert pack | Promtool checked all four rules and passed the rule tests |
| Parliament source contract, panic-recovery inventory and retired-codec guards | Passed |
| Workspace formatting and original project-history verification | Passed |

The affected-target compilation passed for Core, CLI, daemon, Kagami, schema
generator and both Mochi crates. Subsequent rebuilt Core, CLI, data-model,
native-bridge, GUI and generated-genesis tests passed after the final source and
fixture repairs. The dedicated genesis-helper graph also rebuilt Core, Torii and
daemon dependencies. These are focused checks, not a full-workspace test run.

The main Cargo invocations were:

```sh
CARGO_TARGET_DIR=target/wf-sim cargo test -p iroha_sumeragi --features sim --release --lib
CARGO_TARGET_DIR=target/wf-core-tests cargo test -p connect_norito_bridge -p iroha_data_model --lib -- sumeragi_finality wire_schema kagemusha_mobile_bootstrap kagemusha_reserve_finality kagemusha_testnet_finality_chain kagemusha_testnet_native_mint_runtime validation_fee_policy_proof_bridge --test-threads=4
CARGO_TARGET_DIR=target/wf-sdk2 cargo test -p mochi-core -p mochi-ui -p iroha_kagami --features mochi-ui/gui --lib --bins -- sumeragi invalid_first_release_inputs_fail_before_creating_the_data_root supervisor_exposes_config_overrides config::tests taira_launcher_keeps_consumed_launch_paths_in_either_consumption_order --test-threads=4
CARGO_TARGET_DIR=target/wf-sdk2 cargo test -p mochi-core -p mochi-ui -p iroha_kagami -p iroha_test_network --features mochi-ui/gui --lib --bins -- genesis_support::tests --test-threads=4
```

Core and CLI selections were also run directly from the fresh compiled test
harnesses. Detailed local logs are under `/tmp/codex-sumeragi-*.log`; the checkpoint
exporter's retained binary and three logs are identified by
`fixtures/sumeragi/native-finality/capture.json` with exact hashes and an explicit
unclean-source marker. This handoff run did not create a commit or a release
artifact. While the final checks completed, the implementation appeared in
commit `325be07de4` (`update sumeragi`); this final evidence update was written
afterward. The tests and retained exporter ran from the preceding working tree,
not from a sealed release build of that commit.

## Outstanding qualification and wider implementation

- Current-source four-peer restart/fault and multilane throughput/latency runs,
  24-hour soaks at 4 and 22 validators, and the complete recorded 10,000-seed
  simulator/mutation qualification remain required.
- Authenticated accelerated snapshot restoration and native dataspace AMX hosting
  remain implementation goals in `specs/sumeragi_goals.md`. The complete World
  accumulator does not authenticate all restored State and native history.
- The JavaScript SDK run passed 74 tests and failed 15 because its native addon
  was absent. It requires a source-sealed native build and rerun. The cached
  Python wheel used here does not qualify same-source native delivery.
- Full-workspace and complete affected-crate runtime matrices remain unrun.
  Strict Sumeragi Clippy remains blocked by existing dependency/crate lints;
  this run did not suppress them or claim that lint gate passed.
