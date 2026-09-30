# Environment toggle inventory

_Last refreshed via `python3 scripts/inventory_env_toggles.py --json specs/agents/env_var_inventory.json --md specs/agents/env_var_inventory.md`_

Total references: **907** · Unique variables: **216**

## CARGO (prod: 2, test: 3)

- test: crates/iroha_test_network/src/lib.rs:2860 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- test: crates/norito_derive/tests/ui.rs:38 — `let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());`
- test: integration_tests/src/kagami.rs:141 — `let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());`
- prod: mochi/mochi-core/src/supervisor.rs:918 — `let cargo = env::var_os("CARGO")`
- prod: mochi/mochi-core/src/supervisor.rs:967 — `let cargo = env::var_os("CARGO")`

## CARGO_BIN_EXE_attachment_sanitizer (test: 9)

- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:91 — `let mut cmd = Command::new(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:132 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:155 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:293 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:316 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:337 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:352 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:367 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:383 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`

## CARGO_BIN_EXE_iroha (test: 2)

- test: crates/iroha_cli/bins/tests/cli_smoke.rs:55 — `env!("CARGO_BIN_EXE_iroha")`
- test: crates/iroha_cli/bins/tests/taikai_policy.rs:16 — `env!("CARGO_BIN_EXE_iroha")`

## CARGO_BIN_EXE_iroha_monitor (test: 4)

- test: crates/iroha_monitor/tests/attach_render.rs:10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/http_limits.rs:10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/invalid_credentials.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/smoke.rs:21 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## CARGO_BIN_EXE_kagami (test: 3)

