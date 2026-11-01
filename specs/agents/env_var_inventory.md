# Environment toggle inventory

_Last refreshed via `python3 scripts/inventory_env_toggles.py --json specs/agents/env_var_inventory.json --md specs/agents/env_var_inventory.md`_

Total references: **1197** · Unique variables: **309**

## ADVICE_SCHEDULE (test: 1)

- test: crates/iroha_plonk_gadgets/tests/m3_gates/advice_schedule.rs — `let mode = std::env::var("ADVICE_SCHEDULE").unwrap();`

## ADVICE_WORKLOAD (test: 1)

- test: crates/iroha_plonk_gadgets/tests/m3_gates/advice_schedule.rs — `let workload = std::env::var("ADVICE_WORKLOAD").unwrap();`

## CARGO (test: 5)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/policy/cargo_image_tests.rs — `let selected = std::env::var("CARGO").unwrap();`
- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/policy/cargo_image_tests.rs — `let cargo = std::env::var("CARGO").unwrap();`
- test: crates/iroha_test_network/src/lib.rs — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- test: crates/norito_derive/tests/ui.rs — `let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());`
- test: integration_tests/src/kagami.rs — `let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## CARGO_BIN_EXE_attachment_sanitizer (test: 9)

- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let mut cmd = Command::new(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`

## CARGO_BIN_EXE_iroha (test: 2)

- test: crates/iroha_cli/bins/tests/cli_smoke.rs — `env!("CARGO_BIN_EXE_iroha")`
- test: crates/iroha_cli/bins/tests/taikai_policy.rs — `env!("CARGO_BIN_EXE_iroha")`

## CARGO_BIN_EXE_iroha_monitor (test: 1)

- test: crates/iroha_monitor/tests/smoke.rs — `PathBuf::from(env!("CARGO_BIN_EXE_iroha_monitor"))`

## CARGO_BIN_EXE_kagami (test: 3)