- test: crates/iroha_kagami/tests/common/mod.rs:19 — `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- test: crates/iroha_kagami/tests/pop_embed.rs:30 — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- test: integration_tests/src/kagami.rs:61 — `if let Ok(path) = env::var("CARGO_BIN_EXE_kagami") {`

## CARGO_BIN_EXE_kagami_mock (test: 1)

- test: mochi/mochi-integration/tests/supervisor.rs:35 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## CARGO_BIN_EXE_koto (test: 4)

- test: crates/kotodama_toolchain/tests/cli_smoke.rs:18 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs:61 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs:86 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs:114 — `let bin = env!("CARGO_BIN_EXE_koto");`

## CARGO_BIN_EXE_sorafs_chunk_dump (test: 1)

- test: crates/sorafs_chunker/tests/one_gib.rs:93 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## CARGO_BIN_EXE_sorafs_fetch (test: 1)

- test: crates/sorafs_car/tests/sorafs_fetch_cli.rs:30 — `AssertCommand::new(env!("CARGO_BIN_EXE_sorafs_fetch"))`

## CARGO_BUILD_JOBS (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs:1127 — `std::env::var("CARGO_BUILD_JOBS").ok().as_deref() == Some("1")`

## CARGO_BUILD_TARGET (tool: 2)

- tool: xtask/src/poseidon_bench.rs:79 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- tool: xtask/src/stage1_bench.rs:54 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_FEATURE (prod: 1)

- prod: crates/build-support/src/lib.rs:31 — `let parsed_features = env::var("CARGO_CFG_FEATURE")`

## CARGO_CFG_TARGET_ARCH (build: 2, prod: 2, tool: 2)

- build: crates/gpuzstd_metal/build.rs:9 — `let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:653 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:683 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- build: crates/norito/accelerators/jsonstage1_metal/build.rs:9 — `let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();`
- tool: xtask/src/poseidon_bench.rs:80 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- tool: xtask/src/stage1_bench.rs:55 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## CARGO_CFG_TARGET_OS (build: 6, prod: 2, tool: 2)

- build: crates/fastpq_prover/build.rs:39 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/gpuzstd_cuda/build.rs:28 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/gpuzstd_metal/build.rs:8 — `let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:654 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:684 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- build: crates/ivm/build.rs:676 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:31 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/norito/accelerators/jsonstage1_metal/build.rs:8 — `let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- tool: xtask/src/poseidon_bench.rs:82 — `os: std::env::var("CARGO_CFG_TARGET_OS")`
- tool: xtask/src/stage1_bench.rs:57 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## CARGO_ENCODED_RUSTFLAGS (build: 1)

- build: crates/iroha_sumeragi/build.rs:28 — `let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();`

## CARGO_FEATURE_CUDA_KERNEL (build: 2)

- build: crates/gpuzstd_cuda/build.rs:14 — `if env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_none() {`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:15 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (build: 1)

- build: crates/fastpq_prover/build.rs:19 — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## CARGO_FEATURE_METAL (test: 1)

- test: crates/ivm/build.rs:90 — `&& env::var_os("CARGO_FEATURE_METAL").is_some()`

## CARGO_FEATURE_MUTATION_TESTING (build: 1)

- build: crates/iroha_sumeragi/build.rs:26 — `if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {`

## CARGO_INCREMENTAL (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs:1128 — `&& std::env::var("CARGO_INCREMENTAL").ok().as_deref() == Some("0")`

## CARGO_MANIFEST_DIR (bench: 2, build: 4, debug: 1, example: 1, prod: 42, test: 428, tool: 6)

- prod: crates/build-support/src/lib.rs:138 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);`
- prod: crates/connect_norito_bridge/src/bin/swift_parity_regen.rs:275 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/bridge_tail_tests.rs:234 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/archives.rs:293 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/archives/tests.rs:243 — `let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/archives/tests.rs:267 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/enrolled_open/tests.rs:542 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_device_bridge_v1/control_payload.rs:1033 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_device_bridge_v1/receiver_payload.rs:405 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_device_bridge_v1/sender_payload.rs:1501 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_device_bridge_v1/sender_payload.rs:1697 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_device_bridge_v1/sender_payload.rs:1761 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_device_bridge_v1/sender_payload.rs:1917 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_fixture_tests.rs:32 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_fixture_tests.rs:502 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/kagemusha_mobile_bootstrap_v1_tests.rs:27 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_mobile_bootstrap_v1_tests.rs:32 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_reserve_finality_v1_tests.rs:24 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_reserve_finality_v1_tests.rs:111 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_sender_release_evidence_tests.rs:622 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/kagemusha_testnet_finality_chain_v1.rs:121 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_testnet_finality_chain_v1.rs:125 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_testnet_native_mint_runtime_v1_tests.rs:27 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/kagemusha_testnet_native_mint_runtime_v1_tests.rs:31 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/sorafs_tests.rs:149 — `fs::read(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))`
- test: crates/connect_norito_bridge/src/validation_fee_policy_proof_bridge_tests.rs:63 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/validation_fee_policy_proof_bridge_tests.rs:67 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/fastpq_prover/src/backend/compact_protocol/shared_openings/compact_diagnostic.rs:399 — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/src/backend/compact_public_api.rs:697 — `let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/fastpq_prover/src/backend/compact_public_diagnostic.rs:148 — `let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/src/backend/compact_public_transfer.rs:890 — `let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/fastpq_prover/src/backend/compact_quantity_diagnostic.rs:107 — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/fastpq_prover/src/backend/compact_quantity_diagnostic.rs:294 — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/src/backend/compact_quantity_producer/tests.rs:663 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/fastpq_prover/src/backend/deep_prover/diagnostic_artifact.rs:15 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/fastpq-proof-diagnostics")`
- test: crates/fastpq_prover/src/bin/fastpq_cuda_bench.rs:1994 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/src/bin/fastpq_cuda_bench.rs:2001 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");`
- prod: crates/fastpq_prover/src/poseidon_manifest.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:22 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:40 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:59 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:64 — `let field_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/field.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:69 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- test: crates/fastpq_prover/tests/support/offline_compact_capture.rs:341 — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/support/offline_compact_single.rs:376 — `let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/support/offline_compact_two.rs:337 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/fastpq-proof-diagnostics");`
- test: crates/fastpq_prover/tests/trace_commitment.rs:22 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- test: crates/fastpq_prover/tests/transcript_replay.rs:25 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/client.rs:24617 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha/src/client/sumeragi_api_separation_tests.rs:132 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha/src/sm.rs:184 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/tests/sm_signing.rs:30 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/bins/tests/cli_smoke.rs:163 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/bins/tests/cli_smoke.rs:4948 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/bins/tests/sorafs_validate_cli.rs:36 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/commands/sorafs.rs:12305 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/commands/sorafs/toolkit/validation/final_promotion_receipt_tests.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/commands/sorafs/toolkit/validation/tests.rs:21 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/compute.rs:746 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_cli/src/main_shared.rs:1784 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/iroha_cli/src/soracloud.rs:21593 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/soracloud/tests/part_01.rs:46 — `let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");`
- test: crates/iroha_cli/src/soracloud/tests/part_01.rs:718 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/soracloud/tests/part_01.rs:836 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/soracloud/tests/part_01.rs:973 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/taira.rs:10666 — `.tempdir_in(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/taira.rs:12105 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/taira_public_reset.rs:691 — `let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");`
- test: crates/iroha_cli/src/taira_public_reset_host.rs:20119 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/taira_public_reset_host.rs:20168 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_config/src/parameters/user.rs:5522 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/src/parameters/user.rs:34351 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/autoscale_config.rs:10 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/checked_in_profiles_parse.rs:29 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/connect_relay_strategy_hard_cut.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/da_ingest_compute_limit.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/fastpq_queue_overrides.rs:13 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:34 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:748 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:879 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:1687 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:1726 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/kaigi_authorization_config_v1.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml"),`
- test: crates/iroha_config/tests/kura_retention_hard_cut.rs:28 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/minamoto_profile.rs:10 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/native_context_archive_limit.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/network_scion_hard_cut.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/nexus_staking_bounds.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/nexus_staking_withdraw_grace_hard_cut.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/operator_auth_bootstrap_hard_cut.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/p2p_hard_cut.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs:58 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_signature_batch_alias_hard_cut.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/push_provider_credentials.rs:10 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/queue_plan_retirement.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml"),`
- test: crates/iroha_config/tests/sccp_node_config.rs:19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml")`
- test: crates/iroha_config/tests/sorafs_gateway_runtime_providers.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_governance_dag_runtime_signer.rs:8 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_native_transaction_signers.rs:8 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_por_replay_archive.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_provider_ingest_finalized_archive.rs:8 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_reputation_finalized_archive.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_storage_pin_aliases.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_stream_token_runtime_signer.rs:8 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/soranet_privacy_ingest_hard_cut.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sumeragi_core_config.rs:15 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- test: crates/iroha_config/tests/transaction_gossip_config.rs:10 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/transaction_ingress_limits.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/trusted_peers_pop_validation.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- bench: crates/iroha_core/benches/blocks/common.rs:140 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- bench: crates/iroha_core/benches/validation.rs:121 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- example: crates/iroha_core/examples/generate_parity_fixtures.rs:26 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/src/block.rs:8067 — `let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/executor.rs:17601 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- test: crates/iroha_core/src/executor_contract_dispatch_tests.rs:260 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- test: crates/iroha_core/src/fastpq/mod.rs:2430 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/frame_identity_tests.rs:14 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/query/final_promotion_authority/observation/tests/fixture.rs:41 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/query/provider_ingest_finalized/tests/frame_identity_tests.rs:11 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/query/reputation_finalized/tests/frame_identity_tests.rs:11 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/asset/core_numeric_mutation_tests.rs:4 — `let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2541 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2545 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs:6805 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs:6813 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs:6821 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs:16092 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/sorafs_final_promotion_authority/tests/check.rs:22 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_core/src/smartcontracts/isi/sorafs_provider_admission/test_fixture.rs:42 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/triggers/set_frame_identity_tests.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/world.rs:20077 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/world.rs:20164 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/world_parliament_due_effect_tests.rs:346 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/world_parliament_due_effect_tests.rs:493 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/ivm/host.rs:12491 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/ivm/host.rs:12499 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/ivm/host.rs:14838 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/state.rs:34716 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- test: crates/iroha_core/src/state/deserialize_world_kagemusha_registry_tests.rs:51 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/streaming.rs:3061 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/src/tx/sandbox_state_tests.rs:64 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/default_domain_independence.rs:32 — `let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/pin_registry.rs:122 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);`
- test: crates/iroha_core/tests/pin_registry.rs:1462 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/snapshots.rs:32 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/sumeragi_doc_sync.rs:87 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- debug: crates/iroha_core_privacy/src/privacy_engines/zk_x509/engine_prover_diagnostic.rs:161 — `let repository = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core_zk/src/kagemusha_v1_recursion/frame_identity_tests.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_v1_state/coordinator_operation_store_tests.rs:708 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_v1_state/coordinator_operation_store_tests.rs:718 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_v1_state/outgoing_frame_identity_tests.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/kagemusha_v1_state/state_frame_identity_tests.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/stark/bfv_full_bootstrap_tests.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core_zk/src/stark/frame_identity_tests.rs:11 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_crypto/tests/confidential_keyset_vectors.rs:48 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm2_fixture_vectors.rs:49 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm_cli_matrix.rs:15 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:33 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:37 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:41 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/bin/kagemusha_verified_receipt_v1.rs:1441 — `let repo = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_data_model/src/bin/privacy_exact12_fixtures.rs:103 — `let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/governance/types/tests/build_closure.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/governance/types/tests/build_closure.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/identifier.rs:1005 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/isi/escrow.rs:743 — `let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_data_model/src/isi/generated_record_identity_tests/sorafs_values.rs:15 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/isi/governance.rs:925 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/isi/registry.rs:492 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/isi/registry.rs:499 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_enrolled_open_selector_v1.rs:199 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_mobile_bootstrap_freshness_v1_tests.rs:23 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_mobile_bootstrap_freshness_v1_tests.rs:28 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_mobile_bootstrap_v1_tests.rs:23 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_mobile_bootstrap_v1_tests.rs:28 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_release_v1_tests.rs:2185 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_release_v1_tests.rs:2197 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_release_v1_tests.rs:2216 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_release_v1_tests.rs:2272 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/kagemusha/kagemusha_release_v1_tests.rs:2280 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1385 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1736 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1790 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/qr_stream.rs:853 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_data_model/src/soranet/vpn.rs:4417 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- prod: crates/iroha_data_model/src/testing/axt.rs:14 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs:18 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs:22 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/cancel_asset_lock.rs:43 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed_norito_rpc_fixture_tests.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed_norito_rpc_fixture_tests.rs:20 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/account_address_vectors.rs:106 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/address_curve_registry.rs:27 — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/manual_frame_identity/block_signature.rs:77 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/identifier.rs:82 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/proof.rs:374 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/protocol.rs:383 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/manual_frame_identity/scalar.rs:226 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:22 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:26 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:30 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:34 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:38 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:42 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:46 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:50 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:54 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:58 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:188 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/runtime_doc_sync.rs:6 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:55 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:59 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:63 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:67 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:71 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:75 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:79 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:105 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:109 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:113 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:117 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:121 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:125 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:129 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:133 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:2284 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/validator_staking_sdk_fixtures.rs:306 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_deploy/tests/definition.rs:71 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")`
- test: crates/iroha_deploy/tests/definition.rs:75 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs:766 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs:862 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs:988 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs:1049 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/genesis_instructions_json/tests.rs:1157 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/genesis_tail_tests.rs:21 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/genesis_tail_tests.rs:108 — `let genesis_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/lib.rs:3221 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);`
- test: crates/iroha_genesis/src/lib.rs:3230 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/lib.rs:3524 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:3577 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:3672 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:3741 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_i18n/src/lib.rs:457 — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- test: crates/iroha_js_codec/src/manifest.rs:149 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_js_codec/src/manifest.rs:153 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_js_codec/src/manifest.rs:157 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_js_host/src/lib.rs:9741 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_js_host/src/lib.rs:10923 — `let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_js_host/src/lib.rs:12405 — `let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/codec.rs:721 — `concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec/account.json"),`
- test: crates/iroha_kagami/src/codec.rs:742 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/codec.rs:753 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/codec.rs:773 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/genesis/generate.rs:1225 — `let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/generate.rs:1245 — `let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/generate.rs:1300 — `let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/prepared.rs:760 — `let defaults = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/genesis/sign.rs:2023 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:2718 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:2752 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:2776 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:2823 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:2851 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:3443 — `let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:5340 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/genesis/sign.rs:5348 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:5368 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/iroha_kagami/src/localnet.rs:625 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_kagami/src/localnet.rs:4804 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_kagami/src/localnet.rs:4808 — `|| PathBuf::from(env!("CARGO_MANIFEST_DIR")),`
- test: crates/iroha_kagami/src/wizard.rs:1581 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/tests/codec.rs:11 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- test: crates/iroha_model_base/src/name/scratch_tests.rs:35 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_model_base/src/name/scratch_tests.rs:38 — `let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));`
- test: crates/iroha_musubi_service/src/tests/wire_fixtures.rs:226 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/src/frame_identity_tests.rs:12 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/src/frame_identity_tests.rs:22 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/src/frame_identity_tests.rs:138 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/src/frame_identity_tests.rs:161 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_p2p/tests/production_source_reachability.rs:278 — `let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_p2p/tests/production_source_reachability.rs:347 — `let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_sccp/tests/ethereum_light_client.rs:1107 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp")`
- test: crates/iroha_sccp/tests/v1_vectors.rs:1828 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_sccp_rpc/tests/ton_liteclient.rs:61 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/rpc/ton/transport")`
- test: crates/iroha_sccp_rpc/tests/transport.rs:331 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sccp/rpc/transport")`
- test: crates/iroha_sccp_wallet/tests/evm_pure.rs:62 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_sumeragi/tests/spec.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_swarm/tests/default_compose_soranet.rs:23 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_test_network/src/lib.rs:843 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- build: crates/iroha_test_samples/build.rs:19 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_torii/src/da/tests/replay_manifest_and_metrics.rs:1859 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- test: crates/iroha_torii/src/identifier_resolution.rs:753 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/openapi/tests/sorafs_contracts.rs:45 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/openapi/tests/vpn_da.rs:6 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing.rs:53302 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing.rs:53339 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing.rs:54104 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing.rs:54546 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/routing/pipeline_preflight_fixture_tests.rs:27 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/soracloud.rs:8333 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/admission.rs:698 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs:26603 — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs:37879 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/tests/lib_routed_reads/routed_read_source_bounds.rs:483 — `let mut pending = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];`
- test: crates/iroha_torii/src/zk_attachments/tests.rs:611 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/tests/accounts_portfolio.rs:91 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/tests/sorafs_discovery.rs:1557 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_zkp_halo2/src/vega/canonical_mc_exact.rs:246 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc.rs:531 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc.rs:535 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/prover_key.rs:133 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verifier_key.rs:880 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verifier_key.rs:884 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verify.rs:927 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verify.rs:931 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/incremental_source_phase23_tests.rs:1813 — `let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/vega/zk_ams/mkhe");`
- test: crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/phase23_rns_link.rs:3354 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/vega/zk_ams/mkhe");`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs:5 — `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega/engine.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs:6 — `const FACADE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs:8 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:7 — `const CRATE_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:8 — `const VEGA_FACADE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:14 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:18 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:22 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/irohad/src/external_software_signer/consensus_threshold.rs:1223 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main.rs:7153 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs:7192 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs:10004 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main/node_file_tests.rs:42 — `.tempdir_in(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main/shared_sorafs_provider_cache_tests.rs:48 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/node_secrets/tests.rs:25 — `.tempdir_in(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/runtime_provider_registry.rs:4960 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime/tests/part_01.rs:1338 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime/tests/part_01.rs:1348 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/sorafs_provider_ingest_finalized_query.rs:716 — `let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");`
- test: crates/irohad/src/taira_runtime_signer.rs:1771 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- build: crates/ivm/build.rs:398 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- build: crates/ivm/build.rs:529 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- prod: crates/ivm/src/bin/gen_abi_hash_doc.rs:16 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: crates/ivm/src/bin/gen_header_doc.rs:113 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_pointer_types_doc.rs:89 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_syscalls_doc.rs:751 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/ivm_fixture_export.rs:83 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/ivm_prebuild.rs:15 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/src/core_host.rs:2131 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/ivm/src/predecoder_fixtures.rs:218 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- test: crates/ivm/tests/axt_descriptor_builder.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm/tests/docs_consistency.rs:3 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/ivm_abi_doc_sync.rs:3 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/ivm_header_doc_sync.rs:41 — `let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama.rs:1841 — `let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kotodama_lang/src/samples");`
- test: crates/ivm/tests/kotodama_argument_record.rs:44 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama_documentation_examples.rs:21 — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/tests/numeric_v1_sdk_fixture.rs:13 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/numeric_v1_sdk_fixture.rs:20 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/pointer_types_doc_generated.rs:6 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- test: crates/ivm/tests/pointer_types_doc_generated_ivm_md.rs:5 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/repository_ivm_artifacts.rs:20 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/syscalls_doc_generated.rs:6 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_doc_sync.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_gas_names.rs:10 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/tlv_examples.rs:172 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm_abi/src/axt.rs:2330 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm_abi/src/axt.rs:2394 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm_artifact_admission/src/lib.rs:873 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/kotodama_lang/src/compiler/tests.rs:3449 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/kotodama_lang/src/compiler/tests.rs:3457 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/kotodama_lang/src/diagnostic.rs:656 — `let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/kotodama_lang/src/doc_consistency.rs:17 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/kotodama_lang/src/doc_consistency.rs:210 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/kotodama_lang/tests/documentation_fences.rs:10 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/kotodama_toolchain/tests/cli_smoke.rs:6 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- build: crates/norito/build.rs:16 — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- prod: crates/norito/src/bin/norito_regen_goldens.rs:9 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/src/streaming/repo_fixture_test.rs:4 — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/aos_ncb_more_golden.rs:185 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- test: crates/norito/tests/json_golden_loader.rs:13 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:332 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:365 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:522 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:629 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- test: crates/norito/tests/ncb_enum_large_fixture.rs:33 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs:594 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs:895 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/provider_admission_fixtures.rs:1022 — `let committed_dir = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_car/src/bin/soranet_trustless_verifier.rs:143 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/reference.rs:292 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/capacity_simulation_toolkit.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/da_reconstruct_cli.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:48 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:958 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:1151 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/manifest_builder_cli.rs:146 — `let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/taikai_viewer_cli.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/trustless_verifier.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_chunker/src/bin/export_vectors.rs:431 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_chunker/tests/backpressure.rs:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_chunker/tests/vectors.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference.rs:6615 — `let absolute = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference_ffi.rs:1780 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/orderbook_fixtures.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/pdp_fixtures.rs:17 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/por_fixtures.rs:18 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/replication_order_fixtures.rs:6 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_node/tests/cli.rs:495 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/src/lib.rs:6178 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs:6285 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs:7944 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/tests/orchestrator_parity.rs:160 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs:2586 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs:3853 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli/pdp.rs:20 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/soranet_pq/tests/kat_vectors.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- test: integration_tests/src/bin/refresh_nexus_streaming_fixtures.rs:405 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: integration_tests/src/binary_resolver.rs:151 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")`
- test: integration_tests/src/kagami.rs:170 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/src/sorafs_gateway_capability_refusal.rs:141 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- test: integration_tests/src/sorafs_gateway_conformance.rs:1273 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/alias_registry_bootstrap_network.rs:909 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:66 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:108 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:154 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/atomic_private_settlement_localnet.rs:4131 — `env!("CARGO_MANIFEST_DIR"),`
- test: integration_tests/tests/nexus/cbdc_rollout_bundle.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/cbdc_whitelist.rs:25 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/lane_registry.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/norito_burn_fixture.rs:29 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/privacy_exact12_zk_x509_network.rs:151 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(RESOURCE_CERTIFICATE_RELATIVE_PATH);`
- test: integration_tests/tests/sorafs_orchestrator_parity.rs:338 — `let support = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support");`
- test: integration_tests/tests/sorafs_publication.rs:313 — `let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target");`
- test: integration_tests/tests/streaming/mod.rs:404 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/compose.rs:1621 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1625 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1629 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1633 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1637 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-core/src/supervisor.rs:115 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-core/src/supervisor.rs:724 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- prod: mochi/mochi-core/src/supervisor.rs:910 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs:9 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs:16 — `let checked = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs:10 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs:17 — `let checked = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/tests/supervisor.rs:220 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- prod: mochi/mochi-ui-egui/src/gui.rs:120 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:124 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:128 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:132 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:136 — `env!("CARGO_MANIFEST_DIR"),`
- tool: tools/norito_codegen_exporter/src/norito_rpc.rs:94 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: tools/soranet-handshake-harness/tests/fixtures_verify.rs:19 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/interop_parity.rs:65 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/perf_gate.rs:156 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- tool: xtask/src/bin/control_plane_mock.rs:341 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/main.rs:14005 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/nexus.rs:185 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/src/nexus_lane_maintenance.rs:255 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/sorafs/gateway_fixture.rs:26 — `env!("CARGO_MANIFEST_DIR"),`
- tool: xtask/src/sorafs/gateway_fixture.rs:30 — `env!("CARGO_MANIFEST_DIR"),`
- test: xtask/tests/address_vectors.rs:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/android_dashboard_parity_cli.rs:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/codec_rans_tables.rs:16 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/da_proof_bench.rs:6 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/iso_bridge_lint.rs:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/ministry_agenda.rs:6 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soradns_cli.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/sorafs_fetch_fixture.rs:6 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_bug_bounty.rs:9 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_billing.rs:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_m1.rs:9 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_m2.rs:14 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:8 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:70 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:119 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:186 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:285 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:332 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:472 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_bundle_check.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_entropy_bench.rs:6 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_NAME (test: 1)

- test: crates/norito_derive/tests/ui.rs:26 — `let crate_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "norito_derive".to_owned());`

## CARGO_PKG_VERSION (prod: 15, test: 2, tool: 2)

- test: crates/iroha_cli/bins/tests/cli_smoke.rs:701 — `let expected_version = env!("CARGO_PKG_VERSION");`
- prod: crates/iroha_cli/src/main_shared.rs:99 — `env!("CARGO_PKG_VERSION"),`
- prod: crates/iroha_cli/src/main_shared.rs:394 — `#[command(name = "iroha", version = env!("CARGO_PKG_VERSION"), author)]`
- prod: crates/iroha_core/src/release_identity.rs:249 — `env!("CARGO_PKG_VERSION"),`
- prod: crates/iroha_js_host/src/lib.rs:3738 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_kagami/src/genesis/generate.rs:579 — `env!("CARGO_PKG_VERSION")`
- prod: crates/iroha_kagami/src/verify.rs:78 — `writeln!(writer, "kagami_version: {}", env!("CARGO_PKG_VERSION"))?;`
- prod: crates/iroha_sccp_rpc/src/http.rs:87 — `const USER_AGENT: &str = concat!("iroha-sccp-rpc/", env!("CARGO_PKG_VERSION"));`
- prod: crates/iroha_storage_client/src/client.rs:446 — `("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION"))),`
- prod: crates/iroha_torii/src/mcp.rs:1372 — `Value::String(env!("CARGO_PKG_VERSION").to_owned()),`
- prod: crates/iroha_torii/src/mcp/protocol.rs:562 — `"version": (env!("CARGO_PKG_VERSION"))`
- prod: crates/irohad/src/main.rs:827 — `version = env!("CARGO_PKG_VERSION"),`
- test: crates/irohad/src/main.rs:8102 — `env!("CARGO_PKG_VERSION"),`
- prod: crates/kotodama_lang/src/compiler.rs:119 — `const COMPILER_FINGERPRINT: &str = concat!("kotodama_lang/", env!("CARGO_PKG_VERSION"));`
- prod: crates/musubi/src/command.rs:132 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/sorafs_car/src/bin/sorafs_fetch.rs:1145 — `Value::from(env!("CARGO_PKG_VERSION")),`
- prod: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:148 — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/sora-vpn-helper/src/main.rs:106 — `const VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/telemetry-schema-diff/src/main.rs:221 — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs:1133 — `&& std::env::var("CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS")`

## CARGO_PROFILE_RELEASE_CODEGEN_UNITS (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs:1129 — `&& std::env::var("CARGO_PROFILE_RELEASE_CODEGEN_UNITS")`

## CARGO_TARGET_DIR (prod: 3, test: 6, tool: 2)

- prod: crates/iroha_kagami/src/localnet.rs:4828 — `let target_dir = resolve_target_dir(&repo_root, env::var("CARGO_TARGET_DIR").ok().as_deref());`
- test: crates/iroha_test_network/src/lib.rs:1246 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: crates/iroha_test_network/src/lib.rs:2252 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/binary_resolver.rs:70 — `if let Some(target_root) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)`
- test: integration_tests/src/binary_resolver.rs:205 — `if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs:96 — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs:120 — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- prod: mochi/mochi-core/src/supervisor.rs:944 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:990 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- tool: xtask/src/kagami_profiles.rs:1532 — `if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {`
- tool: xtask/src/mochi.rs:381 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## CARGO_TARGET_TMPDIR (test: 1)