- test: crates/iroha_kagami/tests/common/mod.rs — `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- test: crates/iroha_kagami/tests/pop_embed.rs — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- test: integration_tests/src/kagami.rs — `if let Some(path) = env::var_os("CARGO_BIN_EXE_kagami") {`

## CARGO_BIN_EXE_koto (test: 7)

- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `Command::new(env!("CARGO_BIN_EXE_koto"))`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `let built = Command::new(env!("CARGO_BIN_EXE_koto"))`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `let documented = Command::new(env!("CARGO_BIN_EXE_koto"))`

## CARGO_BIN_EXE_sorafs_chunk_dump (test: 1)

- test: crates/sorafs_chunker/tests/one_gib.rs — `let chunk_dump_path = env!("CARGO_BIN_EXE_sorafs_chunk_dump");`

## CARGO_BIN_EXE_sorafs_fetch (test: 1)

- test: crates/sorafs_car/tests/sorafs_fetch_cli.rs — `AssertCommand::new(env!("CARGO_BIN_EXE_sorafs_fetch"))`

## CARGO_BUILD_JOBS (test: 2)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/environment.rs — `assert_eq!(std::env::var("CARGO_BUILD_JOBS").unwrap(), "1");`
- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs — `std::env::var("CARGO_BUILD_JOBS").ok().as_deref() == Some("1")`

## CARGO_BUILD_TARGET (tool: 2)

- tool: xtask/src/poseidon_bench.rs — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- tool: xtask/src/stage1_bench.rs — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_FEATURE (prod: 1)

- prod: crates/build-support/src/lib.rs — `let parsed_features = env::var("CARGO_CFG_FEATURE")`

## CARGO_CFG_TARGET_ARCH (build: 2, prod: 2, tool: 2)

- build: crates/gpuzstd_metal/build.rs — `let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- build: crates/norito/accelerators/jsonstage1_metal/build.rs — `let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();`
- tool: xtask/src/poseidon_bench.rs — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- tool: xtask/src/stage1_bench.rs — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## CARGO_CFG_TARGET_OS (build: 6, prod: 2, tool: 2)

- build: crates/connect_norito_bridge/build.rs — `std::env::var("CARGO_CFG_TARGET_OS").as_deref(),`
- build: crates/fastpq_prover/build.rs — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/gpuzstd_cuda/build.rs — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/gpuzstd_metal/build.rs — `let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/norito/accelerators/jsonstage1_metal/build.rs — `let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- tool: xtask/src/poseidon_bench.rs — `os: std::env::var("CARGO_CFG_TARGET_OS")`
- tool: xtask/src/stage1_bench.rs — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## CARGO_ENCODED_RUSTFLAGS (build: 9)

- build: crates/iroha/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`
- build: crates/iroha_core/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`
- build: crates/iroha_data_model/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`
- build: crates/iroha_deploy/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`
- build: crates/iroha_p2p/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`
- build: crates/iroha_sumeragi/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`
- build: crates/iroha_torii/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`
- build: crates/irohad/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`
- build: crates/norito/build.rs — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`

## CARGO_FEATURE_CUDA_KERNEL (build: 2)

- build: crates/gpuzstd_cuda/build.rs — `if env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_none() {`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (build: 1)

- build: crates/fastpq_prover/build.rs — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## CARGO_FEATURE_METAL (build: 1)

- build: crates/ivm/build.rs — `&& env::var_os("CARGO_FEATURE_METAL").is_some()`

## CARGO_FEATURE_MUTATION_TESTING (build: 9)

- build: crates/iroha/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`
- build: crates/iroha_core/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`
- build: crates/iroha_data_model/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`
- build: crates/iroha_deploy/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`
- build: crates/iroha_p2p/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`
- build: crates/iroha_sumeragi/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`
- build: crates/iroha_torii/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`
- build: crates/irohad/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`
- build: crates/norito/build.rs — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`

## CARGO_HOME (build: 1)

- build: crates/iroha_release_bundle/src/bin/iroha_release_bundle/build.rs — `let home = env::var_os("CARGO_HOME").unwrap();`

## CARGO_INCREMENTAL (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs — `&& std::env::var("CARGO_INCREMENTAL").ok().as_deref() == Some("0")`

## CARGO_MANIFEST_DIR (bench: 2, build: 4, example: 1, prod: 54, test: 532, tool: 6)

- prod: crates/build-support/src/lib.rs — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);`
- prod: crates/connect_norito_bridge/src/bin/swift_parity_regen.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/bridge_tail_tests.rs — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/bridge_tail_tests.rs — `const BRIDGE_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));`
- test: crates/connect_norito_bridge/src/kagemusha_wallet_ffi/background/native_maintenance_tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/kagemusha_wallet_load_original/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/sorafs_tests.rs — `fs::read(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))`
- test: crates/connect_norito_bridge/src/validation_fee_policy_proof_bridge_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/validation_fee_policy_proof_bridge_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/fastpq_prover/src/backend/compact_public_api.rs — `let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/fastpq_prover/src/backend/compact_public_diagnostic.rs — `let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/src/backend/compact_public_transfer.rs — `let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/src/backend/compact_quantity_artifact_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/fastpq_prover/src/backend/compact_quantity_diagnostic.rs — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/fastpq_prover/src/backend/compact_quantity_diagnostic.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/fastpq_prover/src/backend/compact_quantity_producer/tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/src/backend/deep_fixture/frozen_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")`
- prod: crates/fastpq_prover/src/backend/deep_prover/diagnostic_artifact.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/fastpq-proof-diagnostics")`
- prod: crates/fastpq_prover/src/backend/frozen_record.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")`
- test: crates/fastpq_prover/src/bin/fastpq_cuda_bench.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/src/bin/fastpq_cuda_bench.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");`
- prod: crates/fastpq_prover/src/poseidon_manifest.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs — `let field_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/field.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- test: crates/fastpq_prover/tests/shared_backend_inventory.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/support/offline_compact_capture.rs — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/support/offline_compact_single.rs — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/support/offline_compact_two.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/fastpq-proof-diagnostics");`
- test: crates/fastpq_prover/tests/trace_commitment.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- test: crates/fastpq_prover/tests/transcript_replay.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/client.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha/src/client/kagemusha_http_tests/finality.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/client/operation_contract.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/client/sumeragi_api_separation_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha/src/client/workflow_ledger_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/sm.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/workflow/journal_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/tests/sm_signing.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/bins/tests/cli_smoke.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/bins/tests/cli_smoke.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/bins/tests/sorafs_validate_cli.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/commands/sorafs.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/commands/sorafs/toolkit/validation/final_promotion_receipt_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/commands/sorafs/toolkit/validation/tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/compute.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/sccp/administration/control_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- test: crates/iroha_cli/src/sccp/deployment/tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- test: crates/iroha_cli/src/sccp/finalization.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- test: crates/iroha_cli/src/sccp/flows.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- test: crates/iroha_cli/src/sccp/origin_record/receipt_adapter_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/sccp/record.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- test: crates/iroha_cli/src/sccp/ton.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- test: crates/iroha_cli/src/sccp/ton_binding_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- test: crates/iroha_cli/src/sccp/ton_state/causal_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/sccp/ton_state/causal_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- test: crates/iroha_cli/src/sccp/ton_state/causal_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sccp-continuation");`
- prod: crates/iroha_cli/src/soracloud.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/soracloud/tests/part_01.rs — `let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");`
- test: crates/iroha_cli/src/soracloud/tests/part_01.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/soracloud/tests/part_01.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/soracloud/tests/part_01.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/taira.rs — `.tempdir_in(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/taira.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/taira_public_reset.rs — `let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");`
- test: crates/iroha_cli/src/taira_public_reset_host.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/taira_public_reset_host.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_config/src/parameters/user.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/src/parameters/user.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/src/parameters/user/kagemusha_enrollment/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_config/tests/autoscale_config.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/checked_in_profiles_parse.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/connect_relay_strategy_hard_cut.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/da_ingest_compute_limit.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/fastpq_queue_overrides.rs — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/kaigi_authorization_config_v1.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml"),`
- test: crates/iroha_config/tests/kura_retention_hard_cut.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/minamoto_profile.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/native_context_archive_limit.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/network_scion_hard_cut.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/nexus_staking_bounds.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/nexus_staking_withdraw_grace_hard_cut.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/operator_auth_bootstrap_hard_cut.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/p2p_hard_cut.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_signature_batch_alias_hard_cut.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/push_provider_credentials.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/queue_plan_retirement.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml"),`
- test: crates/iroha_config/tests/sccp_node_config.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml")`
- test: crates/iroha_config/tests/sorafs_gateway_runtime_providers.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_gateway_runtime_providers.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/compliance-checkpoint.nrt");`
- test: crates/iroha_config/tests/sorafs_governance_dag_runtime_signer.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_native_transaction_signers.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_por_replay_archive.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_provider_ingest_finalized_archive.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_reputation_finalized_archive.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_storage_pin_aliases.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_stream_token_runtime_signer.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/soranet_privacy_ingest_hard_cut.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sumeragi_core_config.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- test: crates/iroha_config/tests/transaction_gossip_config.rs — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/transaction_ingress_limits.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/trusted_peers_pop_validation.rs — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/zk_validity_env_independence.rs — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- bench: crates/iroha_core/benches/blocks/common.rs — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- bench: crates/iroha_core/benches/validation.rs — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- example: crates/iroha_core/examples/generate_parity_fixtures.rs — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/src/block.rs — `let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/executor_contract_dispatch_tests.rs — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- test: crates/iroha_core/src/executor_tests.rs — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- test: crates/iroha_core/src/fastpq/mod.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/kagemusha_wallet_v1/enrollment_issuer/tests.rs — `let temporary = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/query/final_promotion_authority/observation/tests/fixture.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/query/provider_ingest_finalized/tests/frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/query/reputation_finalized/tests/frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/runtime/catalog/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/asset/core_numeric_mutation_tests.rs — `let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/smartcontracts/isi/musubi/initial_publication_admission_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/musubi/rejection_allocation_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_core/src/smartcontracts/isi/sccp/fast_pause/certification/reference_fixtures.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/fast_pause_v1.json")`
- prod: crates/iroha_core/src/smartcontracts/isi/sccp/governance/reference_fixtures.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/governance_v1.json")`
- test: crates/iroha_core/src/smartcontracts/isi/sccp/recipients.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_core/src/smartcontracts/isi/sccp/reference_fixture_support.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/sorafs_final_promotion_authority/tests/check.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_core/src/smartcontracts/isi/sorafs_provider_admission/test_fixture.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/triggers/set_frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/ivm/host.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/ivm/host.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/ivm/host_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_core/src/state.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- prod: crates/iroha_core/src/state/authority_registry/inventory_test_support.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/state/authority_registry/keyed_commitment/candidates_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/state/authority_registry/keyed_commitment/tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/sumeragi.md"),`
- test: crates/iroha_core/src/state/block_proof_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/state/state_table_inventory_tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")`
- test: crates/iroha_core/src/state/world_state_snapshot_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/streaming.rs — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/src/sumeragi/node/tests/history_cutover_tests.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/sumeragi/node/tests/p2p_owner_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_core/src/sumeragi/test_chain/kagemusha_setup/registration/export.rs — `let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/qualification");`
- test: crates/iroha_core/src/tx/sandbox_state_tests.rs — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/default_domain_independence.rs — `let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/pin_registry.rs — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);`
- test: crates/iroha_core/tests/pin_registry.rs — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/snapshots.rs — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/sumeragi_doc_sync.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_core_privacy/src/privacy_engines/zk_x509/engine_prover_diagnostic.rs — `let repository = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_core_privacy/src/privacy_engines/zk_x509/engine_prover_diagnostic.rs — `let repository = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_core_privacy/src/privacy_engines/zk_x509/engine_prover_diagnostic.rs — `let repository = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_privacy/tests/state_commitment_candidates_measurement.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/state_table_inventory.json");`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/credential.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/credential.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/credential.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/issuer_worker_configuration_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/issuer_worker_configuration_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/issuer_worker_protocol_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/issuer_worker_protocol_tests.rs — `let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/issuer_worker_protocol_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/tests.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/tests/native_exchange.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/tests/native_exchange/message_census.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/tests/native_exchange/message_census.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/tests/native_exchange/message_census.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/tests/native_exchange/send_recovery.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_intake_v1/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_intake_v1/tests/credential_source.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_preparation_v1/dispatch.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_preparation_v1/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/lifecycle/intake_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/lifecycle/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/manifest/blacklists/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/native_owner/request_fee.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/native_owner/review/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/stark/bfv_full_bootstrap_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/stark/frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_crypto/src/ram_lfe/phone_control_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_crypto/src/ram_lfe/specification_tests.rs — `let full = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/src/signature/bls/consensus/tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/src/signature/bls/consensus/tests/python_reference.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/src/signature/bls/consensus/tests/python_reference.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_crypto/tests/confidential_keyset_vectors.rs — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm2_fixture_vectors.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm_cli_matrix.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs — `.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")),`
- test: crates/iroha_data_model/src/bin/axt_fixtures.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- prod: crates/iroha_data_model/src/bin/privacy_exact12_fixtures.rs — `let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/identifier.rs — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/isi/escrow.rs — `let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_data_model/src/isi/generated_record_identity_tests/sorafs_values.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/isi/governance.rs — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/kagemusha/enrollment_eligibility_v1/template.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/kagemusha/enrollment_eligibility_v1/tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1/poseidon_tests.rs — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1/test_fixture_cohort.rs — `return PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/qr_stream.rs — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_data_model/src/sorafs/provider_admission/discovery/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/sorafs/provider_admission/discovery/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/sorafs/reserve/captured_reserve_schema_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/soranet/vpn.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- test: crates/iroha_data_model/src/sumeragi_finality/certified_result/tests.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/sumeragi_finality/test_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/sumeragi_finality/test_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/sumeragi_finality/test_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/cancel_asset_lock.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed/ordered_intent_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed_norito_rpc_fixture_tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed_norito_rpc_fixture_tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/validation_fee/retail.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/validation_fee/retail.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/zk_validity_policy.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/account_address_vectors.rs — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/address_curve_registry.rs — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/bilateral_settlement_sdk_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/bilateral_settlement_sdk_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/kagemusha_activate_sdk.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/kagemusha_issue_load_sdk.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/block_signature.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/identifier.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/proof.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/protocol.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/scalar.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/phone_control_v1.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/phone_control_v1.rs — `let source = std::fs::read_to_string(format!("{}{path}", env!("CARGO_MANIFEST_DIR")))`
- test: crates/iroha_data_model/tests/ram_lfe_canonical_v1.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/validator_staking_sdk_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/validator_staking_sdk_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_deploy/src/localnet/bundle.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_deploy/src/localnet/bundle.rs — `let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/iroha_deploy/src/sora_network.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_deploy/src/sora_network.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_deploy/src/sora_network.rs — `|| PathBuf::from(env!("CARGO_MANIFEST_DIR")),`
- test: crates/iroha_deploy/tests/definition.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")`
- test: crates/iroha_deploy/tests/definition.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_fhe/tests/bgv_surface.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/genesis_tail_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/genesis_tail_tests.rs — `let genesis_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/lib.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);`
- test: crates/iroha_genesis/src/lib.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/lib.rs — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/staking_source_template_tests.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_i18n/src/lib.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- test: crates/iroha_js_codec/src/bond_instruction_tests.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_js_codec/src/manifest.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_js_codec/src/manifest.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_js_codec/src/manifest.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_js_host/src/lib.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_js_host/src/lib.rs — `let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_js_host/src/lib.rs — `let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_js_host/src/transaction_attachment_context/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/codec.rs — `concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec/account.json"),`
- test: crates/iroha_kagami/src/codec.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/codec.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/codec.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/genesis/generate.rs — `let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/generate.rs — `let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/generate.rs — `let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/prepared.rs — `let defaults = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `&PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/wizard.rs — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/tests/codec.rs — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- test: crates/iroha_kagemusha_proof/tests/common/mod.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")`
- test: crates/iroha_kagemusha_proof/tests/common/proof_fixtures/omega_hard_fixture.rs — `let p = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagemusha_proof/tests/common/proof_fixtures/omega_hard_fixture_export.rs — `let repository = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagemusha_proof/tests/common/proof_fixtures/omega_hard_fixture_export.rs — `let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/qualification");`
- test: crates/iroha_measurement/src/report.rs — `let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");`
- test: crates/iroha_measurement/src/schema.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_measurement/tests/layering.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_measurement/tests/layering.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/consumers.json"),`
- test: crates/iroha_measurement/tests/public_api.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/recorder.rs");`
- test: crates/iroha_model_base/src/name/scratch_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_model_base/src/name/scratch_tests.rs — `let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));`
- test: crates/iroha_musubi_service/src/tests/wire_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/src/frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/src/frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/src/frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/src/frame_identity_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/tests/production_source_reachability.rs — `let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_p2p/tests/production_source_reachability.rs — `let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_petal/tests/captures.rs — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_petal/tests/fixtures.rs — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_plonk/build_support/source_identity.rs — `r#"include!(concat!(env!("CARGO_MANIFEST_DIR"), "/file.rs"));`
- prod: crates/iroha_plonk/src/captured_goldens/export.rs — `let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_plonk/src/prover/quotient/tile_experiment.rs — `let repo = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_plonk/src/transcript/kat_tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_plonk/tests/pinned_params.rs — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);`
- test: crates/iroha_plonk/tests/source_identity.rs — `const OTHER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../python/client/native_input.py"));`
- test: crates/iroha_plonk_gadgets/src/bytes/tests.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_plonk_gadgets/src/p256/tests.rs — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_plonk_gadgets/src/p256/tests.rs — `|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),`
- test: crates/iroha_plonk_gadgets/tests/common/mod.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")`
- test: crates/iroha_plonk_gadgets/tests/m3_gates/export_descriptors.rs — `let repo = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_plonk_gadgets/tests/pow5_fq_lane.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")`
- test: crates/iroha_sccp/src/api.rs — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_sccp/src/test_support/ton_capture.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/rpc/ton/transport")`
- test: crates/iroha_sccp/src/ton_native/minter_state/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_sccp/src/v1/finality.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_sccp/src/v1/ton_cell.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_sccp/tests/ethereum_light_client.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp")`
- test: crates/iroha_sccp/tests/reference_support/mod.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_sccp/tests/v1_vectors.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_sccp_rpc/src/builders/ethereum/evidence_tests.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_sccp_rpc/src/limits.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/rpc");`
- test: crates/iroha_sccp_rpc/tests/ton_liteclient.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/rpc/ton/transport")`
- test: crates/iroha_sccp_rpc/tests/transport.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/rpc/transport")`
- test: crates/iroha_sccp_wallet/src/pure/ton/golden_tests.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/ton_boc_v1.json")`
- test: crates/iroha_sccp_wallet/tests/evm_pure.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_sumeragi/tests/spec.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_swarm/tests/default_compose_soranet.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_test_network/src/lib.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- build: crates/iroha_test_samples/build.rs — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_torii/src/da/tests/replay_manifest_and_metrics.rs — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- test: crates/iroha_torii/src/identifier_resolution.rs — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/kagemusha_enrollment/observation/tests.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/kagemusha_enrollment/signer/tests.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/kagemusha_enrollment/worker/linux_tests.rs — `let repo = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/kagemusha_enrollment/worker/process.rs — `let root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/kagemusha_enrollment/worker/tests.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/openapi/tests/sorafs_contracts.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/openapi/tests/vpn_da.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing/pipeline_preflight_fixture_tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/soracloud.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/admission.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/tests/lib_routed_reads/routed_read_source_bounds.rs — `let mut pending = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];`
- test: crates/iroha_torii/src/zk_attachments/tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/tests/accounts_portfolio.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/tests/sorafs_discovery.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii_shared/src/list_query/vectors_tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/torii/list_query/vectors.json")`
- test: crates/iroha_zkp_halo2/src/vega/canonical_mc_exact.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/prover_key.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verifier_key.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verifier_key.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verify.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verify.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/incremental_source_phase23_tests.rs — `let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/vega/zk_ams/mkhe");`
- test: crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/phase23_rns_link.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/vega/zk_ams/mkhe");`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs — `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega/engine.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs — `const FACADE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs — `const CRATE_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs — `const VEGA_FACADE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_vole/tests/common/mod.rs — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_zkp_vole/tests/conformance_crt.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/crt/generated")`
- test: crates/iroha_zkp_vole/tests/conformance_crt.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/bins/src/bin/sorafs_external_software_signer.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/irohad/src/external_software_signer/consensus_threshold.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main/node_file_tests.rs — `.tempdir_in(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main/shared_sorafs_provider_cache_tests.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/node_secrets/tests.rs — `.tempdir_in(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/runtime_provider_registry.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime/tests/part_01.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime/tests/part_01.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/sorafs_provider_ingest_finalized_query.rs — `let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");`
- test: crates/irohad/src/taira_runtime_signer.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- build: crates/ivm/build.rs — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- build: crates/ivm/build.rs — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- prod: crates/ivm/src/bin/gen_abi_hash_doc.rs — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: crates/ivm/src/bin/gen_header_doc.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_pointer_types_doc.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_proof_coverage_inventory.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_syscalls_doc.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/ivm_fixture_export.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/ivm_prebuild.rs — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/src/core_host.rs — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/ivm/src/predecoder_fixtures.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- prod: crates/ivm/src/proof_coverage/source_scan.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/axt_descriptor_builder.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm/tests/docs_consistency.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/ivm_abi_doc_sync.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/ivm_header_doc_sync.rs — `let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama.rs — `let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kotodama_lang/src/samples");`
- test: crates/ivm/tests/kotodama_argument_record.rs — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama_compact_emission.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/ivm/tests/kotodama_documentation_examples.rs — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/tests/kotodama_frame_emission.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/ivm/tests/kotodama_local_emission.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/ivm/tests/kotodama_numeric_zero.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/ivm/tests/kotodama_private_single_use.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/ivm/tests/numeric_v1_sdk_fixture.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/numeric_v1_sdk_fixture.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/pointer_types_doc_generated.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- test: crates/ivm/tests/pointer_types_doc_generated_ivm_md.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/repository_ivm_artifacts.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/syscalls_doc_generated.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_doc_sync.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_gas_names.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/tlv_examples.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm_abi/src/axt.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm_abi/src/axt.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm_artifact_admission/src/lib.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm_artifact_admission/src/policy/nominal_abort_tests.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/kotodama_lang/src/compiler/compact_emission/native_pairs.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/kotodama_lang/src/compiler/frame_emission/native_pairs.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/kotodama_lang/src/compiler/local_emission/native_pairs.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/kotodama_lang/src/compiler/numeric_zero/native_pairs.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/kotodama_lang/src/compiler/single_use_fixtures.rs — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/kotodama_lang/src/compiler/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/kotodama_lang/src/compiler/tests.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/kotodama_lang/src/diagnostic.rs — `let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/kotodama_lang/src/doc_consistency.rs — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/kotodama_lang/src/doc_consistency.rs — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/kotodama_lang/src/formatter/tests.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/kotodama_lang/src/syntax/lexer.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/kotodama_lang/tests/documentation_fences.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/kotodama_toolchain/src/bin/koto.rs — `let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/kotodama_toolchain/src/bin/koto.rs — `let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/kotodama_toolchain/src/bin/koto/explain.rs — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- build: crates/norito/build.rs — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- prod: crates/norito/src/bin/norito_regen_goldens.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/src/streaming/repo_fixture_test.rs — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/src/streaming/repo_fixture_test.rs — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/aos_ncb_more_golden.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- test: crates/norito/tests/json_golden_loader.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- test: crates/norito/tests/ncb_enum_iter_samples.rs — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- test: crates/norito/tests/ncb_enum_large_fixture.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/provider_admission_fixtures.rs — `let committed_dir = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_car/src/bin/soranet_trustless_verifier.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/reference.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/capacity_simulation_toolkit.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/da_reconstruct_cli.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/manifest_builder_cli.rs — `let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/taikai_viewer_cli.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/trustless_verifier.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_chunker/src/bin/export_vectors.rs — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_chunker/tests/backpressure.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_chunker/tests/vectors.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/gateway_compliance/captured_owner_identity_tests.rs — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference.rs — `let absolute = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference_ffi.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/orderbook_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/pdp_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/por_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/provider_admission_fixtures.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/replication_order_fixtures.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_node/tests/cli.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/src/lib.rs — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/tests/orchestrator_parity.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli/pdp.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/soranet_pq/tests/kat_vectors.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: integration_tests/src/bin/refresh_nexus_streaming_fixtures.rs — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: integration_tests/src/binary_resolver.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")`
- test: integration_tests/src/kagami.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/src/sorafs_gateway_capability_refusal.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- test: integration_tests/src/sorafs_gateway_conformance.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/alias_registry_bootstrap_network.rs — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kagemusha_network_setup/continuation/issuer/provisioning.rs — `let repository = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kagemusha_network_setup/continuation/issuer/provisioning/scopes/tests.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kagemusha_network_setup/continuation/issuer/provisioning/tests.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kagemusha_network_setup/continuation/issuer/provisioning/tests.rs — `let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();`
- test: integration_tests/tests/kotodama_examples.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/atomic_private_settlement_localnet.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: integration_tests/tests/nexus/cbdc_rollout_bundle.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/cbdc_whitelist.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/lane_registry.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/norito_burn_fixture.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/privacy_exact12_zk_x509_network.rs — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(RESOURCE_CERTIFICATE_RELATIVE_PATH);`
- test: integration_tests/tests/sccp_key_transition/main.rs — `let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/sccp_lanes/main.rs — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/sccp_lanes/main.rs — `let directory = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/sccp_lanes/main.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: integration_tests/tests/sccp_lanes/main.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: integration_tests/tests/sorafs_orchestrator_parity.rs — `let support = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support");`
- test: integration_tests/tests/sorafs_publication.rs — `let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target");`
- test: integration_tests/tests/streaming/mod.rs — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/compose.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs — `let checked = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs — `let checked = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/tests/readiness_smoke.rs — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- tool: tools/norito_codegen_exporter/src/norito_rpc.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: tools/soranet-handshake-harness/tests/fixtures_verify.rs — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/interop_parity.rs — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/perf_gate.rs — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- tool: xtask/src/bin/control_plane_mock.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/main.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/nexus.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/src/nexus_lane_maintenance.rs — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/sorafs/gateway_fixture.rs — `env!("CARGO_MANIFEST_DIR"),`
- tool: xtask/src/sorafs/gateway_fixture.rs — `env!("CARGO_MANIFEST_DIR"),`
- test: xtask/tests/address_vectors.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/android_dashboard_parity_cli.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/codec_rans_tables.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/da_proof_bench.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/iso_bridge_lint.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/ministry_agenda.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/mochi_bundle.rs — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soradns_cli.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/sorafs_fetch_fixture.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_bug_bounty.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_billing.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_m1.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_m2.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_bundle_check.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_entropy_bench.rs — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_NAME (test: 1)

- test: crates/norito_derive/tests/ui.rs — `let crate_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "norito_derive".to_owned());`

## CARGO_PKG_VERSION (prod: 19, test: 5, tool: 2)

- test: crates/iroha_cli/bins/tests/cli_smoke.rs — `let expected_version = env!("CARGO_PKG_VERSION");`
- prod: crates/iroha_cli/src/main_shared.rs — `env!("CARGO_PKG_VERSION"),`
- prod: crates/iroha_cli/src/main_shared.rs — `#[command(name = "iroha", version = env!("CARGO_PKG_VERSION"), author)]`
- prod: crates/iroha_core/src/release_identity.rs — `env!("CARGO_PKG_VERSION"),`
- prod: crates/iroha_js_host/src/lib.rs — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_kagami/src/genesis/generate.rs — `env!("CARGO_PKG_VERSION")`
- prod: crates/iroha_kagami/src/main.rs — `let version = env!("CARGO_PKG_VERSION");`
- test: crates/iroha_kagami/src/main.rs — `assert!(super::version_text().starts_with(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_kagami/src/verify.rs — `writeln!(writer, "kagami_version: {}", env!("CARGO_PKG_VERSION"))?;`
- prod: crates/iroha_sccp_rpc/src/http.rs — `const USER_AGENT: &str = concat!("iroha-sccp-rpc/", env!("CARGO_PKG_VERSION"));`
- prod: crates/iroha_storage_client/src/client.rs — `("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION"))),`
- prod: crates/iroha_torii/src/mcp.rs — `Value::String(env!("CARGO_PKG_VERSION").to_owned()),`
- prod: crates/iroha_torii/src/mcp/protocol.rs — `"version": (env!("CARGO_PKG_VERSION"))`
- prod: crates/irohad/src/main.rs — `version = env!("CARGO_PKG_VERSION"),`
- test: crates/irohad/src/main.rs — `env!("CARGO_PKG_VERSION"),`
- test: crates/irohad/src/main.rs — `env!("CARGO_PKG_VERSION"),`
- prod: crates/kotodama_lang/src/compiler.rs — `const COMPILER_FINGERPRINT: &str = concat!("kotodama_lang/", env!("CARGO_PKG_VERSION"));`
- prod: crates/kotodama_toolchain/src/bin/koto.rs — `const KOTO_VERSION: &str = env!("CARGO_PKG_VERSION");`
- test: crates/kotodama_toolchain/src/lsp/lsp_transport.rs — `Some(env!("CARGO_PKG_VERSION"))`
- prod: crates/kotodama_toolchain/src/lsp/mod.rs — `("version", env!("CARGO_PKG_VERSION").into()),`
- prod: crates/musubi/src/command.rs — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/sorafs_car/src/bin/sorafs_fetch.rs — `Value::from(env!("CARGO_PKG_VERSION")),`
- prod: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
- prod: mochi/mochi-ui-egui/src/main.rs — `println!("mochi {}", env!("CARGO_PKG_VERSION"));`
- tool: tools/sora-vpn-helper/src/main.rs — `const VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/telemetry-schema-diff/src/main.rs — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs — `&& std::env::var("CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS")`

## CARGO_PROFILE_RELEASE_CODEGEN_UNITS (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs — `&& std::env::var("CARGO_PROFILE_RELEASE_CODEGEN_UNITS")`

## CARGO_TARGET_DIR (build: 1, prod: 2, test: 6, tool: 1)

- prod: crates/iroha_deploy/src/localnet/bundle.rs — `let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(`
- prod: crates/iroha_deploy/src/sora_network.rs — `let target_dir = resolve_target_dir(&repo_root, env::var("CARGO_TARGET_DIR").ok().as_deref());`
- build: crates/iroha_release_bundle/src/bin/iroha_release_bundle/build.rs — `let release = Path::new(&env::var_os("CARGO_TARGET_DIR").unwrap()).join("@NATIVE@").join("release");`
- test: crates/iroha_test_network/src/lib.rs — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: crates/iroha_test_network/src/lib.rs — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/binary_resolver.rs — `if let Some(target_root) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)`
- test: integration_tests/src/binary_resolver.rs — `if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- tool: xtask/src/kagami_profiles.rs — `if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {`

## CARGO_TARGET_TMPDIR (test: 2)

- test: crates/iroha_plonk_gadgets/tests/m3_gates.rs — `Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pinned_params_{curve}_k{k}.bin"))`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs — `PathBuf::from(env!("CARGO_TARGET_TMPDIR"))`

## CREDENTIALS_DIRECTORY (prod: 2)

- prod: crates/irohad/bins/src/bin/sorafs_external_software_signer.rs — `let directory = env::var_os("CREDENTIALS_DIRECTORY")`
- prod: crates/irohad/bins/src/bin/sorafs_external_software_signer.rs — `let credential_directory = env::var_os("CREDENTIALS_DIRECTORY").map(PathBuf::from);`

## CRYPTO_SM_INTRINSICS (bench: 1)

- bench: crates/iroha_crypto/benches/sm_perf.rs — `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`

## CUDA_HOME (build: 5)

- build: crates/fastpq_prover/build.rs — `env::var_os("CUDA_HOME")`
- build: crates/gpuzstd_cuda/build.rs — `for root in env::var_os("CUDA_HOME")`
- build: crates/gpuzstd_cuda/build.rs — `let root = env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `for root in env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (build: 5)

- build: crates/fastpq_prover/build.rs — `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/gpuzstd_cuda/build.rs — `.chain(env::var_os("CUDA_PATH"))`
- build: crates/gpuzstd_cuda/build.rs — `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `.chain(env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `.or_else(|| env::var_os("CUDA_PATH"))`

## CXX (build: 3)

- build: crates/fastpq_prover/build.rs — `env::var_os("CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs — `env::var_os("CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `env::var_os("CXX").is_some()`

## DAG_DESCRIPTOR_OUTPUT (test: 1)

- test: crates/iroha_plonk_gadgets/tests/m3_gates/export_descriptors.rs — `let output = PathBuf::from(std::env::var_os("DAG_DESCRIPTOR_OUTPUT").unwrap());`

## DAG_TILE_DESCRIPTOR (prod: 1)

- prod: crates/iroha_plonk/src/prover/quotient/tile_experiment.rs — `std::env::var_os("DAG_TILE_DESCRIPTOR").expect("independently pinned actual M3 descriptor"),`

## DAG_TILE_DESCRIPTOR_SHA256 (prod: 1)

- prod: crates/iroha_plonk/src/prover/quotient/tile_experiment.rs — `std::env::var("DAG_TILE_DESCRIPTOR_SHA256").expect("independently pinned SHA256");`

## DAG_TILE_WIDTH (prod: 1)

- prod: crates/iroha_plonk/src/prover/quotient/tile_experiment.rs — `let width: usize = std::env::var("DAG_TILE_WIDTH").unwrap().parse().unwrap();`

## DAG_TILE_WORKERS (prod: 1)

- prod: crates/iroha_plonk/src/prover/quotient/tile_experiment.rs — `let workers: usize = std::env::var("DAG_TILE_WORKERS").unwrap().parse().unwrap();`

## DOCS_RS (build: 1)

- build: crates/norito/build.rs — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (bench: 1)

- bench: crates/norito/benches/enum_packed_bench.rs — `let n: usize = std::env::var("ENUM_BENCH_N")`

## EXPECTED_CARGO_FD (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/policy/cargo_image_tests.rs — `assert_eq!(cargo, std::env::var("EXPECTED_CARGO_FD").unwrap());`

## EXPECTED_CARGO_ID (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/policy/cargo_image_tests.rs — `assert_eq!(format!("{{}}:{{}}",metadata.dev(),metadata.ino()), std::env::var("EXPECTED_CARGO_ID").unwrap());`

## EXPECTED_CARGO_VERSION (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/policy/cargo_image_tests.rs — `assert_eq!(output.stdout, std::env::var("EXPECTED_CARGO_VERSION").unwrap().as_bytes());`

## EXPECTED_ID (test: 1)

- test: crates/iroha_fs/src/build_input_command_tests.rs — `let expected=std::env::var("EXPECTED_ID").unwrap();`

## FASTPQ_CUDA_REQUIRE (test: 5)

- test: crates/fastpq_prover/src/fastpq_cuda.rs — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_none(),`
- test: crates/fastpq_prover/src/fastpq_cuda.rs — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_some(),`
- test: crates/fastpq_prover/src/fastpq_cuda.rs — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_some(),`
- test: crates/fastpq_prover/src/fastpq_cuda.rs — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_some(),`
- test: crates/fastpq_prover/src/fastpq_cuda.rs — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_some(),`

## FASTPQ_RESOURCE_OUTPUT_DIR (test: 1)

- test: crates/fastpq_prover/tests/resource_profile.rs — `let output_dir = std::env::var_os("FASTPQ_RESOURCE_OUTPUT_DIR").map(PathBuf::from);`

## FASTPQ_SKIP_GPU_BUILD (build: 1)

- build: crates/fastpq_prover/build.rs — `let skip_gpu_build = env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some();`

## FASTPQ_TEST_FIXED_SMT_ARTIFACT (prod: 1)

- prod: crates/fastpq_prover/src/backend/deep_prover/diagnostic_artifact.rs — `std::env::var_os("FASTPQ_TEST_FIXED_SMT_ARTIFACT").expect("FASTPQ_TEST_FIXED_SMT_ARTIFACT"),`

## FASTPQ_UPDATE_FIXTURES (test: 1)

- test: crates/fastpq_prover/tests/common/mod.rs — `fixture_update_requested_from(std::env::var_os("FASTPQ_UPDATE_FIXTURES").as_deref())`

## FFT_BATCH_CURVE (test: 1)

- test: crates/iroha_pasta/src/fft_plan_tests.rs — `match std::env::var("FFT_BATCH_CURVE").unwrap().as_str() {`

## GENESIS_DEBUG_MODE (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## GENESIS_DEBUG_PAYLOAD (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## GITHUB_STEP_SUMMARY (prod: 2)

- prod: crates/iroha_crypto/src/bin/gost_perf_check.rs — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs — `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## GITHUB_TOKEN (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/environment.rs — `assert!(std::env::var_os("GITHUB_TOKEN").is_none());`

## GPUZSTD_CUDA_ARCH (build: 1)

- build: crates/gpuzstd_cuda/build.rs — `if let Some(arch_flag) = env::var_os("GPUZSTD_CUDA_ARCH") {`

## GPUZSTD_CUDA_REQUIRE (test: 3)

- test: crates/gpuzstd_cuda/src/lib.rs — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some() {`
- test: crates/gpuzstd_cuda/src/lib.rs — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_none() {`
- test: crates/norito/src/core/gpu_zstd.rs — `std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some()`

## GPUZSTD_CUDA_SKIP_BUILD (build: 1)

- build: crates/gpuzstd_cuda/build.rs — `if env::var_os("GPUZSTD_CUDA_SKIP_BUILD").is_some() {`

## HOME (prod: 8, test: 14)

- test: crates/iroha_cli/src/bridge/retained_network.rs — `PathBuf::from(std::env::var_os("HOME").expect("safe owner home for custody test"))`
- prod: crates/iroha_cli/src/client_config.rs — `env::var_os("HOME").map(PathBuf::from)`
- test: crates/iroha_cli/src/taira_public_reset_config.rs — `let home = std::env::var_os("HOME").expect("native operator test home");`
- test: crates/iroha_cli/src/taira_public_reset_context_release.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_edge.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_edge.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_edge.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_edge_adoption.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_edge_owner_prepare.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_edge_prepare.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_edge_prepare.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_edge_prepare.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- test: crates/iroha_cli/src/taira_public_reset_native_python.rs — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs — `if let Some(home) = env::var_os("HOME") {`
- test: crates/iroha_test_network/tests/support/production_beacon_bootstrap.rs — `let home = std::env::var_os("HOME").ok_or_else(|| eyre!("HOME absent"))?;`
- prod: crates/iroha_wallet/src/custody.rs — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/iroha_wallet/src/custody.rs — `std::env::var_os("HOME").map(PathBuf::from),`
- test: crates/irohad/src/soracloud_runtime/tests/part_05.rs — `if std::env::var_os("HOME").is_none() {`
- prod: crates/musubi/src/cache.rs — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/musubi/src/cache.rs — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/musubi/src/command.rs — `let root = std::env::var_os("HOME").map(PathBuf::from).map(|path| {`
- prod: crates/musubi/src/command.rs — `std::env::var_os("HOME")`

## HOST_CXX (build: 3)

- build: crates/fastpq_prover/build.rs — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `|| env::var_os("HOST_CXX").is_some()`

## IROHA_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs — `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## IROHA_BFV_CONFORMANCE_OUTPUT (prod: 1)

- prod: crates/iroha_crypto/src/fhe_bfv/conformance.rs — `std::env::var_os("IROHA_BFV_CONFORMANCE_OUTPUT")`

## IROHA_BRIDGE_TEST_MODE (test: 1)

- test: crates/iroha_process_bridge/src/tests.rs — `let Ok(mode) = std::env::var("IROHA_BRIDGE_TEST_MODE") else {`

## IROHA_BRIDGE_TEST_RELEASE (test: 1)

- test: crates/iroha_process_bridge/src/tests.rs — `let release = std::env::var("IROHA_BRIDGE_TEST_RELEASE").unwrap();`

## IROHA_CLASSED_STARK_OUTPUT (test: 2)

- test: crates/iroha_core_privacy/src/execution_proofs/classed_race_v1/proof_tests.rs — `if let Some(dir) = std::env::var_os("IROHA_CLASSED_STARK_OUTPUT") {`
- test: crates/iroha_core_privacy/src/execution_proofs/classed_race_v1/proof_tests.rs — `std::env::var_os("IROHA_CLASSED_STARK_OUTPUT")`

## IROHA_COMMON_VALUE (test: 1)

- test: crates/iroha_fs/src/process/tests.rs — `"inspect" => { println!("{:?}", std::env::args().skip(2).collect::<Vec<_>>()); println!("{}", std::env::var("IROHA_COMMON_VALUE").unwrap()); }`

## IROHA_CONF_GAS_SEED (test: 1)

- test: crates/iroha_test_samples/src/lib.rs — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_CONNECT_ACCOUNT_SEED_HEX (example: 1)

- example: crates/iroha_torii_shared/examples/connect_wallet.rs — `let account_seed_hex = std::env::var("IROHA_CONNECT_ACCOUNT_SEED_HEX")`

## IROHA_CORE_ISOLATED_UNIT_TEST_CHILD (prod: 1)

- prod: crates/iroha_core/src/unit_test_support.rs — `if std::env::var_os("IROHA_CORE_ISOLATED_UNIT_TEST_CHILD").as_deref()`

## IROHA_CRYPTO_FIXED_ALGORITHM (test: 1)

- test: crates/iroha_crypto/src/signature/admission/tests.rs — `std::env::var("IROHA_CRYPTO_FIXED_ALGORITHM")`

## IROHA_CRYPTO_FIXED_CASE (test: 1)

- test: crates/iroha_crypto/src/signature/admission/tests.rs — `let mode = std::env::var("IROHA_CRYPTO_FIXED_CASE").unwrap();`

## IROHA_CRYPTO_FIXED_PUBLIC_KEY (test: 1)

- test: crates/iroha_crypto/src/signature/admission/tests.rs — `let bytes = hex::decode(std::env::var("IROHA_CRYPTO_FIXED_PUBLIC_KEY").unwrap()).unwrap();`

## IROHA_CRYPTO_FIXED_SIGNATURE (test: 1)

- test: crates/iroha_crypto/src/signature/admission/tests.rs — `let signature = hex::decode(std::env::var("IROHA_CRYPTO_FIXED_SIGNATURE").unwrap()).unwrap();`

## IROHA_DA_SPOOL_DIR (test: 1)

- test: crates/iroha_core/src/state.rs — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_DEBUG_GENESIS_PATH (test: 3)

- test: crates/iroha_genesis/src/genesis_manifest_tests.rs — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/genesis_manifest_tests.rs — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/genesis_manifest_tests.rs — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`

## IROHA_DEBUG_SIGNED_GENESIS_PATH (test: 1)

- test: crates/iroha_genesis/src/genesis_manifest_tests.rs — `let path = env::var("IROHA_DEBUG_SIGNED_GENESIS_PATH")`

## IROHA_DEVEX_BUNDLE_BIN (test: 3)

- test: xtask/src/mochi/developer_smoke.rs — `let directory = std::env::var_os("IROHA_DEVEX_BUNDLE_BIN")`
- test: xtask/src/mochi/developer_smoke/cold_start_smoke.rs — `let directory = std::env::var_os("IROHA_DEVEX_BUNDLE_BIN")`
- test: xtask/src/mochi/developer_smoke/publication_smoke/tests.rs — `let directory = std::env::var_os("IROHA_DEVEX_BUNDLE_BIN")`

## IROHA_DPN_VALIDATOR_RELEASE_COMMIT (prod: 1)

- prod: crates/iroha_core/src/release_identity.rs — `option_env!("IROHA_DPN_VALIDATOR_RELEASE_COMMIT"),`

## IROHA_DUMP_MANIFEST_JSON (test: 1)

- test: crates/iroha_data_model/src/nexus/manifest.rs — `if std::env::var_os("IROHA_DUMP_MANIFEST_JSON").is_some() {`

## IROHA_EXECUTION_PARITY_OUTPUT (test: 1)

- test: crates/iroha_core_privacy/src/execution_proofs/proof_tests.rs — `let output = std::env::var_os("IROHA_EXECUTION_PARITY_OUTPUT").map(PathBuf::from);`

## IROHA_EXECUTION_PARITY_REQUEST (test: 1)

- test: crates/iroha_core_privacy/src/execution_proofs/proof_tests.rs — `let request = std::env::var_os("IROHA_EXECUTION_PARITY_REQUEST").map_or_else(`

## IROHA_EXPLORER_PROOF_FIXTURE_OUTPUT (test: 1)

- test: crates/iroha_torii/src/routing.rs — `if let Some(path) = std::env::var_os("IROHA_EXPLORER_PROOF_FIXTURE_OUTPUT") {`

## IROHA_FASTPQ_GENUINE_EFFECT_CAPTURE_HASH (test: 1)

- test: crates/fastpq_prover/src/gadgets/public_transfer_statement/execution_effect/tests.rs — `let expected_frame_hash = std::env::var("IROHA_FASTPQ_GENUINE_EFFECT_CAPTURE_HASH")`

## IROHA_FASTPQ_GENUINE_EFFECT_CAPTURE_PATH (test: 1)

- test: crates/fastpq_prover/src/gadgets/public_transfer_statement/execution_effect/tests.rs — `let path = std::env::var_os("IROHA_FASTPQ_GENUINE_EFFECT_CAPTURE_PATH")`

## IROHA_FI_CONTINUATION_FIXTURE_OUTPUT_DIR (test: 1)

- test: crates/connect_norito_bridge/src/fi_context_continuation_tests.rs — `if let Some(destination) = std::env::var_os("IROHA_FI_CONTINUATION_FIXTURE_OUTPUT_DIR")`

## IROHA_GENESIS_FILE (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs — `let genesis_path = std::env::var("IROHA_GENESIS_FILE")`

## IROHA_GENESIS_PUBLIC_KEY (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs — `let pub_key_str = std::env::var("IROHA_GENESIS_PUBLIC_KEY").unwrap_or_else(|_| {`

## IROHA_GIT_COMMIT_HASH (prod: 3, test: 1)

- prod: crates/iroha_core/src/release_identity.rs — `option_env!("IROHA_GIT_COMMIT_HASH"),`
- prod: crates/iroha_js_host/src/lib.rs — `option_env!("IROHA_GIT_COMMIT_HASH")`
- test: crates/iroha_js_host/src/lib.rs — `option_env!("IROHA_GIT_COMMIT_HASH").unwrap_or("unknown")`
- prod: crates/iroha_kagami/src/main.rs — `const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");`

## IROHA_INROU_PORTABLE_INITRD_IMAGE (test: 1, tool: 1)

- test: crates/irohad/src/soracloud_runtime/tests/part_06.rs — `let initrd_image = std::env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`
- tool: xtask/src/soracloud_inrou.rs — `if let Ok(value) = env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`

## IROHA_IVM_NATIVE_CALL_RUNTIME_CAPTURE (test: 2)

- test: crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/frame_descriptor/tests.rs — `let path = std::env::var_os("IROHA_IVM_NATIVE_CALL_RUNTIME_CAPTURE")`
- test: crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/return_copyback/tests.rs — `let path = std::env::var_os("IROHA_IVM_NATIVE_CALL_RUNTIME_CAPTURE")`

## IROHA_IVM_NATIVE_FRAME_OWNER_CAPTURE (test: 2)

- test: crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/frame_descriptor/tests.rs — `let path = std::env::var_os("IROHA_IVM_NATIVE_FRAME_OWNER_CAPTURE")`
- test: crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus/return_copyback/tests.rs — `let path = std::env::var_os("IROHA_IVM_NATIVE_FRAME_OWNER_CAPTURE")`

## IROHA_KAGAMI_LOCALNET_KEEP (test: 1)

- test: integration_tests/tests/sumeragi_kagami_localnet.rs — `if std::env::var_os("IROHA_KAGAMI_LOCALNET_KEEP").is_some() {`

## IROHA_KAGAMI_NATIVE_EXECUTION_CAPTURE (test: 1)

- test: crates/connect_norito_bridge/src/native_execution_capture_tests.rs — `let path = std::env::var_os("IROHA_KAGAMI_NATIVE_EXECUTION_CAPTURE")`

## IROHA_KAGEMUSHA_RETAINED_MANIFEST (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/tests.rs — `let path = std::env::var_os("IROHA_KAGEMUSHA_RETAINED_MANIFEST")`

## IROHA_KAGEMUSHA_RETAINED_MANIFEST_DIGEST (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/tests.rs — `let expected = std::env::var("IROHA_KAGEMUSHA_RETAINED_MANIFEST_DIGEST")`

## IROHA_KAGEMUSHA_TEST_FIXTURE_ROOT (test: 1)

- test: crates/iroha_plonk_gadgets/src/p256/tests.rs — `let root = std::env::var_os("IROHA_KAGEMUSHA_TEST_FIXTURE_ROOT").map_or_else(`

## IROHA_LISTENER_TEST_DIRECTORY (test: 4)

- test: crates/iroha_deploy/src/managed/listeners.rs — `let path = std::env::var_os("IROHA_LISTENER_TEST_DIRECTORY").unwrap();`
- test: crates/iroha_deploy/src/managed/listeners.rs — `let directory = std::env::var_os("IROHA_LISTENER_TEST_DIRECTORY").unwrap();`
- test: crates/iroha_managed_ipc/src/listeners.rs — `let path = std::env::var_os("IROHA_LISTENER_TEST_DIRECTORY").unwrap();`
- test: crates/iroha_managed_ipc/src/listeners.rs — `let directory = std::env::var_os("IROHA_LISTENER_TEST_DIRECTORY").unwrap();`

## IROHA_LISTENER_TEST_P2P (test: 2)

- test: crates/iroha_deploy/src/managed/listeners.rs — `let p2p = std::env::var("IROHA_LISTENER_TEST_P2P")`
- test: crates/iroha_managed_ipc/src/listeners.rs — `let p2p = std::env::var("IROHA_LISTENER_TEST_P2P")`

## IROHA_LISTENER_TEST_TORII (test: 2)

- test: crates/iroha_deploy/src/managed/listeners.rs — `let torii = std::env::var("IROHA_LISTENER_TEST_TORII")`
- test: crates/iroha_managed_ipc/src/listeners.rs — `let torii = std::env::var("IROHA_LISTENER_TEST_TORII")`

## IROHA_METRICS_PANIC_ON_DUPLICATE (debug: 1, test: 1)

- debug: crates/iroha_telemetry/src/metrics.rs — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- test: crates/iroha_torii/tests/metrics_registry.rs — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## IROHA_MOCHI_CANONICAL_FIXTURE_STAGE (test: 1)

- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs — `let Some(raw_stage) = env::var_os("IROHA_MOCHI_CANONICAL_FIXTURE_STAGE") else {`

## IROHA_MOCHI_REPLAY_FIXTURE_STAGE (test: 1)

- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs — `let Some(raw_stage) = env::var_os("IROHA_MOCHI_REPLAY_FIXTURE_STAGE") else {`

## IROHA_NATIVE_PROVER_CAPTURE_DIR (prod: 2)

- prod: crates/iroha_plonk/src/captured_goldens/export.rs — `std::env::var_os("IROHA_NATIVE_PROVER_CAPTURE_DIR")`
- prod: crates/iroha_plonk/src/native_reference_capture.rs — `std::env::var_os("IROHA_NATIVE_PROVER_CAPTURE_DIR")`

## IROHA_OWNER_REMOVED (prod: 1)

- prod: crates/iroha_fs/src/process_owner_probe.rs — `std::env::var_os("IROHA_OWNER_REMOVED").is_none()`

## IROHA_OWNER_VALUE (prod: 1)

- prod: crates/iroha_fs/src/process_owner_probe.rs — `units(&std::env::var_os("IROHA_OWNER_VALUE").unwrap())`

## IROHA_PARITY_NODE (test: 1)

- test: integration_tests/tests/sorafs_orchestrator_parity.rs — `let node = std::env::var("IROHA_PARITY_NODE").unwrap_or_else(|_| "node".to_owned());`

## IROHA_PARITY_PYTHON (test: 1)

- test: integration_tests/tests/sorafs_orchestrator_parity.rs — `let python = std::env::var("IROHA_PARITY_PYTHON").unwrap_or_else(|_| "python3".to_owned());`

## IROHA_PORT_LEASE_TEST_MODE (test: 1)

- test: crates/iroha_test_network/src/fslock_ports/port_lease.rs — `let mode = std::env::var("IROHA_PORT_LEASE_TEST_MODE").unwrap();`

## IROHA_PORT_LEASE_TEST_PORT (test: 1)

- test: crates/iroha_test_network/src/fslock_ports/port_lease.rs — `let port = std::env::var("IROHA_PORT_LEASE_TEST_PORT")`

## IROHA_PORT_LEASE_TEST_ROOT (test: 1)

- test: crates/iroha_test_network/src/fslock_ports/port_lease.rs — `let Some(root) = std::env::var_os("IROHA_PORT_LEASE_TEST_ROOT") else {`

## IROHA_PRINT_PREPARED_TRANSACTION_SIGNATURE_FIXTURE (test: 1)

- test: crates/iroha_torii/src/routing.rs — `if std::env::var_os("IROHA_PRINT_PREPARED_TRANSACTION_SIGNATURE_FIXTURE").is_none() {`

## IROHA_PUBLIC_COLUMN_FIXTURE_DIR (prod: 5, test: 1)

- test: crates/iroha_kagemusha_proof/src/q_signature/native/public_witness_tests.rs — `if let Some(directory) = std::env::var_os("IROHA_PUBLIC_COLUMN_FIXTURE_DIR") {`
- prod: crates/iroha_plonk/src/pcs/ipa/public_column_cache/blocks/benchmark.rs — `let directory = PathBuf::from(std::env::var_os("IROHA_PUBLIC_COLUMN_FIXTURE_DIR").unwrap());`
- prod: crates/iroha_plonk/src/pcs/ipa/public_column_cache/blocks/benchmark.rs — `let directory = PathBuf::from(std::env::var_os("IROHA_PUBLIC_COLUMN_FIXTURE_DIR").unwrap());`
- prod: crates/iroha_plonk/src/pcs/ipa/public_column_cache/blocks/benchmark.rs — `let directory = PathBuf::from(std::env::var_os("IROHA_PUBLIC_COLUMN_FIXTURE_DIR").unwrap());`
- prod: crates/iroha_plonk/src/pcs/ipa/public_column_cache/blocks/benchmark.rs — `let directory = PathBuf::from(std::env::var_os("IROHA_PUBLIC_COLUMN_FIXTURE_DIR").unwrap());`
- prod: crates/iroha_plonk/src/pcs/ipa/public_column_cache/blocks/benchmark.rs — `let directory = PathBuf::from(std::env::var_os("IROHA_PUBLIC_COLUMN_FIXTURE_DIR").unwrap());`

## IROHA_REALISTIC_30TPS_LOAD_KIND (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `let Some(raw) = std::env::var("IROHA_REALISTIC_30TPS_LOAD_KIND")`

## IROHA_REALISTIC_30TPS_LOG_LEVEL (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `std::env::var("IROHA_REALISTIC_30TPS_LOG_LEVEL").unwrap_or_else(|_| "WARN".into());`

## IROHA_REGENERATE_LIST_QUERY_VECTORS (test: 1)

- test: crates/iroha_torii_shared/src/list_query/vectors_tests.rs — `if std::env::var_os("IROHA_REGENERATE_LIST_QUERY_VECTORS").is_some() {`

## IROHA_RELEASE_ARTIFACT_ROOT (test: 1)

- test: crates/iroha_test_network/tests/support/production_beacon_bootstrap.rs — `.or_else(|| std::env::var_os("IROHA_RELEASE_ARTIFACT_ROOT"))`

## IROHA_RELEASE_NATIVE_TARGET (prod: 1)

- prod: crates/iroha_release_bundle/src/bundle.rs — `pub const NATIVE_TARGET: &str = env!("IROHA_RELEASE_NATIVE_TARGET");`

## IROHA_RELEASE_TEST_A (test: 1)

- test: crates/iroha_release_bundle/src/tests.rs — `print!("{}:{}:{}", std::env::var("IROHA_RELEASE_TEST_A").unwrap(), std::env::var("IROHA_RELEASE_TEST_B").unwrap(), std::env::var("IROHA_RELEASE_TEST_REMOVED").is_err());`

## IROHA_RELEASE_TEST_B (test: 1)

- test: crates/iroha_release_bundle/src/tests.rs — `print!("{}:{}:{}", std::env::var("IROHA_RELEASE_TEST_A").unwrap(), std::env::var("IROHA_RELEASE_TEST_B").unwrap(), std::env::var("IROHA_RELEASE_TEST_REMOVED").is_err());`

## IROHA_RELEASE_TEST_REMOVED (test: 1)

- test: crates/iroha_release_bundle/src/tests.rs — `print!("{}:{}:{}", std::env::var("IROHA_RELEASE_TEST_A").unwrap(), std::env::var("IROHA_RELEASE_TEST_B").unwrap(), std::env::var("IROHA_RELEASE_TEST_REMOVED").is_err());`

## IROHA_RESTART_PROBE_BLOCKS (test: 1)

- test: crates/iroha_core/src/snapshot/reconciliation_generation_tests.rs — `let blocks = std::env::var("IROHA_RESTART_PROBE_BLOCKS")`

## IROHA_ROTATION_DESCRIPTOR_FIXTURE (test: 1)

- test: crates/iroha_test_network/src/disposable_rotation_ceremony.rs — `let parameters = std::env::var("IROHA_ROTATION_DESCRIPTOR_FIXTURE").unwrap();`

## IROHA_RUNTIME_SIGNING_KEY (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/environment.rs — `assert!(std::env::var_os("IROHA_RUNTIME_SIGNING_KEY").is_none());`

## IROHA_RUN_IGNORED (test: 18)

- test: crates/iroha_core/tests/check_genesis_sig.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/zk_roots_get_cap.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints_router.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_read_endpoints_router.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/beep_test.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/kotodama_struct_fields.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/pipeline_block_rejected.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/sorting.rs — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_SCCP_RELATIVE_ROOT_CHILD (test: 1)

- test: crates/iroha_sccp_wallet/src/journal.rs — `std::env::var("IROHA_SCCP_RELATIVE_ROOT_CHILD").as_deref(),`

## IROHA_SKIP_BIND_CHECKS (test: 1)

- test: crates/iroha_test_network/src/lib.rs — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (test: 1)

- test: crates/iroha_crypto/tests/sm_cli_matrix.rs — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_TEST_BUILD_PROFILE (test: 1)

- test: integration_tests/src/binary_resolver.rs — `std::env::var("IROHA_TEST_BUILD_PROFILE").ok().as_deref(),`

## IROHA_TEST_CLIENT_TTL_MS (test: 5)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`

## IROHA_TEST_DUMP_GENESIS (test: 1)

- test: crates/iroha_test_network/src/lib.rs — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`

## IROHA_TEST_HANDOFF_DEADLINE (test: 1)

- test: crates/iroha_deploy/src/managed/runtime/handoff_tests.rs — `let deadline: u128 = std::env::var("IROHA_TEST_HANDOFF_DEADLINE")`

## IROHA_TEST_HANDOFF_DIRECTORY (test: 1)

- test: crates/iroha_deploy/src/managed/runtime/handoff_tests.rs — `let Some(path) = std::env::var_os("IROHA_TEST_HANDOFF_DIRECTORY") else {`

## IROHA_TEST_HANDOFF_MODE (test: 1)

- test: crates/iroha_deploy/src/managed/runtime/handoff_tests.rs — `let mode = std::env::var("IROHA_TEST_HANDOFF_MODE").unwrap();`

## IROHA_TEST_HANDOFF_STORE (test: 1)

- test: crates/iroha_deploy/src/managed/runtime/handoff_tests.rs — `let root = std::env::var_os("IROHA_TEST_HANDOFF_STORE").unwrap();`

## IROHA_TEST_INGRESS_LIFETIME_CHILD (test: 1)

- test: crates/iroha_deploy/src/managed/attachment_installed_tests.rs — `std::env::var("IROHA_TEST_INGRESS_LIFETIME_CHILD").as_deref(),`

## IROHA_TEST_NETWORK_PARALLELISM (test: 4)

- test: integration_tests/tests/address_canonicalisation.rs — `if let Ok(raw) = env::var("IROHA_TEST_NETWORK_PARALLELISM")`
- test: integration_tests/tests/native_amx_network/rotation.rs — `std::env::var("IROHA_TEST_NETWORK_PARALLELISM")`
- test: integration_tests/tests/support/native_amx_harness.rs — `let permits = std::env::var("IROHA_TEST_NETWORK_PARALLELISM")`
- test: integration_tests/tests/tides_independent_roots.rs — `let permits = std::env::var("IROHA_TEST_NETWORK_PARALLELISM")`

## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (build: 1, test: 1)

- test: crates/iroha_test_network/src/config.rs — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- build: crates/iroha_test_samples/build.rs — `if env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_PRIVATE_STORE (test: 1)

- test: crates/iroha_deploy/src/managed/installed_tests.rs — `let root = std::env::var_os("IROHA_TEST_PRIVATE_STORE").map_or_else(`

## IROHA_TEST_REAL_SORAFS_NODE (test: 1)

- test: crates/iroha_cli/src/soracloud/tests/part_02.rs — `let helper = std::env::var_os("IROHA_TEST_REAL_SORAFS_NODE")`

## IROHA_TEST_REQUIRE_NETWORK (test: 4)

- test: integration_tests/tests/contracts/ivm_proved.rs — `std::env::var("IROHA_TEST_REQUIRE_NETWORK").as_deref() == Ok("1"),`
- test: integration_tests/tests/kaigi_privacy_network.rs — `std::env::var("IROHA_TEST_REQUIRE_NETWORK").as_deref() == Ok("1"),`
- test: integration_tests/tests/proofs/fastpq_finalized_transcripts.rs — `std::env::var("IROHA_TEST_REQUIRE_NETWORK").as_deref() == Ok("1"),`
- test: integration_tests/tests/proofs/full_tree_wallet.rs — `std::env::var("IROHA_TEST_REQUIRE_NETWORK").as_deref() == Ok("1"),`

## IROHA_TEST_RUNTIME_DIRECTORY (test: 5)

- test: crates/iroha_deploy/src/bootstrap/native/tests.rs — `std::env::var_os("IROHA_TEST_RUNTIME_DIRECTORY")`
- test: crates/iroha_deploy/src/managed/attachment_installed_tests.rs — `std::env::var_os("IROHA_TEST_RUNTIME_DIRECTORY")`
- test: crates/iroha_deploy/src/managed/installed_renewal_tests.rs — `std::env::var_os("IROHA_TEST_RUNTIME_DIRECTORY")`
- test: crates/iroha_deploy/src/managed/installed_tests.rs — `std::env::var_os("IROHA_TEST_RUNTIME_DIRECTORY")`
- test: crates/iroha_deploy/src/managed/installed_tests.rs — `std::env::var_os("IROHA_TEST_RUNTIME_DIRECTORY")`

## IROHA_TEST_SERIALIZE_NETWORKS (test: 5)

- test: integration_tests/tests/address_canonicalisation.rs — `if let Ok(raw) = env::var("IROHA_TEST_SERIALIZE_NETWORKS")`
- test: integration_tests/tests/asset.rs — `if std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none() {`
- test: integration_tests/tests/native_amx_network/rotation.rs — `&& std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none(),`
- test: integration_tests/tests/support/native_amx_harness.rs — `permits >= 3 && std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none(),`
- test: integration_tests/tests/tides_independent_roots.rs — `permits >= 2 && std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none(),`

## IROHA_TEST_SKIP_BUILD (test: 2)

- test: integration_tests/src/binary_resolver.rs — `std::env::var("IROHA_TEST_SKIP_BUILD").ok().as_deref(),`
- test: integration_tests/tests/alias_registry_bootstrap_network.rs — `std::env::var("IROHA_TEST_SKIP_BUILD").as_deref() == Ok("1"),`

## IROHA_TEST_USE_DEFAULT_EXECUTOR (test: 2)

- test: crates/iroha_core/src/executor_contract_dispatch_tests.rs — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- test: crates/iroha_core/src/executor_tests.rs — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`

## IROHA_TEST_WINDOWS_REFRESH_ROOT (test: 1)

- test: crates/iroha_cli/src/taira_dataspace_deploy_windows_refresh_tests.rs — `let Some(path) = std::env::var_os("IROHA_TEST_WINDOWS_REFRESH_ROOT") else {`

## IROHA_TEST_WINDOWS_REFRESH_STEP (test: 1)

- test: crates/iroha_cli/src/taira_dataspace_deploy_windows_refresh_tests.rs — `let stop = std::env::var("IROHA_TEST_WINDOWS_REFRESH_STEP").unwrap();`

## IROHA_THROUGHPUT_ARTIFACT_DIR (test: 3)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`

## IROHA_THROUGHPUT_DELAY_MS (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs — `if let Ok(delay) = std::env::var("IROHA_THROUGHPUT_DELAY_MS")`

## IROHA_TORII_OPENAPI_ACTUAL (test: 1)

- test: crates/iroha_torii/tests/router_feature_matrix.rs — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_EXPECTED (test: 2)

- test: crates/iroha_torii/tests/router_feature_matrix.rs — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- test: crates/iroha_torii/tests/router_feature_matrix.rs — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS (test: 3)

- test: crates/iroha_data_model/src/events/data/kagemusha/tests.rs — `if std::env::var_os("IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS").is_some_and(|value| value == "1") {`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1/ledger_finality_tests.rs — `if std::env::var_os("IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS").is_some_and(|value| value == "1") {`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1/load_finality.rs — `if std::env::var_os("IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS")`

## IROHA_WRITE_DIRECT_CONVICTION_GOLDEN_V1 (test: 1)

- test: crates/iroha_data_model/src/isi/governance.rs — `if std::env::var("IROHA_WRITE_DIRECT_CONVICTION_GOLDEN_V1")`

## IROHA_X509_PUBLIC_CANDIDATE (prod: 1)

- prod: crates/iroha_core_privacy/src/privacy_engines/zk_x509/engine_prover_diagnostic.rs — `std::env::var_os("IROHA_X509_PUBLIC_CANDIDATE")`

## IVM_COMPILER_DEBUG (prod: 1)

- prod: crates/kotodama_lang/src/compiler.rs — `if cfg!(any(test, debug_assertions)) && std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_SELFTEST_TRACE (test: 1)

- test: crates/ivm/src/cuda.rs — `if std::env::var_os("IVM_CUDA_SELFTEST_TRACE").is_some() {`

## IVM_DEBUG_AED_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `let aed_asset_raw = std::env::var("IVM_DEBUG_AED_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `let asset_raw = std::env::var("IVM_DEBUG_ASSET_DEFINITION")`

## IVM_DEBUG_CBDC_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `let cbdc_asset_raw = std::env::var("IVM_DEBUG_CBDC_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_DOMAIN (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `let domain_raw = std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank".to_owned());`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank.universal".to_owned());`

## IVM_DEBUG_FROM_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`

## IVM_DEBUG_METAL_SELFTEST (prod: 1)

- prod: crates/ivm/src/vector.rs — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DEBUG_RATIO (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `let ratio_raw = std::env::var("IVM_DEBUG_RATIO").unwrap_or_else(|_| "76".to_owned());`

## IVM_DEBUG_TO_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `let to_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs — `let dst_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`

## IVM_DISABLE_CUDA (test: 2)

- test: crates/ivm/tests/cuda_disable_on_mismatch.rs — `original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),`
- test: crates/ivm/tests/cuda_env.rs — `original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),`

## IVM_DISABLE_METAL (prod: 1)

- prod: crates/ivm/src/vector.rs — `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELFTEST_FAIL (test: 2)

- test: crates/ivm/tests/cuda_disable_on_mismatch.rs — `original_force_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),`
- test: crates/ivm/tests/cuda_env.rs — `original_force_selftest_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),`

## IVM_FORCE_METAL_SELFTEST_FAIL (prod: 1)

- prod: crates/ivm/src/vector.rs — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IZANAMI_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs — `std::env::var("IZANAMI_ALLOW_NET")`

## IZANAMI_TUI_ALLOW_ZERO_SEED (prod: 1)

- prod: crates/izanami/src/tui.rs — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_REQUIRE (test: 2)

- test: crates/norito/accelerators/jsonstage1_cuda/src/lib.rs — `std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some()`
- test: crates/norito/src/lib.rs — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`

## JSONSTAGE1_CUDA_SKIP_BUILD (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KAGEMUSHA_BUILTIN_BATCH_ACCEPTANCE_OUTPUT (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/builtin_batch_tests.rs — `PathBuf::from(std::env::var_os("KAGEMUSHA_BUILTIN_BATCH_ACCEPTANCE_OUTPUT").unwrap());`

## KAGEMUSHA_BUILTIN_WRAPPER_ACCEPTANCE_OUTPUT (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/builtin_wrapper_tests.rs — `PathBuf::from(std::env::var_os("KAGEMUSHA_BUILTIN_WRAPPER_ACCEPTANCE_OUTPUT").unwrap());`

## KAGEMUSHA_CAPACITY_INPUT (prod: 1)

- prod: crates/iroha_kagemusha_proof/src/a_relation/native/receive/original/source_capacity.rs — `let input = std::env::var("KAGEMUSHA_CAPACITY_INPUT").unwrap();`

## KAGEMUSHA_CAPACITY_OUTPUT (prod: 1)

- prod: crates/iroha_kagemusha_proof/src/a_relation/native/receive/original/source_capacity.rs — `let output = std::env::var("KAGEMUSHA_CAPACITY_OUTPUT").unwrap();`

## KAGEMUSHA_DIAGNOSTIC_INPUT (prod: 1)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/diagnostic.rs — `let input = PathBuf::from(std::env::var("KAGEMUSHA_DIAGNOSTIC_INPUT").unwrap());`

## KAGEMUSHA_DIAGNOSTIC_ORIGINALS (prod: 2)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/diagnostic.rs — `let directory = PathBuf::from(std::env::var("KAGEMUSHA_DIAGNOSTIC_ORIGINALS").unwrap());`
- prod: crates/iroha_kagemusha_proof/src/a_relation/native/receive/original/source_capacity.rs — `let original = std::env::var("KAGEMUSHA_DIAGNOSTIC_ORIGINALS").unwrap();`

## KAGEMUSHA_ENGINEERING_ISSUER_REQUEST (test: 1)

- test: integration_tests/tests/kagemusha_network_setup/continuation/issuer/provisioning.rs — `std::env::var_os("KAGEMUSHA_ENGINEERING_ISSUER_REQUEST")`

## KAGEMUSHA_ENGINEERING_ISSUER_REQUEST_SHA256 (test: 1)

- test: integration_tests/tests/kagemusha_network_setup/continuation/issuer/provisioning.rs — `let sha256 = std::env::var("KAGEMUSHA_ENGINEERING_ISSUER_REQUEST_SHA256")?;`

## KAGEMUSHA_EXECUTED_LEDGER_SETUP_OUTPUT (prod: 1)

- prod: crates/iroha_core/src/sumeragi/test_chain/kagemusha_setup.rs — `std::env::var_os("KAGEMUSHA_EXECUTED_LEDGER_SETUP_OUTPUT").expect("exclusive output path");`

## KAGEMUSHA_HARD_PREDECESSOR_DESCRIPTOR_SHA256 (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/native_worker/incoming/soft_omega_differential.rs — `&std::env::var("KAGEMUSHA_HARD_PREDECESSOR_DESCRIPTOR_SHA256")`

## KAGEMUSHA_HARD_PREDECESSOR_FIXTURE (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/native_worker/incoming/soft_omega_differential.rs — `std::env::var("KAGEMUSHA_HARD_PREDECESSOR_FIXTURE")`

## KAGEMUSHA_HARD_PREDECESSOR_KEY_SHA256 (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/native_worker/incoming/soft_omega_differential.rs — `pin(&std::env::var("KAGEMUSHA_HARD_PREDECESSOR_KEY_SHA256")`

## KAGEMUSHA_HARD_PREDECESSOR_MANIFEST_SHA256 (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/native_worker/incoming/soft_omega_differential.rs — `pin(&std::env::var("KAGEMUSHA_HARD_PREDECESSOR_MANIFEST_SHA256")`

## KAGEMUSHA_HARD_PREDECESSOR_OUTPUT (test: 1)

- test: crates/iroha_kagemusha_proof/src/a_relation/archive/tests/hard_predecessor.rs — `std::env::var_os("KAGEMUSHA_HARD_PREDECESSOR_OUTPUT").expect("fresh output path"),`

## KAGEMUSHA_HISTORICAL_INSTRUCTIONS (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/tests.rs — `std::fs::read_dir(std::env::var("KAGEMUSHA_HISTORICAL_INSTRUCTIONS").unwrap())`

## KAGEMUSHA_HUNDRED_PAYERS_COUNT (prod: 1)

- prod: crates/iroha_core/src/sumeragi/test_chain/kagemusha_setup/hundred_payers.rs — `std::env::var("KAGEMUSHA_HUNDRED_PAYERS_COUNT")`

## KAGEMUSHA_HUNDRED_PAYERS_OUTPUT (prod: 1)

- prod: crates/iroha_core/src/sumeragi/test_chain/kagemusha_setup/hundred_payers.rs — `std::env::var_os("KAGEMUSHA_HUNDRED_PAYERS_OUTPUT").unwrap_or_else(|| {`

## KAGEMUSHA_INCOMING_BOUNDARY_OUTPUT (test: 1)

- test: crates/iroha_kagemusha_proof/src/a_relation/archive/tests/current_omega_soft.rs — `PathBuf::from(std::env::var("KAGEMUSHA_INCOMING_BOUNDARY_OUTPUT").expect("fresh output"));`

## KAGEMUSHA_INCOMING_NATIVE_CASES (test: 1)

- test: crates/iroha_kagemusha_proof/src/a_relation/archive/tests/current_omega_soft.rs — `std::env::var("KAGEMUSHA_INCOMING_NATIVE_CASES")`

## KAGEMUSHA_INCOMING_NATIVE_RESULT_SHA256 (test: 1)

- test: crates/iroha_kagemusha_proof/src/a_relation/archive/tests/current_omega_soft.rs — `let selected = pin(&std::env::var("KAGEMUSHA_INCOMING_NATIVE_RESULT_SHA256")`

## KAGEMUSHA_INSTALLED_PACK (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/tests.rs — `let original = std::fs::read(std::env::var("KAGEMUSHA_INSTALLED_PACK").unwrap()).unwrap();`

## KAGEMUSHA_KOTLIN_ACTIVATE_FIXTURE (test: 1)

- test: crates/iroha_data_model/tests/kagemusha_activate_sdk.rs — `std::env::var_os("KAGEMUSHA_KOTLIN_ACTIVATE_FIXTURE")`

## KAGEMUSHA_KOTLIN_ISSUE_LOAD_FIXTURE (test: 1)

- test: crates/iroha_data_model/tests/kagemusha_issue_load_sdk.rs — `std::env::var_os("KAGEMUSHA_KOTLIN_ISSUE_LOAD_FIXTURE")`

## KAGEMUSHA_LOAD_OUTPUT (test: 5)

- test: crates/iroha_kagemusha_proof/tests/a_consuming_recursive/native_load_tests.rs — `std::env::var_os("KAGEMUSHA_LOAD_OUTPUT")`
- test: crates/iroha_kagemusha_proof/tests/common/first_load_fixture.rs — `std::env::var_os("KAGEMUSHA_LOAD_OUTPUT")`
- test: crates/iroha_kagemusha_proof/tests/lean_load_omega.rs — `std::env::var_os("KAGEMUSHA_LOAD_OUTPUT")`
- test: crates/iroha_kagemusha_proof/tests/ordinary_load_finality.rs — `std::env::var_os("KAGEMUSHA_LOAD_OUTPUT")`
- test: crates/iroha_kagemusha_proof/tests/receive_omega/native_load_tests.rs — `std::env::var_os("KAGEMUSHA_LOAD_OUTPUT")`

## KAGEMUSHA_MSM_BATCH_CURVE (prod: 1)

- prod: crates/iroha_pasta/src/msm/batch_cap_experiment.rs — `let curve = std::env::var("KAGEMUSHA_MSM_BATCH_CURVE").unwrap();`

## KAGEMUSHA_MSM_BATCH_SHAPE (prod: 1)

- prod: crates/iroha_pasta/src/msm/batch_cap_experiment.rs — `let shape = std::env::var("KAGEMUSHA_MSM_BATCH_SHAPE").unwrap();`

## KAGEMUSHA_MSM_STREAM_CURVE (prod: 1)

- prod: crates/iroha_pasta/src/msm/streaming_experiment.rs — `let curve = std::env::var("KAGEMUSHA_MSM_STREAM_CURVE").unwrap();`

## KAGEMUSHA_MSM_STREAM_MODE (prod: 1)

- prod: crates/iroha_pasta/src/msm/streaming_experiment.rs — `let mode = std::env::var("KAGEMUSHA_MSM_STREAM_MODE").unwrap();`

## KAGEMUSHA_MSM_STREAM_SHAPE (prod: 1)

- prod: crates/iroha_pasta/src/msm/streaming_experiment.rs — `let shape = std::env::var("KAGEMUSHA_MSM_STREAM_SHAPE").unwrap();`

## KAGEMUSHA_MSM_STREAM_WORKERS (prod: 1)

- prod: crates/iroha_pasta/src/msm/streaming_experiment.rs — `let workers = std::env::var("KAGEMUSHA_MSM_STREAM_WORKERS")`

## KAGEMUSHA_NATIVE_ACTIVATION_INVOCATION (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/tests/native_exchange/load/activation_retention.rs — `let invocation = std::env::var("KAGEMUSHA_NATIVE_ACTIVATION_INVOCATION").unwrap();`

## KAGEMUSHA_NATIVE_FINALITY_ACCEPTANCE_OUTPUT (prod: 1)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/acceptance.rs — `PathBuf::from(std::env::var_os("KAGEMUSHA_NATIVE_FINALITY_ACCEPTANCE_OUTPUT").unwrap());`

## KAGEMUSHA_OMEGA_CODEC_ROLE (test: 1)

- test: crates/iroha_kagemusha_proof/tests/common/proof_fixtures/omega_hard_fixture.rs — `match std::env::var("KAGEMUSHA_OMEGA_CODEC_ROLE").as_deref() {`

## KAGEMUSHA_OMEGA_CODEC_VECTOR (test: 1)

- test: crates/iroha_kagemusha_proof/tests/common/proof_fixtures/omega_hard_fixture.rs — `std::env::var_os("KAGEMUSHA_OMEGA_CODEC_VECTOR").expect("explicit DATA vector path"),`

## KAGEMUSHA_OMEGA_FIXTURE_PROVENANCE_SHA256 (test: 1)

- test: crates/iroha_kagemusha_proof/tests/common/proof_fixtures/omega_hard_fixture_export.rs — `let provenance_sha = pin(&std::env::var("KAGEMUSHA_OMEGA_FIXTURE_PROVENANCE_SHA256")`

## KAGEMUSHA_OPENING_BASELINE_OUTPUT (test: 1)

- test: crates/iroha_core_zk/src/bin/kagemusha_wallet_artifacts/builtin.rs — `let output = std::env::var_os("KAGEMUSHA_OPENING_BASELINE_OUTPUT").unwrap();`

## KAGEMUSHA_OPENING_BASELINE_REQUEST (test: 1)

- test: crates/iroha_core_zk/src/bin/kagemusha_wallet_artifacts/builtin.rs — `let path = std::env::var("KAGEMUSHA_OPENING_BASELINE_REQUEST").unwrap();`

## KAGEMUSHA_PARALLEL_ROUTES_ACCEPTANCE_OUTPUT (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/operation/session/tests.rs — `std::env::var_os("KAGEMUSHA_PARALLEL_ROUTES_ACCEPTANCE_OUTPUT").unwrap(),`

## KAGEMUSHA_REAL_FEE_CAMPAIGN (test: 1)

- test: integration_tests/tests/kagemusha_network_setup/continuation/fee.rs — `std::env::var_os("KAGEMUSHA_REAL_FEE_CAMPAIGN")`

## KAGEMUSHA_REAL_NETWORK_CAMPAIGN (test: 1)

- test: integration_tests/tests/kagemusha_network_setup/continuation.rs — `std::env::var_os("KAGEMUSHA_REAL_NETWORK_CAMPAIGN")`

## KAGEMUSHA_REAL_NETWORK_SETUP_OUTPUT (test: 1)

- test: integration_tests/tests/kagemusha_network_setup.rs — `let output = std::env::var_os("KAGEMUSHA_REAL_NETWORK_SETUP_OUTPUT")`

## KAGEMUSHA_REGISTER_CAPTURE_OUTPUT (prod: 1)

- prod: crates/iroha_core/src/sumeragi/test_chain/kagemusha_setup/registration/export.rs — `std::env::var_os("KAGEMUSHA_REGISTER_CAPTURE_OUTPUT").expect("fresh private output child");`

## KAGEMUSHA_REGISTER_SCHEME_ORIGINAL (prod: 1)

- prod: crates/iroha_core/src/sumeragi/test_chain/kagemusha_setup/registration/export.rs — `std::env::var_os("KAGEMUSHA_REGISTER_SCHEME_ORIGINAL")`

## KAGEMUSHA_SIGNED_GENESIS_FIXTURE (prod: 1)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog.rs — `std::env::var_os("KAGEMUSHA_SIGNED_GENESIS_FIXTURE").expect("selected genesis fixture"),`

## KAGEMUSHA_SOFT_LOCAL_OUTPUT (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_state_v1/native_worker/incoming/soft_omega_differential.rs — `std::env::var("KAGEMUSHA_SOFT_LOCAL_OUTPUT")`

## KAGEMUSHA_WALLET_ACCEPTANCE_OUTPUT (prod: 2)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/acceptance.rs — `let output = PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_ACCEPTANCE_OUTPUT").unwrap());`
- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/acceptance.rs — `let output = PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_ACCEPTANCE_OUTPUT").unwrap());`

## KAGEMUSHA_WALLET_CATALOG_OUTPUT (prod: 1)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog.rs — `std::env::var_os("KAGEMUSHA_WALLET_CATALOG_OUTPUT")`

## KAGEMUSHA_WALLET_CRASH_SEEDS (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_advance_v1/crash_matrix_tests.rs — `std::env::var("KAGEMUSHA_WALLET_CRASH_SEEDS")`

## KAGEMUSHA_WALLET_NATIVE_OUTPUT (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_wallet_enrollment_v1/tests.rs — `std::env::var_os("KAGEMUSHA_WALLET_NATIVE_OUTPUT").expect("fresh native owner output"),`

## KAGEMUSHA_WALLET_NEW_RAW_BYTES (prod: 1)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog.rs — `new_write_limit(std::env::var_os("KAGEMUSHA_WALLET_NEW_RAW_BYTES").as_deref()).unwrap();`

## KAGEMUSHA_WALLET_RETAINED_ORIGINALS (prod: 1)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/acceptance.rs — `let originals = PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_RETAINED_ORIGINALS").unwrap());`

## KAGEMUSHA_WALLET_REUSE_INPUTS (prod: 1)

- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/reuse.rs — `let Some(manifest) = std::env::var_os("KAGEMUSHA_WALLET_REUSE_INPUTS") else {`

## KAGEMUSHA_WALLET_SIGNED_CATALOG (prod: 3)

- prod: crates/iroha_core/src/sumeragi/test_chain/kagemusha_setup/hundred_payers.rs — `std::env::var_os("KAGEMUSHA_WALLET_SIGNED_CATALOG")`
- prod: crates/iroha_core/src/sumeragi/test_chain/kagemusha_setup/hundred_payers.rs — `PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_SIGNED_CATALOG").unwrap())`
- prod: crates/iroha_core_zk/src/kagemusha_wallet_artifacts_v1/producer_inventory/compiler/full_catalog/acceptance.rs — `let catalog = PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_SIGNED_CATALOG").unwrap());`

## LANG (test: 2)

- test: crates/ivm/tests/i18n.rs — `let old_lang = env::var("LANG").ok();`
- test: crates/ivm/tests/i18n.rs — `let old_lang = env::var("LANG").ok();`

## LC_ALL (test: 2)

- test: crates/ivm/tests/i18n.rs — `let old_lc_all = env::var("LC_ALL").ok();`
- test: crates/ivm/tests/i18n.rs — `let old_lc_all = env::var("LC_ALL").ok();`

## LC_MESSAGES (test: 2)

- test: crates/ivm/tests/i18n.rs — `let old_lc_messages = env::var("LC_MESSAGES").ok();`
- test: crates/ivm/tests/i18n.rs — `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## LOCALAPPDATA (prod: 4, test: 1)

- prod: crates/iroha_deploy/src/managed/workspace.rs — `std::env::var_os("LOCALAPPDATA")`
- prod: crates/iroha_wallet/src/custody.rs — `std::env::var_os("LOCALAPPDATA").map(PathBuf::from),`
- test: crates/iroha_wallet/src/custody_native_tests.rs — `let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);`
- prod: crates/musubi/src/cache.rs — `std::env::var_os("LOCALAPPDATA").map(PathBuf::from),`
- prod: crates/musubi/src/command.rs — `let root = std::env::var_os("LOCALAPPDATA")`

## M3_SEED (test: 1)

- test: crates/iroha_plonk_gadgets/tests/m3_gates.rs — `let seed = measurement_seed(std::env::var_os("M3_SEED").as_deref())`

## MOCHI_PREVIEW_OUT (test: 3)

- test: mochi/mochi-ui-egui/src/gui/preview_tests.rs — `let out = PathBuf::from(std::env::var_os("MOCHI_PREVIEW_OUT").expect("MOCHI_PREVIEW_OUT"));`
- test: mochi/mochi-ui-egui/src/gui/preview_tests.rs — `let out = PathBuf::from(std::env::var_os("MOCHI_PREVIEW_OUT").expect("MOCHI_PREVIEW_OUT"))`
- test: mochi/mochi-ui-egui/src/gui/preview_tests.rs — `let out = PathBuf::from(std::env::var_os("MOCHI_PREVIEW_OUT").expect("MOCHI_PREVIEW_OUT"))`

## MOCHI_PREVIEW_RUNTIME (test: 3)

- test: mochi/mochi-ui-egui/src/gui/preview_tests.rs — `let Some(runtime) = std::env::var_os("MOCHI_PREVIEW_RUNTIME") else {`
- test: mochi/mochi-ui-egui/src/gui/preview_tests.rs — `let runtime = std::env::var_os("MOCHI_PREVIEW_RUNTIME").expect("MOCHI_PREVIEW_RUNTIME");`
- test: mochi/mochi-ui-egui/src/gui/preview_tests.rs — `let Some(runtime) = std::env::var_os("MOCHI_PREVIEW_RUNTIME") else {`

## MUSUBI_BINDGEN_CAPTURE_DIR (test: 1)

- test: crates/musubi/src/bindgen/tests.rs — `if let Some(directory) = std::env::var_os("MUSUBI_BINDGEN_CAPTURE_DIR") {`

## NESTED_TOOL (test: 1)

- test: crates/iroha_fs/src/build_input_command_tests.rs — `let inherited=std::fs::metadata(std::env::var_os("NESTED_TOOL").unwrap())`

## NORITO_CHECK_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs — `if env::var_os("NORITO_CHECK_BINDINGS_SYNC").is_none() {`

## NORITO_CPU_INFO (tool: 1)

- tool: xtask/src/stage1_bench.rs — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_CUDA_REQUIRE (test: 1)

- test: crates/norito/src/core/simd_crc64.rs — `if std::env::var_os("NORITO_CRC64_CUDA_REQUIRE").is_none() {`

## NORITO_CRC64_GPU_LIB (prod: 1)

- prod: crates/norito/src/core/simd_crc64.rs — `let raw = std::env::var_os("NORITO_CRC64_GPU_LIB")?;`

## NORITO_GPU_CRC64_MIN_BYTES (prod: 1)

- prod: crates/norito/src/core/simd_crc64.rs — `let configured = std::env::var("NORITO_GPU_CRC64_MIN_BYTES")`

## NORITO_PAR_STAGE1_MIN (prod: 1)

- prod: crates/norito/src/lib.rs — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## NORITO_SKIP_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_GPU_MIN_BYTES (prod: 1)

- prod: crates/norito/src/lib.rs — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## NORITO_TRACE (debug: 1, test: 2)

- test: crates/norito/src/lib.rs — `std::env::var_os("NORITO_TRACE").is_some()`
- debug: crates/norito/src/lib.rs — `*ENABLED.get_or_init(|| std::env::var_os("NORITO_TRACE").is_some())`
- test: crates/norito/src/lib.rs — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## NOTIFY_SOCKET (prod: 1)

- prod: crates/irohad/src/runtime_provider_broker/launcher.rs — `let notify_socket = std::env::var_os("NOTIFY_SOCKET")`

## OUT_DIR (build: 6, prod: 7, test: 6)

- build: crates/connect_norito_bridge/build.rs — `let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))`
- prod: crates/connect_norito_bridge/src/kagemusha_wallet_ffi/installed/selection.rs — `env!("OUT_DIR"),`
- build: crates/iroha_test_samples/build.rs — `let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR"));`
- test: crates/iroha_test_samples/src/lib.rs — `PathBuf::from(env!("OUT_DIR"))`
- test: crates/iroha_test_samples/src/lib.rs — `PathBuf::from(env!("OUT_DIR")).join("ivm/build_config.toml")`
- test: crates/iroha_test_samples/src/lib.rs — `Path::new(env!("OUT_DIR")).join("ivm/samples")`
- test: crates/iroha_test_samples/src/lib.rs — `Path::new(env!("OUT_DIR")).join("ivm/build_config.toml")`
- build: crates/ivm/build.rs — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- prod: crates/ivm/src/cuda_artifact.rs — `include!(concat!(env!("OUT_DIR"), "/cuda_bundle.rs"));`
- prod: crates/ivm/src/iso20022.rs — `include!(concat!(env!("OUT_DIR"), "/iso20022_schema_v1.rs"));`
- prod: crates/ivm/src/ivm.rs — `include!(concat!(env!("OUT_DIR"), "/syscall_signatures.rs"));`
- build: crates/kotodama_lang/build.rs — `let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));`
- prod: crates/kotodama_lang/src/diagnostic.rs — `env!("OUT_DIR"),`
- prod: crates/kotodama_lang/src/i18n/mod.rs — `include_bytes!(concat!(env!("OUT_DIR"), "/kotodama_i18n_v1_offsets.bin"));`
- prod: crates/kotodama_lang/src/lexer.rs — `include!(concat!(env!("OUT_DIR"), "/kotodama_v1_lexical.rs"));`
- test: crates/kotodama_lang/tests/compile_fail_goldens.rs — `include!(concat!(env!("OUT_DIR"), "/kotodama_compile_fail_cases.rs"));`
- test: crates/kotodama_lang/tests/secret_security_diagnostics.rs — `include!(concat!(env!("OUT_DIR"), "/kotodama_secret_reject_cases.rs"));`

## PATH (prod: 2, test: 5)

- test: crates/iroha_deploy/src/sora_network/path_and_script_tests.rs — `.chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),`
- test: crates/iroha_fs/src/build_input_command_tests.rs — `.env("PATH", std::env::var_os("PATH").unwrap())`
- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs — `if env::var_os("PATH").as_deref() != Some(OsStr::new("/usr/bin:/bin")) {`
- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/policy/cargo_image_tests.rs — `.env("PATH", std::env::var_os("PATH").unwrap())`
- test: crates/iroha_test_network/src/development_process.rs — `let path = select(tool, std::env::var_os("PATH").as_deref())?;`
- prod: crates/irohad/src/soracloud_runtime.rs — `if let Some(path) = std::env::var_os("PATH") {`
- test: integration_tests/tests/kotodama_examples.rs — `let path = env::var_os("PATH")?;`

## PRINT_SORACLES_FIXTURES (test: 1)

- test: crates/iroha_data_model/src/oracle/mod.rs — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORII_SPEC (test: 1)

- test: crates/iroha_torii/src/openapi/tests/catalog_and_contracts.rs — `if std::env::var("PRINT_TORII_SPEC").is_ok()`

## PROBE_ROOT (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/policy/cargo_image_tests.rs — `let root = std::path::PathBuf::from(std::env::var_os("PROBE_ROOT").unwrap());`

## PROFILE (build: 1, test: 3)

- test: crates/iroha_test_network/src/lib.rs — `if let Ok(profile) = std::env::var("PROFILE") {`
- build: crates/iroha_test_samples/build.rs — `let profile = if env::var("PROFILE").ok().as_deref() == Some("release") {`
- test: integration_tests/src/binary_resolver.rs — `if let Ok(profile) = std::env::var("PROFILE")`
- test: integration_tests/src/kagami.rs — `let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());`

## PYTHON3 (test: 1)

- test: crates/sorafs_car/tests/taikai_viewer_cli.rs — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONDONTWRITEBYTECODE (prod: 1)

- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs — `if let Some(value) = env::var_os("PYTHONDONTWRITEBYTECODE")`

## PYTHONPATH (test: 1)

- test: crates/iroha_cli/bins/tests/cli_smoke.rs — `match env::var("PYTHONPATH") {`

## RAYON_NUM_THREADS (test: 7)

- test: crates/iroha_core_privacy/src/privacy_engines/zk_x509/stark/main_bounded_transform_tests.rs — `std::env::var("RAYON_NUM_THREADS").as_deref(),`
- test: crates/iroha_kagemusha_proof/tests/common/proof_fixtures/native_load_measurement.rs — `let requested = parse_workers(std::env::var("RAYON_NUM_THREADS").ok().as_deref())`
- test: crates/iroha_plonk_gadgets/src/ff/tests.rs — `std::env::var("RAYON_NUM_THREADS").ok(),`
- test: crates/iroha_plonk_gadgets/tests/m3_gates.rs — `let requested = std::env::var("RAYON_NUM_THREADS").ok();`
- test: crates/iroha_plonk_gadgets/tests/m3_gates.rs — `std::env::var("RAYON_NUM_THREADS").ok(),`
- test: crates/iroha_plonk_gadgets/tests/m3_gates/advice_schedule.rs — `let workers: usize = std::env::var("RAYON_NUM_THREADS").unwrap().parse().unwrap();`
- test: crates/iroha_plonk_gadgets/tests/pow5_fq_lane.rs — `std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".to_owned())`

## REBIND_LEAF (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/policy/cargo_image_tests.rs — `if {level} == 0 && std::env::var("REBIND_LEAF").unwrap() == "yes" {{`

## REPO_PROOF_DIGEST_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUSTC (test: 12)

- test: crates/iroha_fs/src/build_input_command_tests.rs — `std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),`
- test: crates/iroha_fs/src/private_files/executable_reader/tests.rs — `let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: crates/iroha_fs/src/private_files/executable_tests.rs — `let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: crates/iroha_fs/src/process/tests.rs — `let compiled = std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: crates/iroha_fs/src/sealed_batch/tests.rs — `let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: crates/iroha_fs/src/unix/inherited_descriptors_tests.rs — `let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: crates/iroha_fs/src/windows/process_owner/tests.rs — `let output = Compiler::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/build.rs — `let result = std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: crates/iroha_release_bundle/src/tests.rs — `std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),`
- test: crates/iroha_test_network/src/native_program_fixture.rs — `let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: crates/iroha_test_network/src/resolved_program_tests.rs — `process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`
- test: tools/norito_codegen_exporter/src/source_inventory/context_graph/tests.rs — `std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`

## RUSTC_WORKSPACE_WRAPPER (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs — `&& std::env::var_os("RUSTC_WORKSPACE_WRAPPER").is_none(),`

## RUSTC_WRAPPER (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs — `&& std::env::var_os("RUSTC_WRAPPER").is_none()`

## RUSTFLAGS (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs — `&& std::env::var_os("RUSTFLAGS").is_none()`

## RUSTUP_TOOLCHAIN (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/environment.rs — `assert!(std::env::var_os("RUSTUP_TOOLCHAIN").is_some());`

## RUST_LOG (prod: 2, test: 3)

- test: crates/iroha_test_network/src/lib.rs — `let original = env::var("RUST_LOG").ok();`
- test: crates/iroha_test_network/src/lib.rs — `let original = env::var("RUST_LOG").ok();`
- prod: crates/izanami/src/chaos.rs — `if let Ok(filter) = std::env::var("RUST_LOG") {`
- prod: crates/izanami/src/config.rs — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`
- test: integration_tests/tests/sumeragi_kagami_localnet.rs — `if std::env::var_os("RUST_LOG").is_none() {`

## RUST_MIN_STACK (test: 2)

- test: crates/iroha_core/src/privacy_release_evidence/tests.rs — `std::env::var("RUST_MIN_STACK").as_deref(),`
- test: crates/norito/src/json_cleanup_stack_tests.rs — `std::env::var_os("RUST_MIN_STACK").is_none(),`

## SCCP_LIVE_CAPTURE_DIR (test: 1)

- test: crates/iroha_sccp_rpc/tests/live_mainnet.rs — `if let Some(directory) = std::env::var_os("SCCP_LIVE_CAPTURE_DIR") {`

## SCCP_LIVE_REPLAY_FILE (test: 1)

- test: crates/iroha_sccp_rpc/tests/live_mainnet.rs — `let path = std::env::var_os("SCCP_LIVE_REPLAY_FILE").expect("set SCCP_LIVE_REPLAY_FILE");`

## SM_PERF_CPU_LABEL (prod: 2)

- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## SORAFS_CHECKPOINT_TEST_MODE (test: 1)

- test: crates/sorafs_node/src/durable_transaction_forwarder.rs — `std::env::var("SORAFS_CHECKPOINT_TEST_MODE").expect("parent supplies probe mode");`

## SORAFS_CHECKPOINT_TEST_ROOT (test: 1)

- test: crates/sorafs_node/src/durable_transaction_forwarder.rs — `std::env::var_os("SORAFS_CHECKPOINT_TEST_ROOT").expect("parent supplies state root"),`

## SORAFS_NODE_SKIP_INGEST_TESTS (test: 1)

- test: crates/sorafs_node/tests/cli.rs — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORAFS_QUALIFICATION_DIST (test: 1)

- test: crates/sorafs_node/tests/publication_roundtrip.rs — `std::env::var_os("SORAFS_QUALIFICATION_DIST").expect("set SORAFS_QUALIFICATION_DIST");`

## SORAFS_QUALIFICATION_PACKAGE (test: 1)

- test: crates/sorafs_node/tests/publication_roundtrip.rs — `std::env::var_os("SORAFS_QUALIFICATION_PACKAGE").expect("set SORAFS_QUALIFICATION_PACKAGE");`

## SORAFS_TORII_SKIP_INGEST_TESTS (test: 1)

- test: crates/iroha_torii/tests/sorafs_discovery.rs — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORA_CARS_FULL_PROOF_FIXTURE (test: 1)

- test: crates/iroha_data_model/tests/full_race_payload_codec.rs — `let path = std::env::var_os("SORA_CARS_FULL_PROOF_FIXTURE")`

## SORA_CARS_GAME_FIXTURE_OUTPUT (test: 1)

- test: crates/iroha_data_model/tests/game_v1_codec.rs — `std::env::var_os("SORA_CARS_GAME_FIXTURE_OUTPUT").expect("native output path");`

## SORA_CARS_GAME_FIXTURE_SEED (test: 1)

- test: crates/iroha_data_model/tests/game_v1_codec.rs — `let seed_path = std::env::var_os("SORA_CARS_GAME_FIXTURE_SEED").expect("semantic seed path");`

## SORA_CARS_NATIVE_PROOF_OUTPUT (test: 1)

- test: crates/iroha_core_privacy/src/execution_proofs/proof_tests.rs — `let output = std::env::var_os("SORA_CARS_NATIVE_PROOF_OUTPUT").map(std::path::PathBuf::from);`

## SORA_CARS_RESOURCE_FIXTURE_OUTPUT (test: 1)

- test: crates/iroha_data_model/tests/game_resources_v1_codec.rs — `let output = std::env::var_os("SORA_CARS_RESOURCE_FIXTURE_OUTPUT").expect("fresh output path");`

## SOURCE_DATE_EPOCH (test: 1)

- test: crates/iroha_release_bundle/src/bin/iroha_release_bundle/environment.rs — `assert_eq!(std::env::var("SOURCE_DATE_EPOCH").unwrap(), "123456789");`

## SUMERAGI_BASELINE_ARTIFACT_DIR (prod: 1)

- prod: crates/build-support/src/bin/sumeragi_baseline_report.rs — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`

## SUMERAGI_DA_ARTIFACT_DIR (prod: 1)

- prod: crates/build-support/src/bin/sumeragi_da_report.rs — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`

## SUMERAGI_FUZZ_SEEDS (test: 1)

- test: crates/iroha_sumeragi/src/machine/tests/fuzz.rs — `let seeds: u64 = std::env::var("SUMERAGI_FUZZ_SEEDS")`

## SUMERAGI_SIM_GAPS (prod: 1)

- prod: crates/iroha_sumeragi/src/sim/oracle.rs — `if std::env::var("SUMERAGI_SIM_GAPS").is_ok() && !sorted.is_empty() {`

## SUMERAGI_SIM_TRACE (prod: 1)

- prod: crates/iroha_sumeragi/src/sim/world.rs — `verbose: std::env::var("SUMERAGI_SIM_TRACE").is_ok(),`

## SystemRoot (prod: 2, test: 1)

- prod: crates/fastpq_prover/src/backend.rs — `env::var_os("SystemRoot").map(PathBuf::from)`
- test: crates/iroha_process_bridge/src/tests.rs — `if let Some(value) = std::env::var_os("SystemRoot") {`
- prod: crates/irohad/src/soracloud_runtime.rs — `if let Some(system_root) = std::env::var_os("SystemRoot") {`

## TAIRA_TESTNET_BEACON_FIXTURE_DIR (test: 1)

- test: crates/iroha_test_network/tests/support/production_beacon_bootstrap.rs — `let root = std::env::var_os("TAIRA_TESTNET_BEACON_FIXTURE_DIR")`

## TARGET (build: 2, prod: 1, tool: 2)

- prod: crates/build-support/src/lib.rs — `let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());`
- build: crates/iroha_release_bundle/build.rs — `let target = std::env::var("TARGET").expect("Cargo supplies the compilation target");`
- build: crates/ivm/build.rs — `if env::var("TARGET").is_ok_and(|target| target.contains("apple-darwin"))`
- tool: xtask/src/poseidon_bench.rs — `target: std::env::var("TARGET")`
- tool: xtask/src/stage1_bench.rs — `target: std::env::var("TARGET")`

## TEST_LOG_FILTER (prod: 1)

- prod: crates/iroha_logger/src/lib.rs — `filter: std::env::var("TEST_LOG_FILTER")`

## TEST_LOG_LEVEL (prod: 1)

- prod: crates/iroha_logger/src/lib.rs — `level: std::env::var("TEST_LOG_LEVEL")`

## TEST_NETWORK_BIN_SORAFS_CLI (test: 1)

- test: integration_tests/tests/sorafs_publication.rs — `let cli_binary = std::env::var_os("TEST_NETWORK_BIN_SORAFS_CLI")`

## TEST_NETWORK_CARGO (test: 1)

- test: crates/iroha_test_network/src/lib.rs — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TEST_NETWORK_IROHAD_FEATURES (test: 5)

- test: integration_tests/tests/nexus/cross_dataspace_zk_stark_localnet.rs — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/privacy_exact12_activation_network.rs — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/privacy_exact12_zk_x509_network.rs — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_ace_localnet.rs — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_stark_network.rs — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`

## TMPDIR (prod: 2, test: 1)

- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs — `let temporary = env::var_os("TMPDIR")`
- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs — `let temporary = env::var_os("TMPDIR")`
- test: crates/irohad/src/runtime_provider_broker/server_source_tests.rs — `assert_eq!(std::env::var_os("TMPDIR"), Some(cwd.into_os_string()));`

## TORII_MOCK_HARNESS_METRICS_PATH (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_RETRY_TOTAL (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## UPDATE_FIXTURES (test: 3)

- test: crates/iroha_core/tests/pin_registry.rs — `if env::var_os("UPDATE_FIXTURES").is_some() {`
- test: crates/iroha_core/tests/snapshots.rs — `let update = env::var("UPDATE_FIXTURES")`
- test: crates/iroha_torii/src/routing/pipeline_preflight_fixture_tests.rs — `if std::env::var_os("UPDATE_FIXTURES").is_some() {`

## USERPROFILE (prod: 1)

- prod: crates/iroha_cli/src/client_config.rs — `env::var_os("USERPROFILE").map(PathBuf::from)`

## VERGEN_CARGO_FEATURES (prod: 1)

- prod: crates/iroha_core/src/release_identity.rs — `option_env!("VERGEN_CARGO_FEATURES"),`

## VERGEN_CARGO_TARGET_TRIPLE (prod: 1)

- prod: crates/iroha_core/src/release_identity.rs — `option_env!("VERGEN_CARGO_TARGET_TRIPLE"),`

## VERGEN_GIT_SHA (prod: 2)

- prod: crates/iroha_core/src/release_identity.rs — `option_env!("VERGEN_GIT_SHA"),`
- prod: crates/iroha_kagami/src/main.rs — `.or(option_env!("VERGEN_GIT_SHA"))`

## X509_FORWARD_AB_FIRST_MODE (test: 1)

- test: crates/iroha_core_privacy/src/privacy_engines/zk_x509/stark/main_forward_transform_tests.rs — `let first = std::env::var("X509_FORWARD_AB_FIRST_MODE")`

## X509_MAPPING_RESIDENT_GIB (test: 1)

- test: crates/fastpq_prover/src/metal_exact_root_mapping_tests.rs — `let resident_gib: usize = std::env::var("X509_MAPPING_RESIDENT_GIB").map_or(0, |value| {`

## XDG_CACHE_HOME (prod: 1)

- prod: crates/musubi/src/cache.rs — `if let Some(root) = std::env::var_os("XDG_CACHE_HOME") {`

## XDG_DATA_HOME (prod: 1)

- prod: crates/iroha_wallet/src/custody.rs — `if let Some(base) = std::env::var_os("XDG_DATA_HOME") {`

## XDG_STATE_HOME (prod: 2)

- prod: crates/iroha_deploy/src/managed/workspace.rs — `let base = std::env::var_os("XDG_STATE_HOME")`
- prod: crates/musubi/src/command.rs — `let root = std::env::var_os("XDG_STATE_HOME")`

## XTASK_TEST_KAGAMI_BIN (test: 2)

- test: xtask/src/kagami_profiles.rs — `if std::env::var("XTASK_TEST_KAGAMI_BIN").is_err() {`
- test: xtask/src/kagami_profiles.rs — `let kagami_path = PathBuf::from(std::env::var("XTASK_TEST_KAGAMI_BIN").unwrap());`