- test: crates/kotodama_toolchain/tests/cli_smoke.rs:13 — `PathBuf::from(env!("CARGO_TARGET_TMPDIR"))`

## CREDENTIALS_DIRECTORY (prod: 2)

- prod: crates/irohad/bins/src/bin/sorafs_external_software_signer.rs:88 — `let directory = env::var_os("CREDENTIALS_DIRECTORY")`
- prod: crates/irohad/bins/src/bin/sorafs_external_software_signer.rs:607 — `let credential_directory = env::var_os("CREDENTIALS_DIRECTORY").map(PathBuf::from);`

## CRYPTO_SM_INTRINSICS (bench: 1)

- bench: crates/iroha_crypto/benches/sm_perf.rs:165 — `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`

## CUDA_HOME (build: 5)

- build: crates/fastpq_prover/build.rs:340 — `env::var_os("CUDA_HOME")`
- build: crates/gpuzstd_cuda/build.rs:80 — `for root in env::var_os("CUDA_HOME")`
- build: crates/gpuzstd_cuda/build.rs:99 — `let root = env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:85 — `for root in env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:104 — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (build: 5)

- build: crates/fastpq_prover/build.rs:341 — `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/gpuzstd_cuda/build.rs:82 — `.chain(env::var_os("CUDA_PATH"))`
- build: crates/gpuzstd_cuda/build.rs:100 — `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:87 — `.chain(env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:105 — `.or_else(|| env::var_os("CUDA_PATH"))`

## CXX (build: 4)

- build: crates/fastpq_prover/build.rs:384 — `env::var_os("CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs:130 — `env::var_os("CXX").is_some()`
- build: crates/ivm/build.rs:833 — `env::var_os("CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:135 — `env::var_os("CXX").is_some()`

## DOCS_RS (build: 1)

- build: crates/norito/build.rs:4 — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (bench: 1)

- bench: crates/norito/benches/enum_packed_bench.rs:74 — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_CUDA_REQUIRE (test: 5)

- test: crates/fastpq_prover/src/fastpq_cuda.rs:1292 — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_none(),`
- test: crates/fastpq_prover/src/fastpq_cuda.rs:1316 — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_some(),`
- test: crates/fastpq_prover/src/fastpq_cuda.rs:1354 — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_some(),`
- test: crates/fastpq_prover/src/fastpq_cuda.rs:1373 — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_some(),`
- test: crates/fastpq_prover/src/fastpq_cuda.rs:1403 — `std::env::var_os("FASTPQ_CUDA_REQUIRE").is_some(),`

## FASTPQ_METAL_LIB (prod: 1, test: 1)

- prod: crates/fastpq_prover/src/backend.rs:1259 — `option_env!("FASTPQ_METAL_LIB")`
- test: crates/fastpq_prover/src/metal.rs:3087 — `option_env!("FASTPQ_METAL_LIB"),`

## FASTPQ_RESOURCE_OUTPUT_DIR (test: 1)

- test: crates/fastpq_prover/tests/resource_profile.rs:180 — `let output_dir = std::env::var_os("FASTPQ_RESOURCE_OUTPUT_DIR").map(PathBuf::from);`

## FASTPQ_SKIP_GPU_BUILD (build: 1)

- build: crates/fastpq_prover/build.rs:40 — `let skip_gpu_build = env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some();`

## FASTPQ_TEST_FIXED_SMT_ARTIFACT (prod: 1)

- prod: crates/fastpq_prover/src/backend/deep_prover/diagnostic_artifact.rs:174 — `std::env::var_os("FASTPQ_TEST_FIXED_SMT_ARTIFACT").expect("FASTPQ_TEST_FIXED_SMT_ARTIFACT"),`

## FASTPQ_UPDATE_FIXTURES (test: 1)

- test: crates/fastpq_prover/tests/common/mod.rs:12 — `fixture_update_requested_from(std::env::var_os("FASTPQ_UPDATE_FIXTURES").as_deref())`

## GENESIS_DEBUG_MODE (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs:16 — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## GENESIS_DEBUG_PAYLOAD (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs:135 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## GITHUB_STEP_SUMMARY (prod: 2)

- prod: crates/iroha_crypto/src/bin/gost_perf_check.rs:28 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:185 — `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## GPUZSTD_CUDA_ARCH (build: 1)

- build: crates/gpuzstd_cuda/build.rs:44 — `if let Some(arch_flag) = env::var_os("GPUZSTD_CUDA_ARCH") {`

## GPUZSTD_CUDA_REQUIRE (test: 3)

- test: crates/gpuzstd_cuda/src/lib.rs:278 — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some() {`
- test: crates/gpuzstd_cuda/src/lib.rs:675 — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_none() {`
- test: crates/norito/src/core/gpu_zstd.rs:674 — `std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some()`

## GPUZSTD_CUDA_SKIP_BUILD (build: 1)

- build: crates/gpuzstd_cuda/build.rs:17 — `if env::var_os("GPUZSTD_CUDA_SKIP_BUILD").is_some() {`

## HOME (prod: 8, test: 4)

- prod: crates/iroha_cli/src/client_config.rs:32 — `env::var_os("HOME").map(PathBuf::from)`
- test: crates/iroha_cli/src/taira_public_reset_config.rs:752 — `let home = std::env::var_os("HOME").expect("native operator test home");`
- test: crates/iroha_cli/src/taira_public_reset_context_release.rs:382 — `.tempdir_in(std::env::var_os("HOME").unwrap())`
- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:1366 — `if let Some(home) = env::var_os("HOME") {`
- test: crates/iroha_test_network/tests/support/production_beacon_bootstrap.rs:2240 — `let home = std::env::var_os("HOME").ok_or_else(|| eyre!("HOME absent"))?;`
- prod: crates/iroha_wallet/src/custody.rs:139 — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/iroha_wallet/src/custody.rs:149 — `std::env::var_os("HOME").map(PathBuf::from),`
- test: crates/irohad/src/soracloud_runtime/tests/part_05.rs:557 — `if std::env::var_os("HOME").is_none() {`
- prod: crates/musubi/src/cache.rs:154 — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/musubi/src/cache.rs:171 — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/musubi/src/command.rs:3174 — `let root = std::env::var_os("HOME").map(PathBuf::from).map(|path| {`
- prod: crates/musubi/src/command.rs:3185 — `std::env::var_os("HOME")`

## HOST_CXX (build: 4)

- build: crates/fastpq_prover/build.rs:385 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs:131 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/ivm/build.rs:834 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:136 — `|| env::var_os("HOST_CXX").is_some()`

## IROHA_ACCOUNT_ID (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:454 — `account_id: std::env::var("IROHA_ACCOUNT_ID").ok(),`

## IROHA_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs:7300 — `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## IROHA_API_BASE (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:447 — `api_base: std::env::var("IROHA_API_BASE")`

## IROHA_BFV_CONFORMANCE_OUTPUT (prod: 1)

- prod: crates/iroha_crypto/src/fhe_bfv/conformance.rs:448 — `std::env::var_os("IROHA_BFV_CONFORMANCE_OUTPUT")`

## IROHA_CHAIN_ID (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:452 — `chain_id: std::env::var("IROHA_CHAIN_ID")`

## IROHA_CLASSED_STARK_OUTPUT (test: 2)

- test: crates/iroha_core_privacy/src/execution_proofs/classed_race_v1/proof_tests.rs:1040 — `if let Some(dir) = std::env::var_os("IROHA_CLASSED_STARK_OUTPUT") {`
- test: crates/iroha_core_privacy/src/execution_proofs/classed_race_v1/proof_tests.rs:1138 — `std::env::var_os("IROHA_CLASSED_STARK_OUTPUT")`

## IROHA_CONF_GAS_SEED (test: 1)

- test: crates/iroha_test_samples/src/lib.rs:73 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_CONNECT_ACCOUNT_SEED_HEX (example: 1)

- example: crates/iroha_torii_shared/examples/connect_wallet.rs:74 — `let account_seed_hex = std::env::var("IROHA_CONNECT_ACCOUNT_SEED_HEX")`

## IROHA_CRYPTO_FIXED_ALGORITHM (test: 1)

- test: crates/iroha_crypto/src/signature/admission/tests.rs:238 — `std::env::var("IROHA_CRYPTO_FIXED_ALGORITHM")`

## IROHA_CRYPTO_FIXED_CASE (test: 1)

- test: crates/iroha_crypto/src/signature/admission/tests.rs:246 — `let mode = std::env::var("IROHA_CRYPTO_FIXED_CASE").unwrap();`

## IROHA_CRYPTO_FIXED_PUBLIC_KEY (test: 1)

- test: crates/iroha_crypto/src/signature/admission/tests.rs:244 — `let bytes = hex::decode(std::env::var("IROHA_CRYPTO_FIXED_PUBLIC_KEY").unwrap()).unwrap();`

## IROHA_CRYPTO_FIXED_SIGNATURE (test: 1)

- test: crates/iroha_crypto/src/signature/admission/tests.rs:245 — `let signature = hex::decode(std::env::var("IROHA_CRYPTO_FIXED_SIGNATURE").unwrap()).unwrap();`

## IROHA_DA_SPOOL_DIR (test: 1)

- test: crates/iroha_core/src/state.rs:28274 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_DEBUG_GENESIS_PATH (test: 3)

- test: crates/iroha_genesis/src/genesis_manifest_tests.rs:875 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/genesis_manifest_tests.rs:923 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/genesis_manifest_tests.rs:948 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`

## IROHA_DEBUG_SIGNED_GENESIS_PATH (test: 1)

- test: crates/iroha_genesis/src/genesis_manifest_tests.rs:897 — `let path = env::var("IROHA_DEBUG_SIGNED_GENESIS_PATH")`

## IROHA_DPN_VALIDATOR_RELEASE_COMMIT (prod: 1)

- prod: crates/iroha_core/src/release_identity.rs:252 — `option_env!("IROHA_DPN_VALIDATOR_RELEASE_COMMIT"),`

## IROHA_DUMP_MANIFEST_JSON (test: 1)

- test: crates/iroha_data_model/src/nexus/manifest.rs:1732 — `if std::env::var_os("IROHA_DUMP_MANIFEST_JSON").is_some() {`

## IROHA_EXECUTION_PARITY_OUTPUT (test: 1)

- test: crates/iroha_core_privacy/src/execution_proofs/proof_tests.rs:176 — `let output = std::env::var_os("IROHA_EXECUTION_PARITY_OUTPUT").map(PathBuf::from);`

## IROHA_EXECUTION_PARITY_REQUEST (test: 1)

- test: crates/iroha_core_privacy/src/execution_proofs/proof_tests.rs:164 — `let request = if let Some(path) = std::env::var_os("IROHA_EXECUTION_PARITY_REQUEST") {`

## IROHA_GENESIS_FILE (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs:18 — `let genesis_path = std::env::var("IROHA_GENESIS_FILE")`

## IROHA_GENESIS_PUBLIC_KEY (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs:20 — `let pub_key_str = std::env::var("IROHA_GENESIS_PUBLIC_KEY").unwrap_or_else(|_| {`

## IROHA_GIT_COMMIT_HASH (prod: 3, test: 1)

- prod: crates/iroha_core/src/release_identity.rs:251 — `option_env!("IROHA_GIT_COMMIT_HASH"),`
- prod: crates/iroha_js_host/src/lib.rs:910 — `option_env!("IROHA_GIT_COMMIT_HASH")`
- test: crates/iroha_js_host/src/lib.rs:13919 — `option_env!("IROHA_GIT_COMMIT_HASH").unwrap_or("unknown")`
- prod: crates/iroha_kagami/src/main.rs:47 — `const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");`

## IROHA_HISTORY_TEST_MODE (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_v1_state/disk_history_store_tests.rs:461 — `let mode = std::env::var("IROHA_HISTORY_TEST_MODE").unwrap();`

## IROHA_HISTORY_TEST_PATH (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_v1_state/disk_history_store_tests.rs:457 — `let Some(path) = std::env::var_os("IROHA_HISTORY_TEST_PATH") else {`

## IROHA_INROU_PORTABLE_INITRD_IMAGE (test: 1, tool: 1)

- test: crates/irohad/src/soracloud_runtime/tests/part_06.rs:6 — `let initrd_image = std::env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`
- tool: xtask/src/soracloud_inrou.rs:87 — `if let Ok(value) = env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`

## IROHA_KAGAMI_LOCALNET_KEEP (test: 1)

- test: integration_tests/tests/sumeragi_kagami_localnet.rs:75 — `if std::env::var_os("IROHA_KAGAMI_LOCALNET_KEEP").is_some() {`

## IROHA_KAGEMUSHA_PROFILE_CELLS (test: 3)

- test: crates/iroha_core_zk/src/kagemusha_v1_recursion/mint_hash_claim_fold.rs:126 — `if std::env::var_os("IROHA_KAGEMUSHA_PROFILE_CELLS").is_some() {`
- test: crates/iroha_core_zk/src/kagemusha_v1_recursion/mint_hash_claim_fold.rs:3490 — `if std::env::var_os("IROHA_KAGEMUSHA_PROFILE_CELLS").is_some() {`
- test: crates/iroha_core_zk/src/kagemusha_v1_recursion/mint_hash_claim_fold.rs:3504 — `if std::env::var_os("IROHA_KAGEMUSHA_PROFILE_CELLS").is_some() {`

## IROHA_MCP_URL (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:451 — `mcp_url: std::env::var("IROHA_MCP_URL").ok().or_else(|| {mcp_url}),`

## IROHA_METRICS_PANIC_ON_DUPLICATE (test: 2)

- test: crates/iroha_telemetry/src/metrics.rs:4538 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- test: crates/iroha_torii/tests/metrics_registry.rs:30 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## IROHA_MOCHI_CANONICAL_FIXTURE_STAGE (test: 1)

- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs:19 — `let Some(raw_stage) = env::var_os("IROHA_MOCHI_CANONICAL_FIXTURE_STAGE") else {`

## IROHA_MOCHI_REPLAY_FIXTURE_STAGE (test: 1)

- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs:21 — `let Some(raw_stage) = env::var_os("IROHA_MOCHI_REPLAY_FIXTURE_STAGE") else {`

## IROHA_PARITY_NODE (test: 1)

- test: integration_tests/tests/sorafs_orchestrator_parity.rs:339 — `let node = std::env::var("IROHA_PARITY_NODE").unwrap_or_else(|_| "node".to_owned());`

## IROHA_PARITY_PYTHON (test: 1)

- test: integration_tests/tests/sorafs_orchestrator_parity.rs:340 — `let python = std::env::var("IROHA_PARITY_PYTHON").unwrap_or_else(|_| "python3".to_owned());`

## IROHA_PORT_LEASE_TEST_MODE (test: 1)

- test: crates/iroha_test_network/src/fslock_ports/port_lease.rs:331 — `if std::env::var("IROHA_PORT_LEASE_TEST_MODE").unwrap() == "blocked" {`

## IROHA_PORT_LEASE_TEST_OWNER (test: 1)

- test: crates/iroha_test_network/src/fslock_ports/port_lease.rs:322 — `let owner = std::env::var("IROHA_PORT_LEASE_TEST_OWNER")`

## IROHA_PORT_LEASE_TEST_PORT (test: 1)

- test: crates/iroha_test_network/src/fslock_ports/port_lease.rs:326 — `let port = std::env::var("IROHA_PORT_LEASE_TEST_PORT")`

## IROHA_PORT_LEASE_TEST_ROOT (test: 1)

- test: crates/iroha_test_network/src/fslock_ports/port_lease.rs:319 — `let Some(root) = std::env::var_os("IROHA_PORT_LEASE_TEST_ROOT") else {`

## IROHA_PRINT_PREPARED_TRANSACTION_SIGNATURE_FIXTURE (test: 1)

- test: crates/iroha_torii/src/routing.rs:54554 — `if std::env::var_os("IROHA_PRINT_PREPARED_TRANSACTION_SIGNATURE_FIXTURE").is_none() {`

## IROHA_PRIVATE_KEY (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:455 — `private_key: std::env::var("IROHA_PRIVATE_KEY").ok(),`

## IROHA_REALISTIC_30TPS_LOAD_KIND (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:344 — `let Some(raw) = std::env::var("IROHA_REALISTIC_30TPS_LOAD_KIND")`

## IROHA_REALISTIC_30TPS_LOG_LEVEL (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2205 — `std::env::var("IROHA_REALISTIC_30TPS_LOG_LEVEL").unwrap_or_else(|_| "WARN".into());`

## IROHA_RELEASE_ARTIFACT_ROOT (test: 1)

- test: crates/iroha_test_network/tests/support/production_beacon_bootstrap.rs:128 — `.or_else(|| std::env::var_os("IROHA_RELEASE_ARTIFACT_ROOT"))`

## IROHA_RUN_IGNORED (test: 18)

- test: crates/iroha_core/tests/check_genesis_sig.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/zk_roots_get_cap.rs:40 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:13 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:31 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:48 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:70 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints_router.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_read_endpoints_router.rs:36 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/beep_test.rs:6 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/kotodama_struct_fields.rs:10 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:45 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:282 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:439 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:508 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/pipeline_block_rejected.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/sorting.rs:43 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_SKIP_BIND_CHECKS (test: 1)

- test: crates/iroha_test_network/src/lib.rs:8823 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (test: 1)

- test: crates/iroha_crypto/tests/sm_cli_matrix.rs:38 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_TEST_BUILD_PROFILE (test: 1)

- test: integration_tests/src/binary_resolver.rs:157 — `std::env::var("IROHA_TEST_BUILD_PROFILE").ok().as_deref(),`

## IROHA_TEST_CLIENT_TTL_MS (test: 5)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2185 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2985 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3216 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3379 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3918 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`

## IROHA_TEST_DUMP_GENESIS (test: 1)

- test: crates/iroha_test_network/src/lib.rs:15861 — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`

## IROHA_TEST_NETWORK_PARALLELISM (test: 1)

- test: integration_tests/tests/address_canonicalisation.rs:44 — `if let Ok(raw) = env::var("IROHA_TEST_NETWORK_PARALLELISM")`

## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (build: 1, test: 1)

- test: crates/iroha_test_network/src/config.rs:668 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- build: crates/iroha_test_samples/build.rs:59 — `if env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_REAL_SORAFS_NODE (test: 1)

- test: crates/iroha_cli/src/soracloud/tests/part_02.rs:2246 — `let helper = std::env::var_os("IROHA_TEST_REAL_SORAFS_NODE")`

## IROHA_TEST_REQUIRE_NETWORK (test: 3)

- test: integration_tests/tests/contracts/ivm_proved.rs:349 — `std::env::var("IROHA_TEST_REQUIRE_NETWORK").as_deref() == Ok("1"),`
- test: integration_tests/tests/kaigi_privacy_network.rs:328 — `std::env::var("IROHA_TEST_REQUIRE_NETWORK").as_deref() == Ok("1"),`
- test: integration_tests/tests/proofs/full_tree_wallet.rs:145 — `std::env::var("IROHA_TEST_REQUIRE_NETWORK").as_deref() == Ok("1"),`

## IROHA_TEST_SERIALIZE_NETWORKS (test: 2)

- test: integration_tests/tests/address_canonicalisation.rs:39 — `if let Ok(raw) = env::var("IROHA_TEST_SERIALIZE_NETWORKS")`
- test: integration_tests/tests/asset.rs:265 — `if std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none() {`

## IROHA_TEST_SKIP_BUILD (test: 2)

- test: integration_tests/src/binary_resolver.rs:44 — `std::env::var("IROHA_TEST_SKIP_BUILD").ok().as_deref(),`
- test: integration_tests/tests/alias_registry_bootstrap_network.rs:901 — `std::env::var("IROHA_TEST_SKIP_BUILD").as_deref() == Ok("1"),`

## IROHA_TEST_USE_DEFAULT_EXECUTOR (test: 2)

- test: crates/iroha_core/src/executor.rs:17599 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- test: crates/iroha_core/src/executor_contract_dispatch_tests.rs:258 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`

## IROHA_THROUGHPUT_ARTIFACT_DIR (test: 3)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2831 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3869 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4403 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`

## IROHA_THROUGHPUT_DELAY_MS (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:298 — `if let Ok(delay) = std::env::var("IROHA_THROUGHPUT_DELAY_MS") {`

## IROHA_TORII_OPENAPI_ACTUAL (test: 1)

- test: crates/iroha_torii/tests/router_feature_matrix.rs:80 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_EXPECTED (test: 2)

- test: crates/iroha_torii/tests/router_feature_matrix.rs:75 — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- test: crates/iroha_torii/tests/router_feature_matrix.rs:89 — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_TORII_URL (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:449 — `torii_url: std::env::var("IROHA_TORII_URL")`

## IROHA_WRITE_DIRECT_CONVICTION_GOLDEN_V1 (test: 1)

- test: crates/iroha_data_model/src/isi/governance.rs:929 — `if std::env::var("IROHA_WRITE_DIRECT_CONVICTION_GOLDEN_V1")`

## IVM_BIN (test: 2)

- test: integration_tests/tests/kotodama_examples.rs:57 — `let ivm_bin = env::var("IVM_BIN")`
- test: integration_tests/tests/kotodama_examples.rs:145 — `let ivm_bin = env::var("IVM_BIN")`

## IVM_COMPILER_DEBUG (test: 1)

- test: crates/kotodama_lang/src/compiler.rs:8694 — `if cfg!(any(test, debug_assertions)) && std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_GENCODE (build: 1)

- build: crates/ivm/build.rs:679 — `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| DEFAULT_CUDA_GENCODE.to_string());`

## IVM_CUDA_NVCC (build: 1)

- build: crates/ivm/build.rs:673 — `let executable = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVCC_EXTRA (build: 1)

- build: crates/ivm/build.rs:680 — `let extra_flags = env::var("IVM_CUDA_NVCC_EXTRA")`

## IVM_CUDA_PTX_MODE (build: 1)

- build: crates/ivm/build.rs:657 — `match env::var("IVM_CUDA_PTX_MODE") {`

## IVM_CUDA_SELFTEST_TRACE (test: 1)

- test: crates/ivm/src/cuda.rs:131 — `if std::env::var_os("IVM_CUDA_SELFTEST_TRACE").is_some() {`

## IVM_CUDA_TRUSTED_KEY_SHA256 (build: 1)

- build: crates/ivm/build.rs:587 — `let trusted_key_sha256 = env::var("IVM_CUDA_TRUSTED_KEY_SHA256").map_err(`

## IVM_DEBUG_AED_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:351 — `let aed_asset_raw = std::env::var("IVM_DEBUG_AED_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:229 — `let asset_raw = std::env::var("IVM_DEBUG_ASSET_DEFINITION")`

## IVM_DEBUG_CBDC_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:357 — `let cbdc_asset_raw = std::env::var("IVM_DEBUG_CBDC_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_DOMAIN (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:231 — `let domain_raw = std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank".to_owned());`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:364 — `std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank.universal".to_owned());`

## IVM_DEBUG_FROM_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:227 — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:349 — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`

## IVM_DEBUG_METAL_SELFTEST (debug: 1)

- debug: crates/ivm/src/vector.rs:596 — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DEBUG_RATIO (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:365 — `let ratio_raw = std::env::var("IVM_DEBUG_RATIO").unwrap_or_else(|_| "76".to_owned());`

## IVM_DEBUG_TO_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:228 — `let to_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:350 — `let dst_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`

## IVM_DISABLE_CUDA (test: 2)

- test: crates/ivm/tests/cuda_disable_on_mismatch.rs:40 — `original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),`
- test: crates/ivm/tests/cuda_env.rs:26 — `original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),`

## IVM_DISABLE_METAL (debug: 1)

- debug: crates/ivm/src/vector.rs:280 — `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELFTEST_FAIL (test: 2)

- test: crates/ivm/tests/cuda_disable_on_mismatch.rs:39 — `original_force_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),`
- test: crates/ivm/tests/cuda_env.rs:27 — `original_force_selftest_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),`

## IVM_FORCE_METAL_SELFTEST_FAIL (debug: 1)

- debug: crates/ivm/src/vector.rs:585 — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IVM_TOOL_BIN (test: 1)

- test: integration_tests/tests/kotodama_examples.rs:99 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## IZANAMI_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs:7299 — `std::env::var("IZANAMI_ALLOW_NET")`

## IZANAMI_TUI_ALLOW_ZERO_SEED (prod: 1)

- prod: crates/izanami/src/tui.rs:174 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:45 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_REQUIRE (test: 3)

- test: crates/norito/accelerators/jsonstage1_cuda/src/lib.rs:357 — `std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some()`
- test: crates/norito/src/core/sequence_plan_helper_tests.rs:57 — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`
- test: crates/norito/src/lib.rs:5487 — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`

## JSONSTAGE1_CUDA_SKIP_BUILD (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:20 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KAGEMUSHA_OPERATION_STORE_TEST_LOST_REPLY (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_v1_state/coordinator_operation_store_tests.rs:1025 — `if std::env::var_os("KAGEMUSHA_OPERATION_STORE_TEST_LOST_REPLY").is_some() {`

## KAGEMUSHA_OPERATION_STORE_TEST_PATH (test: 1)

- test: crates/iroha_core_zk/src/kagemusha_v1_state/coordinator_operation_store_tests.rs:1016 — `let Some(path) = std::env::var_os("KAGEMUSHA_OPERATION_STORE_TEST_PATH") else {`

## KAGEMUSHA_SENDER_FIXTURE_CONTEXTS (test: 1)

- test: crates/connect_norito_bridge/src/kagemusha_sender_release_evidence_tests.rs:847 — `std::env::var("KAGEMUSHA_SENDER_FIXTURE_CONTEXTS").expect("public fixture input path");`

## KAGEMUSHA_SENDER_FIXTURE_OUTPUT (test: 1)

- test: crates/connect_norito_bridge/src/kagemusha_sender_release_evidence_tests.rs:848 — `let output = std::env::var("KAGEMUSHA_SENDER_FIXTURE_OUTPUT").expect("fixture output path");`

## KOTO_BIN (test: 2)

- test: integration_tests/tests/kotodama_examples.rs:48 — `let koto_bin = env::var("KOTO_BIN")`
- test: integration_tests/tests/kotodama_examples.rs:136 — `let koto_bin = env::var("KOTO_BIN")`

## LANG (test: 2)

- test: crates/ivm/tests/i18n.rs:11 — `let old_lang = env::var("LANG").ok();`
- test: crates/ivm/tests/i18n.rs:65 — `let old_lang = env::var("LANG").ok();`

## LC_ALL (test: 2)

- test: crates/ivm/tests/i18n.rs:12 — `let old_lc_all = env::var("LC_ALL").ok();`
- test: crates/ivm/tests/i18n.rs:66 — `let old_lc_all = env::var("LC_ALL").ok();`

## LC_MESSAGES (test: 2)

- test: crates/ivm/tests/i18n.rs:13 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`
- test: crates/ivm/tests/i18n.rs:67 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## LOCALAPPDATA (prod: 2)

- prod: crates/musubi/src/cache.rs:161 — `std::env::var_os("LOCALAPPDATA").map(PathBuf::from),`
- prod: crates/musubi/src/command.rs:3170 — `let root = std::env::var_os("LOCALAPPDATA")`

## MOCHI_CONFIG (prod: 1)

- prod: mochi/mochi-ui-egui/src/config.rs:418 — `if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## MOCHI_DATA_ROOT (prod: 1)

- prod: mochi/mochi-core/src/supervisor.rs:4038 — `std::env::var_os("MOCHI_DATA_ROOT")`

## MOCHI_DETACHED (prod: 1)

- prod: mochi/mochi-ui-egui/src/sandbox_cli.rs:596 — `if env::var_os("MOCHI_DETACHED").is_some() {`

## MOCHI_KAGAMI (test: 1)

- test: mochi/mochi-core/src/supervisor/tests/genesis.rs:637 — `let script = env::var_os("MOCHI_KAGAMI").expect("installed stub path");`

## MOCHI_REAL_KAGAMI (test: 1)

- test: mochi/mochi-integration/tests/supervisor.rs:52 — `let kagami = std::env::var_os("MOCHI_REAL_KAGAMI")`

## NORITO_CHECK_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs:12 — `if env::var_os("NORITO_CHECK_BINDINGS_SYNC").is_none() {`

## NORITO_CPU_INFO (tool: 1)

- tool: xtask/src/stage1_bench.rs:59 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_CUDA_REQUIRE (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:1102 — `if std::env::var_os("NORITO_CRC64_CUDA_REQUIRE").is_none() {`

## NORITO_CRC64_GPU_LIB (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:274 — `let raw = std::env::var_os("NORITO_CRC64_GPU_LIB")?;`

## NORITO_GPU_CRC64_MIN_BYTES (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:60 — `let configured = std::env::var("NORITO_GPU_CRC64_MIN_BYTES")`

## NORITO_PAR_STAGE1_MIN (test: 1)

- test: crates/norito/src/lib.rs:5747 — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## NORITO_SKIP_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs:9 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_GPU_MIN_BYTES (test: 1)

- test: crates/norito/src/lib.rs:5776 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## NORITO_TRACE (test: 3)

- test: crates/norito/src/lib.rs:136 — `std::env::var_os("NORITO_TRACE").is_some()`
- test: crates/norito/src/lib.rs:141 — `*ENABLED.get_or_init(|| std::env::var_os("NORITO_TRACE").is_some())`
- test: crates/norito/src/lib.rs:156 — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## NOTIFY_SOCKET (test: 1)

- test: crates/irohad/src/runtime_provider_broker/launcher.rs:693 — `let notify_socket = std::env::var_os("NOTIFY_SOCKET")`

## NVCC (build: 1)

- build: crates/ivm/build.rs:674 — `.or_else(|_| env::var("NVCC"))`

## OUT_DIR (build: 6, prod: 15, test: 8)

- build: crates/fastpq_prover/build.rs:150 — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
- build: crates/iroha_test_samples/build.rs:46 — `let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR"));`
- test: crates/iroha_test_samples/src/lib.rs:231 — `PathBuf::from(env!("OUT_DIR"))`
- test: crates/iroha_test_samples/src/lib.rs:239 — `PathBuf::from(env!("OUT_DIR")).join("ivm/build_config.toml")`
- test: crates/iroha_test_samples/src/lib.rs:322 — `Path::new(env!("OUT_DIR")).join("ivm/samples")`
- test: crates/iroha_test_samples/src/lib.rs:330 — `Path::new(env!("OUT_DIR")).join("ivm/build_config.toml")`
- build: crates/ivm/build.rs:393 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs:468 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs:532 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- prod: crates/ivm/src/cuda/aes_api.rs:12 — `concat!(include_str!(concat!(env!("OUT_DIR"), "/aes.ptx")), "\0").as_bytes(),`
- prod: crates/ivm/src/cuda/bitonic_api.rs:11 — `include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx")),`
- prod: crates/ivm/src/cuda/bn254_api.rs:12 — `concat!(include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx")), "\0").as_bytes(),`
- prod: crates/ivm/src/cuda/hash_api.rs:12 — `concat!(include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx")), "\0").as_bytes(),`
- prod: crates/ivm/src/cuda/hash_api.rs:20 — `concat!(include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx")), "\0").as_bytes(),`
- prod: crates/ivm/src/cuda/merkle_api.rs:13 — `include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx")),`
- prod: crates/ivm/src/cuda/merkle_api.rs:25 — `include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx")),`
- prod: crates/ivm/src/cuda/poseidon_api.rs:12 — `include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx")),`
- prod: crates/ivm/src/cuda/signature_api.rs:14 — `include_str!(concat!(env!("OUT_DIR"), "/signature.ptx")),`
- prod: crates/ivm/src/cuda/vector_api.rs:16 — `concat!(include_str!(concat!(env!("OUT_DIR"), "/vector.ptx")), "\0").as_bytes(),`
- prod: crates/ivm/src/iso20022.rs:237 — `include!(concat!(env!("OUT_DIR"), "/iso20022_schema_v1.rs"));`
- prod: crates/ivm/src/ivm.rs:144 — `include!(concat!(env!("OUT_DIR"), "/syscall_signatures.rs"));`
- test: crates/ivm/src/ptx_tests.rs:6 — `let out_dir = env!("OUT_DIR");`
- test: crates/ivm/tests/ptx_kernels.rs:6 — `let out_dir = env!("OUT_DIR");`
- build: crates/kotodama_lang/build.rs:597 — `let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));`
- prod: crates/kotodama_lang/src/diagnostic.rs:76 — `env!("OUT_DIR"),`
- prod: crates/kotodama_lang/src/i18n/mod.rs:10 — `include_bytes!(concat!(env!("OUT_DIR"), "/kotodama_i18n_v1_offsets.bin"));`
- prod: crates/kotodama_lang/src/lexer.rs:118 — `include!(concat!(env!("OUT_DIR"), "/kotodama_v1_lexical.rs"));`
- test: crates/kotodama_lang/tests/compile_fail_goldens.rs:17 — `include!(concat!(env!("OUT_DIR"), "/kotodama_compile_fail_cases.rs"));`
- test: crates/kotodama_lang/tests/secret_security_diagnostics.rs:16 — `include!(concat!(env!("OUT_DIR"), "/kotodama_secret_reject_cases.rs"));`

## PATH (prod: 2, test: 2)

- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:1344 — `if env::var_os("PATH").as_deref() != Some(OsStr::new("/usr/bin:/bin")) {`
- test: crates/irohad/src/soracloud_runtime.rs:17284 — `if let Some(path) = std::env::var_os("PATH") {`
- test: integration_tests/tests/kotodama_examples.rs:18 — `let path = env::var_os("PATH")?;`
- prod: mochi/mochi-core/src/supervisor.rs:887 — `let path_var = env::var_os("PATH")?;`

## PRINT_KAGEMUSHA_CORE_COORDINATOR_ARCHIVES_V1 (test: 1)

- test: crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/archives/tests.rs:234 — `if let Some(export) = std::env::var_os("PRINT_KAGEMUSHA_CORE_COORDINATOR_ARCHIVES_V1") {`

## PRINT_KAGEMUSHA_FIXTURE_V1 (test: 1)

- test: crates/connect_norito_bridge/src/kagemusha_fixture_tests.rs:496 — `if let Some(destination) = std::env::var_os("PRINT_KAGEMUSHA_FIXTURE_V1") {`

## PRINT_SORACLES_FIXTURES (test: 1)

- test: crates/iroha_data_model/src/oracle/mod.rs:3793 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORII_SPEC (test: 1)

- test: crates/iroha_torii/src/openapi/tests/catalog_and_contracts.rs:954 — `if std::env::var("PRINT_TORII_SPEC").is_ok() {`

## PROFILE (build: 2, test: 3)

- test: crates/iroha_test_network/src/lib.rs:1967 — `if let Ok(profile) = std::env::var("PROFILE") {`
- build: crates/iroha_test_samples/build.rs:49 — `let profile = if env::var("PROFILE").ok().as_deref() == Some("release") {`
- build: crates/ivm/build.rs:535 — `reject_generated_release_ptx(mode, &env::var("PROFILE").unwrap_or_default())?;`
- test: integration_tests/src/binary_resolver.rs:217 — `if let Ok(profile) = std::env::var("PROFILE")`
- test: integration_tests/src/kagami.rs:66 — `let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());`

## PYTHON3 (test: 1)

- test: crates/sorafs_car/tests/taikai_viewer_cli.rs:25 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONDONTWRITEBYTECODE (prod: 1)

- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:1356 — `if let Some(value) = env::var_os("PYTHONDONTWRITEBYTECODE")`

## PYTHONPATH (test: 1)

- test: crates/iroha_cli/bins/tests/cli_smoke.rs:4957 — `match env::var("PYTHONPATH") {`

## REPO_PROOF_DIGEST_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2667 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2655 — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUSTC (test: 1)

- test: tools/norito_codegen_exporter/src/source_inventory/context_graph/tests.rs:17 — `std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))`

## RUSTC_WORKSPACE_WRAPPER (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs:1139 — `&& std::env::var_os("RUSTC_WORKSPACE_WRAPPER").is_none(),`

## RUSTC_WRAPPER (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs:1138 — `&& std::env::var_os("RUSTC_WRAPPER").is_none()`

## RUSTFLAGS (test: 1)

- test: integration_tests/tests/nexus/atomic_private_settlement_real_process_harness.rs:1137 — `&& std::env::var_os("RUSTFLAGS").is_none()`

## RUST_LOG (prod: 2, test: 3)

- test: crates/iroha_test_network/src/lib.rs:12320 — `let original = env::var("RUST_LOG").ok();`
- test: crates/iroha_test_network/src/lib.rs:12333 — `let original = env::var("RUST_LOG").ok();`
- prod: crates/izanami/src/chaos.rs:2338 — `if let Ok(filter) = std::env::var("RUST_LOG") {`
- prod: crates/izanami/src/config.rs:565 — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`
- test: integration_tests/tests/sumeragi_kagami_localnet.rs:133 — `if std::env::var_os("RUST_LOG").is_none() {`

## RUST_MIN_STACK (test: 1)

- test: crates/iroha_core/src/privacy_release_evidence/tests.rs:577 — `std::env::var("RUST_MIN_STACK").as_deref(),`

## SM_PERF_CPU_LABEL (prod: 2)

- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:657 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:693 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## SORAFS_NODE_SKIP_INGEST_TESTS (test: 1)

- test: crates/sorafs_node/tests/cli.rs:26 — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORAFS_QUALIFICATION_DIST (test: 1)

- test: crates/sorafs_node/tests/publication_roundtrip.rs:247 — `std::env::var_os("SORAFS_QUALIFICATION_DIST").expect("set SORAFS_QUALIFICATION_DIST");`

## SORAFS_QUALIFICATION_PACKAGE (test: 1)

- test: crates/sorafs_node/tests/publication_roundtrip.rs:249 — `std::env::var_os("SORAFS_QUALIFICATION_PACKAGE").expect("set SORAFS_QUALIFICATION_PACKAGE");`

## SORAFS_TORII_SKIP_INGEST_TESTS (test: 1)

- test: crates/iroha_torii/tests/sorafs_discovery.rs:120 — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORA_CARS_FULL_PROOF_FIXTURE (test: 1)

- test: crates/iroha_data_model/tests/full_race_payload_codec.rs:17 — `let path = std::env::var_os("SORA_CARS_FULL_PROOF_FIXTURE")`

## SORA_CARS_GAME_FIXTURE_OUTPUT (test: 1)

- test: crates/iroha_data_model/tests/game_v1_codec.rs:498 — `std::env::var_os("SORA_CARS_GAME_FIXTURE_OUTPUT").expect("native output path");`

## SORA_CARS_GAME_FIXTURE_SEED (test: 1)

- test: crates/iroha_data_model/tests/game_v1_codec.rs:496 — `let seed_path = std::env::var_os("SORA_CARS_GAME_FIXTURE_SEED").expect("semantic seed path");`

## SORA_CARS_NATIVE_PROOF_OUTPUT (test: 1)

- test: crates/iroha_core_privacy/src/execution_proofs/proof_tests.rs:89 — `let output = std::env::var_os("SORA_CARS_NATIVE_PROOF_OUTPUT").map(std::path::PathBuf::from);`

## SORA_CARS_RESOURCE_FIXTURE_OUTPUT (test: 1)

- test: crates/iroha_data_model/tests/game_resources_v1_codec.rs:24 — `let output = std::env::var_os("SORA_CARS_RESOURCE_FIXTURE_OUTPUT").expect("fresh output path");`

## SUMERAGI_BASELINE_ARTIFACT_DIR (prod: 1)

- prod: crates/build-support/src/bin/sumeragi_baseline_report.rs:40 — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`

## SUMERAGI_DA_ARTIFACT_DIR (prod: 1)

- prod: crates/build-support/src/bin/sumeragi_da_report.rs:41 — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`

## SUMERAGI_FUZZ_SEEDS (test: 1)

- test: crates/iroha_sumeragi/src/machine/tests/fuzz.rs:312 — `let seeds: u64 = std::env::var("SUMERAGI_FUZZ_SEEDS")`

## SUMERAGI_SIM_GAPS (prod: 1)

- prod: crates/iroha_sumeragi/src/sim/oracle.rs:1574 — `if std::env::var("SUMERAGI_SIM_GAPS").is_ok() && !sorted.is_empty() {`

## SUMERAGI_SIM_TRACE (prod: 1)

- prod: crates/iroha_sumeragi/src/sim/world.rs:591 — `verbose: std::env::var("SUMERAGI_SIM_TRACE").is_ok(),`

## SystemRoot (prod: 1, test: 1)

- prod: crates/fastpq_prover/src/backend.rs:821 — `env::var_os("SystemRoot").map(PathBuf::from)`
- test: crates/irohad/src/soracloud_runtime.rs:17281 — `if let Some(system_root) = std::env::var_os("SystemRoot") {`

## TAIRA_TESTNET_BEACON_FIXTURE_DIR (test: 1)

- test: crates/iroha_test_network/tests/support/production_beacon_bootstrap.rs:127 — `let root = std::env::var_os("TAIRA_TESTNET_BEACON_FIXTURE_DIR")`

## TARGET (prod: 1, test: 2, tool: 2)

- prod: crates/build-support/src/lib.rs:27 — `let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());`
- test: crates/ivm/build.rs:78 — `if let Ok(target) = env::var("TARGET") {`
- test: crates/ivm/build.rs:89 — `if env::var("TARGET").is_ok_and(|target| target.contains("apple-darwin"))`
- tool: xtask/src/poseidon_bench.rs:78 — `target: std::env::var("TARGET")`
- tool: xtask/src/stage1_bench.rs:53 — `target: std::env::var("TARGET")`

## TEST_LOG_FILTER (prod: 1)

- prod: crates/iroha_logger/src/lib.rs:114 — `filter: std::env::var("TEST_LOG_FILTER")`

## TEST_LOG_LEVEL (prod: 1)

- prod: crates/iroha_logger/src/lib.rs:110 — `level: std::env::var("TEST_LOG_LEVEL")`

## TEST_NETWORK_BIN_SORAFS_CLI (test: 1)

- test: integration_tests/tests/sorafs_publication.rs:306 — `let cli_binary = std::env::var_os("TEST_NETWORK_BIN_SORAFS_CLI")`

## TEST_NETWORK_CARGO (test: 1)

- test: crates/iroha_test_network/src/lib.rs:2604 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TEST_NETWORK_IROHAD_FEATURES (test: 5)

- test: integration_tests/tests/nexus/cross_dataspace_zk_stark_localnet.rs:222 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/privacy_exact12_activation_network.rs:133 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/privacy_exact12_zk_x509_network.rs:100 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_ace_localnet.rs:45 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_stark_network.rs:21 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`

## TMPDIR (prod: 2, test: 1)

- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:644 — `let temporary = env::var_os("TMPDIR")`
- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:1363 — `let temporary = env::var_os("TMPDIR")`
- test: crates/irohad/src/runtime_provider_broker/server_source_tests.rs:2648 — `assert_eq!(std::env::var_os("TMPDIR"), Some(cwd.into_os_string()));`

## TORII_MOCK_HARNESS_METRICS_PATH (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:91 — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:94 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_RETRY_TOTAL (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:270 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:97 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:89 — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## UPDATE_FIXTURES (test: 3)

- test: crates/iroha_core/tests/pin_registry.rs:121 — `if env::var_os("UPDATE_FIXTURES").is_some() {`
- test: crates/iroha_core/tests/snapshots.rs:43 — `let update = env::var("UPDATE_FIXTURES")`
- test: crates/iroha_torii/src/routing/pipeline_preflight_fixture_tests.rs:124 — `if std::env::var_os("UPDATE_FIXTURES").is_some() {`

## USERPROFILE (prod: 1)

- prod: crates/iroha_cli/src/client_config.rs:30 — `env::var_os("USERPROFILE").map(PathBuf::from)`

## VERGEN_CARGO_FEATURES (prod: 1)

- prod: crates/iroha_core/src/release_identity.rs:253 — `option_env!("VERGEN_CARGO_FEATURES"),`

## VERGEN_CARGO_TARGET_TRIPLE (prod: 1)

- prod: crates/iroha_core/src/release_identity.rs:254 — `option_env!("VERGEN_CARGO_TARGET_TRIPLE"),`

## VERGEN_GIT_SHA (prod: 1)

- prod: crates/iroha_core/src/release_identity.rs:250 — `option_env!("VERGEN_GIT_SHA"),`

## XDG_CACHE_HOME (prod: 1)

- prod: crates/musubi/src/cache.rs:167 — `if let Some(root) = std::env::var_os("XDG_CACHE_HOME") {`

## XDG_DATA_HOME (prod: 1)

- prod: crates/iroha_wallet/src/custody.rs:145 — `if let Some(base) = std::env::var_os("XDG_DATA_HOME") {`

## XDG_STATE_HOME (prod: 1)

- prod: crates/musubi/src/command.rs:3181 — `let root = std::env::var_os("XDG_STATE_HOME")`

## XTASK_TEST_KAGAMI_BIN (test: 2)

- test: xtask/src/kagami_profiles.rs:2671 — `if std::env::var("XTASK_TEST_KAGAMI_BIN").is_err() {`
- test: xtask/src/kagami_profiles.rs:2674 — `let kagami_path = PathBuf::from(std::env::var("XTASK_TEST_KAGAMI_BIN").unwrap());`
