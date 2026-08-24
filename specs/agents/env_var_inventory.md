# Environment toggle inventory

_Last refreshed via `python3 scripts/inventory_env_toggles.py --json specs/agents/env_var_inventory.json --md specs/agents/env_var_inventory.md`_

Total references: **790** · Unique variables: **194**

## ANDROID_SDK_ROOT (prod: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:19685 — `if let Some(android_sdk_root) = std::env::var_os("ANDROID_SDK_ROOT") {`

## CARGO (prod: 3, test: 3)

- test: crates/iroha_test_network/src/lib.rs:2616 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- test: crates/norito_derive/tests/ui.rs:31 — `let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());`
- test: integration_tests/src/kagami.rs:141 — `let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());`
- prod: mochi/mochi-core/src/supervisor.rs:1268 — `let cargo = env::var_os("CARGO")`
- prod: mochi/mochi-core/src/supervisor.rs:1317 — `let cargo = env::var_os("CARGO")`
- prod: mochi/mochi-core/src/supervisor.rs:1363 — `let cargo = env::var_os("CARGO")`

## CARGO_BIN_EXE_attachment_sanitizer (test: 9)

- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:89 — `let mut cmd = Command::new(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:130 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:153 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:291 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:314 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:335 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:350 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:365 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:381 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`

## CARGO_BIN_EXE_iroha (test: 2)

- test: crates/iroha_cli/tests/cli_smoke.rs:48 — `env!("CARGO_BIN_EXE_iroha")`
- test: crates/iroha_cli/tests/taikai_policy.rs:19 — `env!("CARGO_BIN_EXE_iroha")`

## CARGO_BIN_EXE_iroha_monitor (test: 4)

- test: crates/iroha_monitor/tests/attach_render.rs:10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/http_limits.rs:10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/invalid_credentials.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/smoke.rs:21 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## CARGO_BIN_EXE_kagami (test: 3)

- test: crates/iroha_kagami/tests/common/mod.rs:17 — `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- test: crates/iroha_kagami/tests/pop_embed.rs:30 — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- test: integration_tests/src/kagami.rs:61 — `if let Ok(path) = env::var("CARGO_BIN_EXE_kagami") {`

## CARGO_BIN_EXE_kagami_mock (test: 1)

- test: mochi/mochi-integration/tests/supervisor.rs:32 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## CARGO_BIN_EXE_koto (test: 4)

- test: crates/ivm/tests/cli_smoke.rs:5 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/ivm/tests/cli_smoke.rs:54 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/ivm/tests/cli_smoke.rs:87 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/ivm/tests/cli_smoke.rs:123 — `let bin = env!("CARGO_BIN_EXE_koto");`

## CARGO_BIN_EXE_sorafs_chunk_dump (test: 1)

- test: crates/sorafs_chunker/tests/one_gib.rs:93 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## CARGO_BIN_NAME (prod: 1)

- prod: crates/iroha_cli/src/main_shared.rs:397 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`

## CARGO_BUILD_TARGET (tool: 2)

- tool: xtask/src/poseidon_bench.rs:79 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- tool: xtask/src/stage1_bench.rs:56 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_FEATURE (prod: 1)

- prod: crates/build-support/src/lib.rs:30 — `let parsed_features = env::var("CARGO_CFG_FEATURE")`

## CARGO_CFG_TARGET_ARCH (prod: 2, tool: 2)

- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:653 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:683 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- tool: xtask/src/poseidon_bench.rs:80 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- tool: xtask/src/stage1_bench.rs:57 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## CARGO_CFG_TARGET_OS (build: 4, prod: 2, tool: 2)

- build: crates/fastpq_prover/build.rs:26 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/gpuzstd_cuda/build.rs:28 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:654 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:684 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- build: crates/ivm/build.rs:562 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:31 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- tool: xtask/src/poseidon_bench.rs:82 — `os: std::env::var("CARGO_CFG_TARGET_OS")`
- tool: xtask/src/stage1_bench.rs:59 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## CARGO_FEATURE_CUDA (build: 2)

- build: crates/fastpq_prover/build.rs:24 — `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
- build: crates/ivm/build.rs:36 — `if env::var_os("CARGO_FEATURE_CUDA").is_some()`

## CARGO_FEATURE_CUDA_KERNEL (build: 2)

- build: crates/gpuzstd_cuda/build.rs:14 — `if env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_none() {`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:15 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (build: 1)

- build: crates/fastpq_prover/build.rs:25 — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## CARGO_FEATURE_FFI_EXPORT (prod: 1)

- prod: crates/build-support/src/lib.rs:260 — `let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();`

## CARGO_FEATURE_FFI_IMPORT (prod: 1)

- prod: crates/build-support/src/lib.rs:259 — `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`

## CARGO_MANIFEST_DIR (bench: 2, build: 5, example: 1, prod: 44, test: 294, tool: 6)

- prod: crates/build-support/src/lib.rs:100 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);`
- prod: crates/connect_norito_bridge/src/bin/swift_parity_regen.rs:297 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/kagemusha_bridge_tests/tests/recipient_fixture_owner.rs:20 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/kagemusha_bridge_tests/tests/recipient_fixture_owner.rs:27 — `let checked = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/lib.rs:14518 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/sorafs_tests.rs:149 — `fs::read(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))`
- prod: crates/fastpq_prover/src/poseidon_manifest.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/fastpq_prover/tests/packing.rs:13 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:22 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:40 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:59 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:64 — `let field_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/field.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:69 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- test: crates/fastpq_prover/tests/proof_fixture.rs:19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/trace_commitment.rs:22 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- test: crates/fastpq_prover/tests/transcript_replay.rs:90 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/client.rs:24281 — `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/sm.rs:190 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/tests/sm_signing.rs:30 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/commands/sorafs.rs:12894 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/compute.rs:747 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_cli/src/main_shared.rs:1446 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/iroha_cli/src/soracloud.rs:15000 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/tests/cli_smoke.rs:156 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/tests/cli_smoke.rs:5127 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_config/src/parameters/user.rs:5005 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/src/parameters/user.rs:31341 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/autoscale_config.rs:7 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/da_ingest_compute_limit.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/fastpq_queue_overrides.rs:13 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:42 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:560 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:698 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:1534 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/kura_retention_hard_cut.rs:14 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs:58 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sccp_route_manifest_aliases.rs:23 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_gateway_runtime_providers.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_governance_dag_runtime_signer.rs:8 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_native_transaction_signers.rs:8 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_por_replay_archive.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_provider_ingest_finalized_archive.rs:8 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_reputation_finalized_archive.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_storage_pin_aliases.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_stream_token_runtime_signer.rs:8 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/soranet_privacy_ingest_hard_cut.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sumeragi_v2_merge_runtime_config.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/transaction_ingress_limits.rs:6 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/trusted_peers_pop_validation.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- bench: crates/iroha_core/benches/blocks/common.rs:312 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- bench: crates/iroha_core/benches/validation.rs:107 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- build: crates/iroha_core/build.rs:616 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- example: crates/iroha_core/examples/generate_parity_fixtures.rs:23 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:2885 — `fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))?;`
- test: crates/iroha_core/src/block.rs:23646 — `let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/executor.rs:17912 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- test: crates/iroha_core/src/executor_contract_dispatch_tests.rs:260 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- test: crates/iroha_core/src/smartcontracts/isi/asset/core_numeric_mutation_tests.rs:4 — `let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2464 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2468 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs:6366 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs:6374 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs:6382 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud_tests.rs:15474 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_core/src/state.rs:42609 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- test: crates/iroha_core/src/streaming.rs:3674 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/src/tx/sandbox_state_tests.rs:64 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/default_domain_independence.rs:30 — `let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/pin_registry.rs:119 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);`
- test: crates/iroha_core/tests/pin_registry.rs:1444 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/snapshots.rs:32 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/sumeragi_doc_sync.rs:68 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/confidential_keyset_vectors.rs:48 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm2_fixture_vectors.rs:49 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm_cli_matrix.rs:15 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:31 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:35 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:39 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/cbsi_offline_vectors.rs:63 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/privacy_exact12_fixtures.rs:103 — `let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_data_model/src/bin/qr_stream_fixtures.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/qr_stream_fixtures.rs:14 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs:41 — `const FIXTURE_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/sumeragi_v2");`
- test: crates/iroha_data_model/src/identifier.rs:770 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/isi/escrow.rs:743 — `let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_data_model/src/lib.rs:216 — `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- prod: crates/iroha_data_model/src/lib.rs:219 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1279 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1630 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1684 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/qr_stream.rs:850 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_data_model/src/soranet/vpn.rs:2893 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- prod: crates/iroha_data_model/src/testing/axt.rs:14 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs:18 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs:22 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/cancel_asset_lock.rs:38 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed_norito_rpc_fixture_tests.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed_norito_rpc_fixture_tests.rs:20 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/account_address_vectors.rs:119 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/address_curve_registry.rs:27 — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:42 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/consensus_roundtrip.rs:1442 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
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
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:52 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:56 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:60 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:64 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:68 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:72 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:76 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:84 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:88 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:92 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:96 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:100 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:104 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:108 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:112 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:2258 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model_derive/src/model.rs:567 — `let data_model_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_data_model");`
- test: crates/iroha_genesis/src/genesis_tail_tests.rs:79 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_genesis/src/genesis_tail_tests.rs:92 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_genesis/src/lib.rs:2523 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);`
- test: crates/iroha_genesis/src/lib.rs:4638 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_genesis/src/lib.rs:4662 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:4715 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:4804 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:4873 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_i18n/src/lib.rs:457 — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- test: crates/iroha_js_host/src/lib.rs:14747 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_js_host/src/lib.rs:17284 — `let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_kagami/samples/codec/generate.rs:10 — `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- test: crates/iroha_kagami/src/codec.rs:534 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/genesis/generate.rs:890 — `let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/prepared.rs:550 — `let defaults = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/genesis/sign.rs:1712 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1727 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1739 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1788 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1812 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1845 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1863 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:2002 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:2342 — `let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:3569 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/genesis/sign.rs:3577 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:3594 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/iroha_kagami/src/localnet.rs:608 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_kagami/src/localnet.rs:4506 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_kagami/src/localnet.rs:4510 — `|| PathBuf::from(env!("CARGO_MANIFEST_DIR")),`
- prod: crates/iroha_kagami/src/wizard.rs:730 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/wizard.rs:2080 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/tests/codec.rs:11 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- test: crates/iroha_swarm/tests/default_compose_soranet.rs:13 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults")`
- test: crates/iroha_test_network/src/fslock_ports.rs:25 — `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- test: crates/iroha_test_network/src/fslock_ports.rs:27 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_test_network/src/lib.rs:759 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_test_samples/src/lib.rs:233 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_test_samples/src/lib.rs:267 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/da/tests/receipt_outcome_tests.rs:289 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- test: crates/iroha_torii/src/identifier_resolution.rs:658 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/openapi/tests/sorafs_contracts.rs:68 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/openapi/tests/vpn_da.rs:6 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/soracloud.rs:11787 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/admission.rs:565 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs:26935 — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs:38113 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/tests/lib_routed_reads/routed_read_source_bounds.rs:445 — `let mut pending = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];`
- test: crates/iroha_torii/src/zk_attachments.rs:2229 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/tests/accounts_portfolio.rs:90 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/tests/sorafs_discovery.rs:1430 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_zkp_halo2/src/vega/canonical_mc_exact.rs:246 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc.rs:531 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc.rs:535 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/prover_key.rs:116 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verifier_key.rs:761 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verifier_key.rs:765 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verify.rs:927 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verify.rs:931 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs:5 — `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega/engine.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs:6 — `const FACADE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs:8 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:7 — `const CRATE_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:8 — `const VEGA_FACADE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:14 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:18 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:22 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/irohad/src/main.rs:12074 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs:12104 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs:14743 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main/shared_sorafs_provider_cache_tests.rs:70 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/runtime_provider_registry.rs:5154 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime.rs:23292 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime.rs:23298 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime.rs:31991 — `.tempdir_in(env!("CARGO_MANIFEST_DIR"))?;`
- build: crates/ivm/build.rs:326 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- build: crates/ivm/build.rs:456 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- prod: crates/ivm/src/bin/gen_abi_hash_doc.rs:16 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: crates/ivm/src/bin/gen_header_doc.rs:114 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_pointer_types_doc.rs:89 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_syscalls_doc.rs:763 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/ivm_fixture_export.rs:83 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/ivm_prebuild.rs:15 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/ivm/src/predecoder_fixtures.rs:211 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- test: crates/ivm/tests/axt_descriptor_builder.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm/tests/cli_smoke.rs:6 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/cli_smoke.rs:55 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/cli_smoke.rs:88 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/cli_smoke.rs:124 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/tests/docs_consistency.rs:3 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/ivm_abi_doc_sync.rs:3 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/ivm_header_doc_sync.rs:42 — `let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama.rs:1837 — `let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kotodama_lang/src/samples");`
- test: crates/ivm/tests/kotodama_argument_record.rs:48 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama_documentation_examples.rs:21 — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/tests/numeric_v1_sdk_fixture.rs:13 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/numeric_v1_sdk_fixture.rs:20 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/pointer_types_doc_generated.rs:6 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- test: crates/ivm/tests/pointer_types_doc_generated_ivm_md.rs:5 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/repository_ivm_artifacts.rs:20 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/syscalls_doc_generated.rs:6 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_doc_sync.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_gas_names.rs:10 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/kotodama_lang/src/diagnostic.rs:608 — `let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/kotodama_lang/src/doc_consistency.rs:15 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/kotodama_lang/src/doc_consistency.rs:208 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/kotodama_lang/tests/documentation_fences.rs:10 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
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
- test: crates/sorafs_car/src/bin/da_reconstruct.rs:625 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs:908 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/provider_admission_fixtures.rs:1011 — `let committed_dir = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_car/src/bin/soranet_trustless_verifier.rs:144 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/reference.rs:292 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/capacity_simulation_toolkit.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/da_reconstruct_cli.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:55 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:899 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:1019 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/taikai_viewer_cli.rs:20 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/trustless_verifier.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_chunker/src/bin/export_vectors.rs:430 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_chunker/tests/backpressure.rs:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_chunker/tests/vectors.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/bin/sorafs-validate.rs:2990 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference.rs:6604 — `let absolute = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference_ffi.rs:1778 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/orderbook_fixtures.rs:15 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/pdp_fixtures.rs:13 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/por_fixtures.rs:14 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/replication_order_fixtures.rs:5 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/sorafs_validate_cli.rs:23 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_node/tests/cli.rs:233 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/src/lib.rs:6843 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs:6971 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs:8640 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/tests/orchestrator_parity.rs:160 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs:2691 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs:3958 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli/pdp.rs:20 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/soranet_pq/tests/kat_vectors.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- build: integration_tests/build.rs:19 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));`
- test: integration_tests/src/bin/refresh_nexus_streaming_fixtures.rs:405 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: integration_tests/src/binary_resolver.rs:151 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")`
- test: integration_tests/src/kagami.rs:170 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/src/sorafs_gateway_capability_refusal.rs:141 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- test: integration_tests/src/sorafs_gateway_conformance.rs:1273 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/asset.rs:258 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/genesis_json.rs:16 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/genesis_json.rs:33 — `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
- test: integration_tests/tests/kotodama_examples.rs:66 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:108 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:154 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/cbdc_rollout_bundle.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/cbdc_whitelist.rs:24 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/global_commit.rs:15 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/lane_registry.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/norito_burn_fixture.rs:29 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/privacy_exact12_zk_x509_network.rs:149 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(RESOURCE_CERTIFICATE_RELATIVE_PATH);`
- test: integration_tests/tests/repo.rs:32 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/streaming/mod.rs:404 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/compose.rs:1489 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1493 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1497 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1501 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1505 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/supervisor.rs:116 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-core/src/supervisor.rs:839 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- prod: mochi/mochi-core/src/supervisor.rs:1260 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs:9 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs:16 — `let checked = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs:10 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs:17 — `let checked = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: mochi/mochi-integration/tests/supervisor.rs:146 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- prod: mochi/mochi-ui-egui/src/gui.rs:121 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:125 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:129 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:133 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:137 — `env!("CARGO_MANIFEST_DIR"),`
- test: tools/kotlin-fixture-gen/src/bin/swift_kagemusha_peer_payment_v4.rs:692 — `let request_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(`
- tool: tools/norito_codegen_exporter/src/norito_rpc.rs:92 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: tools/soranet-handshake-harness/tests/fixtures_verify.rs:16 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/interop_parity.rs:65 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/perf_gate.rs:156 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- tool: xtask/src/bin/control_plane_mock.rs:334 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/main.rs:14027 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/nexus.rs:154 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
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
- test: xtask/tests/soranet_gateway_m2.rs:23 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:8 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:70 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:119 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:186 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:285 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:332 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:472 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_bundle_check.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_entropy_bench.rs:6 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_NAME (prod: 3, test: 1)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:665 — `crate_name: env!("CARGO_PKG_NAME"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:680 — `crate_name: env!("CARGO_PKG_NAME").to_owned(),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:744 — `|| identity.crate_name != env!("CARGO_PKG_NAME")`
- test: crates/norito_derive/tests/ui.rs:19 — `let crate_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "norito_derive".to_owned());`

## CARGO_PKG_VERSION (prod: 20, test: 3, tool: 2)

- prod: crates/iroha/src/client.rs:5516 — `map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_cli/src/commands/sorafs.rs:4922 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_cli/src/main_shared.rs:397 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- prod: crates/iroha_cli/src/main_shared.rs:1098 — `let client_version = env!("CARGO_PKG_VERSION");`
- test: crates/iroha_cli/src/main_shared_tests.rs:1272 — `&[("version", env!("CARGO_PKG_VERSION"))],`
- test: crates/iroha_cli/tests/cli_smoke.rs:536 — `let expected_version = env!("CARGO_PKG_VERSION");`
- prod: crates/iroha_core/src/bin/pk2_bridge_finality_verify.rs:1229 — `let mut preimage = env!("CARGO_PKG_VERSION").as_bytes().to_vec();`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1336 — `env!("CARGO_PKG_VERSION"),`
- prod: crates/iroha_core/src/sumeragi/v2_runner.rs:2211 — `let mut build_preimage = env!("CARGO_PKG_VERSION").as_bytes().to_vec();`
- prod: crates/iroha_js_host/src/lib.rs:4621 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_kagami/src/genesis/generate.rs:449 — `env!("CARGO_PKG_VERSION")`
- prod: crates/iroha_kagami/src/verify.rs:75 — `writeln!(writer, "kagami_version: {}", env!("CARGO_PKG_VERSION"))?;`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:666 — `crate_version: env!("CARGO_PKG_VERSION"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:681 — `crate_version: env!("CARGO_PKG_VERSION").to_owned(),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:745 — `|| identity.crate_version != env!("CARGO_PKG_VERSION")`
- test: crates/iroha_telemetry/src/metrics.rs:5016 — `version: env!("CARGO_PKG_VERSION").to_owned(),`
- prod: crates/iroha_telemetry/src/ws.rs:246 — `env!("CARGO_PKG_VERSION")`
- prod: crates/irohad/src/main.rs:812 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/irohad/src/main.rs:14339 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/kotodama_lang/src/compiler.rs:114 — `const COMPILER_FINGERPRINT: &str = concat!("kotodama_lang/", env!("CARGO_PKG_VERSION"));`
- prod: crates/musubi/src/command.rs:108 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/sorafs_car/src/bin/sorafs_fetch.rs:1095 — `Value::from(env!("CARGO_PKG_VERSION")),`
- prod: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:145 — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/sora-vpn-helper/src/main.rs:76 — `const VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/telemetry-schema-diff/src/main.rs:221 — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## CARGO_TARGET_DIR (prod: 5, test: 6, tool: 2)

- prod: crates/iroha_kagami/src/localnet.rs:4530 — `let target_dir = resolve_target_dir(&repo_root, env::var("CARGO_TARGET_DIR").ok().as_deref());`
- test: crates/iroha_test_network/src/lib.rs:1070 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: crates/iroha_test_network/src/lib.rs:2024 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/binary_resolver.rs:70 — `if let Some(target_root) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)`
- test: integration_tests/src/binary_resolver.rs:206 — `if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs:96 — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs:120 — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- prod: mochi/mochi-core/src/supervisor.rs:874 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:1294 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:1340 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:1386 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- tool: xtask/src/kagami_profiles.rs:1307 — `if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {`
- tool: xtask/src/mochi.rs:381 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## CONNECT_NORITO_SOURCE_REVISION (build: 1)

- build: crates/connect_norito_bridge/build.rs:43 — `env::var("CONNECT_NORITO_SOURCE_REVISION").unwrap_or_else(|_| "unknown".to_owned());`

## CREDENTIALS_DIRECTORY (prod: 1)

- prod: crates/irohad/src/bin/sorafs_external_software_signer.rs:88 — `let directory = env::var_os("CREDENTIALS_DIRECTORY")`

## CRYPTO_SM_INTRINSICS (bench: 1)

- bench: crates/iroha_crypto/benches/sm_perf.rs:165 — `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`

## CUDA_HOME (build: 5)

- build: crates/fastpq_prover/build.rs:195 — `env::var_os("CUDA_HOME")`
- build: crates/gpuzstd_cuda/build.rs:83 — `for root in env::var_os("CUDA_HOME")`
- build: crates/gpuzstd_cuda/build.rs:102 — `let root = env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:88 — `for root in env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:107 — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (build: 5)

- build: crates/fastpq_prover/build.rs:196 — `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/gpuzstd_cuda/build.rs:85 — `.chain(env::var_os("CUDA_PATH"))`
- build: crates/gpuzstd_cuda/build.rs:103 — `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:90 — `.chain(env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:108 — `.or_else(|| env::var_os("CUDA_PATH"))`

## CXX (build: 4)

- build: crates/fastpq_prover/build.rs:239 — `env::var_os("CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs:133 — `env::var_os("CXX").is_some()`
- build: crates/ivm/build.rs:697 — `env::var_os("CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:138 — `env::var_os("CXX").is_some()`

## DATASPACE_ADVERSARIAL_ARTIFACT_DIR (test: 1)

- test: integration_tests/tests/nexus/cross_lane.rs:702 — `if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {`

## DOCS_RS (build: 1)

- build: crates/norito/build.rs:4 — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (bench: 1)

- bench: crates/norito/benches/enum_packed_bench.rs:69 — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_METAL_LIB (prod: 2)

- prod: crates/fastpq_prover/src/backend.rs:819 — `option_env!("FASTPQ_METAL_LIB")`
- prod: crates/fastpq_prover/src/metal.rs:2436 — `option_env!("FASTPQ_METAL_LIB")`

## FASTPQ_SKIP_GPU_BUILD (build: 1)

- build: crates/fastpq_prover/build.rs:44 — `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## FASTPQ_UPDATE_FIXTURES (test: 1)

- test: crates/fastpq_prover/tests/common/mod.rs:11 — `fixture_update_requested_from(std::env::var_os("FASTPQ_UPDATE_FIXTURES").as_deref())`

## GENESIS_DEBUG_MODE (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs:16 — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## GENESIS_DEBUG_PAYLOAD (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs:135 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## GITHUB_STEP_SUMMARY (prod: 2)

- prod: crates/iroha_crypto/src/bin/gost_perf_check.rs:28 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:185 — `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## GIT_COMMIT_HASH (prod: 3)

- prod: crates/iroha_core/src/bin/pk2_bridge_finality_verify.rs:1231 — `option_env!("GIT_COMMIT_HASH")`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1337 — `option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),`
- prod: crates/iroha_core/src/sumeragi/v2_runner.rs:2213 — `option_env!("GIT_COMMIT_HASH")`

## GPUZSTD_CUDA_ARCH (build: 1)

- build: crates/gpuzstd_cuda/build.rs:44 — `if let Some(arch_flag) = env::var_os("GPUZSTD_CUDA_ARCH") {`

## GPUZSTD_CUDA_REQUIRE (test: 3)

- test: crates/gpuzstd_cuda/src/lib.rs:297 — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some() {`
- test: crates/gpuzstd_cuda/src/lib.rs:694 — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_none() {`
- test: crates/norito/src/core/gpu_zstd.rs:674 — `std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some()`

## GPUZSTD_CUDA_SKIP_BUILD (build: 1)

- build: crates/gpuzstd_cuda/build.rs:17 — `if env::var_os("GPUZSTD_CUDA_SKIP_BUILD").is_some() {`

## HOME (prod: 7, test: 1)

- prod: crates/iroha/src/config.rs:54 — `env::var_os("HOME").map(PathBuf::from)`
- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:1375 — `if let Some(home) = env::var_os("HOME") {`
- prod: crates/irohad/src/soracloud_runtime.rs:19689 — `if let Some(home) = std::env::var_os("HOME") {`
- test: crates/irohad/src/soracloud_runtime.rs:32026 — `if std::env::var_os("HOME").is_none() {`
- prod: crates/musubi/src/cache.rs:154 — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/musubi/src/cache.rs:171 — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/musubi/src/command.rs:3217 — `let root = std::env::var_os("HOME").map(PathBuf::from).map(|path| {`
- prod: crates/musubi/src/command.rs:3228 — `std::env::var_os("HOME")`

## HOST_CXX (build: 4)

- build: crates/fastpq_prover/build.rs:240 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs:134 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/ivm/build.rs:698 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:139 — `|| env::var_os("HOST_CXX").is_some()`

## IROHA_ACCOUNT_ID (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:251 — `account_id: std::env::var("IROHA_ACCOUNT_ID").ok(),`

## IROHA_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs:7566 — `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## IROHA_API_BASE (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:244 — `api_base: std::env::var("IROHA_API_BASE")`

## IROHA_CHAIN_ID (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:249 — `chain_id: std::env::var("IROHA_CHAIN_ID")`

## IROHA_CONF_GAS_SEED (test: 1)

- test: crates/iroha_test_samples/src/lib.rs:72 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_CONNECT_ACCOUNT_SEED_HEX (example: 1)

- example: crates/iroha_torii_shared/examples/connect_wallet.rs:74 — `let account_seed_hex = std::env::var("IROHA_CONNECT_ACCOUNT_SEED_HEX")`

## IROHA_CONNECT_RECIPIENT_FIXTURE_STAGE (test: 1)

- test: crates/connect_norito_bridge/src/kagemusha_bridge_tests/tests/recipient_fixture_owner.rs:30 — `let Some(raw_stage) = env::var_os("IROHA_CONNECT_RECIPIENT_FIXTURE_STAGE") else {`

## IROHA_DA_SPOOL_DIR (test: 1)

- test: crates/iroha_core/src/state.rs:25834 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_DEBUG_GENESIS_PATH (test: 3)

- test: crates/iroha_genesis/src/genesis_manifest_tests.rs:635 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/genesis_manifest_tests.rs:683 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/genesis_manifest_tests.rs:708 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`

## IROHA_DEBUG_SIGNED_GENESIS_PATH (test: 1)

- test: crates/iroha_genesis/src/genesis_manifest_tests.rs:657 — `let path = env::var("IROHA_DEBUG_SIGNED_GENESIS_PATH")`

## IROHA_DPN_VALIDATOR_RELEASE_COMMIT (test: 1)

- test: crates/iroha_telemetry/src/metrics.rs:5020 — `dpn_validator_release_commit: option_env!("IROHA_DPN_VALIDATOR_RELEASE_COMMIT")`

## IROHA_DUMP_MANIFEST_JSON (test: 1)

- test: crates/iroha_data_model/src/nexus/manifest.rs:1626 — `if std::env::var_os("IROHA_DUMP_MANIFEST_JSON").is_some() {`

## IROHA_GENESIS_FILE (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs:18 — `let genesis_path = std::env::var("IROHA_GENESIS_FILE")`

## IROHA_GENESIS_PUBLIC_KEY (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs:20 — `let pub_key_str = std::env::var("IROHA_GENESIS_PUBLIC_KEY").unwrap_or_else(|_| {`

## IROHA_GIT_COMMIT_HASH (build: 1, prod: 6, test: 1)

- build: crates/iroha_core/build.rs:607 — `let commit = env::var("IROHA_GIT_COMMIT_HASH").ok()?;`
- prod: crates/iroha_core/src/bin/pk2_bridge_finality_verify.rs:52 — `const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");`
- prod: crates/iroha_core/src/bin/pk2_bridge_finality_verify.rs:2081 — `assert_eq!(embedded, option_env!("IROHA_GIT_COMMIT_HASH"));`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1338 — `option_env!("IROHA_GIT_COMMIT_HASH").unwrap_or("unknown"),`
- prod: crates/iroha_js_host/src/lib.rs:816 — `option_env!("IROHA_GIT_COMMIT_HASH")`
- test: crates/iroha_js_host/src/lib.rs:18667 — `option_env!("IROHA_GIT_COMMIT_HASH").unwrap_or("unknown")`
- prod: crates/iroha_kagami/src/main.rs:44 — `const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");`
- prod: crates/irohad/src/main.rs:190 — `const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");`

## IROHA_INROU_PORTABLE (test: 3)

- test: crates/irohad/src/soracloud_runtime.rs:23965 — `if std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:33198 — `|| std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime/tests/runtime_tail.rs:8 — `|| std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")`

## IROHA_INROU_PORTABLE_INITRD_IMAGE (test: 1, tool: 1)

- test: crates/irohad/src/soracloud_runtime.rs:33208 — `let initrd_image = std::env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`
- tool: xtask/src/soracloud_inrou.rs:73 — `if let Ok(value) = env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`

## IROHA_INROU_PORTABLE_SMOKE_ENTRYPOINT (test: 1)

- test: crates/irohad/src/soracloud_runtime/tests/runtime_tail.rs:18 — `let external_entrypoint = std::env::var("IROHA_INROU_PORTABLE_SMOKE_ENTRYPOINT")`

## IROHA_INROU_PORTABLE_SMOKE_HEALTHCHECK (test: 1)

- test: crates/irohad/src/soracloud_runtime/tests/runtime_tail.rs:20 — `let external_healthcheck = std::env::var("IROHA_INROU_PORTABLE_SMOKE_HEALTHCHECK")`

## IROHA_KAGAMI_LOCALNET_KEEP (test: 1)

- test: integration_tests/tests/sumeragi_kagami_localnet.rs:75 — `if std::env::var_os("IROHA_KAGAMI_LOCALNET_KEEP").is_some() {`

## IROHA_MCP_URL (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:248 — `mcp_url: std::env::var("IROHA_MCP_URL").ok().or_else(|| {mcp_url}),`

## IROHA_METRICS_PANIC_ON_DUPLICATE (test: 2)

- test: crates/iroha_telemetry/src/metrics.rs:9856 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- test: crates/iroha_torii/tests/metrics_registry.rs:30 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## IROHA_MOCHI_CANONICAL_FIXTURE_STAGE (test: 1)

- test: mochi/mochi-core/src/torii/tests/canonical_fixture_owner.rs:19 — `let Some(raw_stage) = env::var_os("IROHA_MOCHI_CANONICAL_FIXTURE_STAGE") else {`

## IROHA_MOCHI_REPLAY_FIXTURE_STAGE (test: 1)

- test: mochi/mochi-integration/src/mock_torii/tests/replay_fixture_owner.rs:21 — `let Some(raw_stage) = env::var_os("IROHA_MOCHI_REPLAY_FIXTURE_STAGE") else {`

## IROHA_PRIVATE_KEY (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:252 — `private_key: std::env::var("IROHA_PRIVATE_KEY").ok(),`

## IROHA_REALISTIC_30TPS_LOAD_KIND (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:518 — `let Some(raw) = std::env::var("IROHA_REALISTIC_30TPS_LOAD_KIND")`

## IROHA_REALISTIC_30TPS_LOG_LEVEL (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2585 — `std::env::var("IROHA_REALISTIC_30TPS_LOG_LEVEL").unwrap_or_else(|_| "WARN".into());`

## IROHA_RUN_IGNORED (test: 38)

- test: crates/iroha_core/tests/check_genesis_sig.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_finalize_real_vk.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_zk_ballot_lock_verified.rs:9 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_zk_ballot_real_vk.rs:9 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/zk_roots_get_cap.rs:39 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/zk_vote_get_tally.rs:43 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:12 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:30 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:47 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:69 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:450 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:840 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:897 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:987 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1099 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1195 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1293 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1397 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1470 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1572 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1699 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_mode_mismatch_and_autoclose.rs:39 — `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints_router.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_read_endpoints_router.rs:36 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:23959 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:33197 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime/tests/runtime_tail.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/ivm/tests/beep_test.rs:6 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/kotodama_struct_fields.rs:10 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:45 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:278 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:435 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:504 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/pipeline_block_rejected.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/sorting.rs:40 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_RUN_ZK_WRAPPERS (test: 1)

- test: crates/ivm/tests/kotodama_wrappers.rs:3 — `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`

## IROHA_SCCP_BUILD_FEATURES (prod: 1)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:595 — `env!("IROHA_SCCP_BUILD_FEATURES")`

## IROHA_SCCP_BUILD_PROFILE (prod: 2)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:668 — `build_profile: env!("IROHA_SCCP_BUILD_PROFILE"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:683 — `build_profile: env!("IROHA_SCCP_BUILD_PROFILE").to_owned(),`

## IROHA_SCCP_BUILD_TARGET (prod: 2)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:669 — `target_triple: env!("IROHA_SCCP_BUILD_TARGET"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:684 — `target_triple: env!("IROHA_SCCP_BUILD_TARGET").to_owned(),`

## IROHA_SCCP_RUSTC_VERSION (prod: 2)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:670 — `rustc_version: env!("IROHA_SCCP_RUSTC_VERSION"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:685 — `rustc_version: env!("IROHA_SCCP_RUSTC_VERSION").to_owned(),`

## IROHA_SKIP_BIND_CHECKS (test: 1)

- test: crates/iroha_test_network/src/lib.rs:7882 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (test: 1)

- test: crates/iroha_crypto/tests/sm_cli_matrix.rs:38 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_STARTUP_TRACE (prod: 1)

- prod: crates/irohad/src/main.rs:192 — `env::var_os("IROHA_STARTUP_TRACE").is_some()`

## IROHA_SWIFT_UNSHIELD_ATTACHMENT_PATH (test: 1)

- test: crates/iroha_core/tests/swift_confidential_unshield_redeem.rs:22 — `let Ok(path) = env::var("IROHA_SWIFT_UNSHIELD_ATTACHMENT_PATH") else {`

## IROHA_TEST_BUILD_PROFILE (test: 1)

- test: integration_tests/src/binary_resolver.rs:158 — `std::env::var("IROHA_TEST_BUILD_PROFILE").ok().as_deref(),`

## IROHA_TEST_CLIENT_TTL_MS (test: 5)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2565 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3694 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3933 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4104 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4693 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`

## IROHA_TEST_DUMP_GENESIS (test: 1)

- test: crates/iroha_test_network/src/lib.rs:14115 — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`

## IROHA_TEST_NETWORK_PARALLELISM (test: 1)

- test: integration_tests/tests/address_canonicalisation.rs:42 — `if let Ok(raw) = env::var("IROHA_TEST_NETWORK_PARALLELISM")`

## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (build: 1, test: 1)

- test: crates/iroha_test_network/src/config.rs:494 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- build: integration_tests/build.rs:64 — `if env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_SERIALIZE_NETWORKS (test: 2)

- test: integration_tests/tests/address_canonicalisation.rs:37 — `if let Ok(raw) = env::var("IROHA_TEST_SERIALIZE_NETWORKS")`
- test: integration_tests/tests/asset.rs:248 — `if std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none() {`

## IROHA_TEST_SKIP_BUILD (test: 1)

- test: integration_tests/src/binary_resolver.rs:44 — `std::env::var("IROHA_TEST_SKIP_BUILD").ok().as_deref(),`

## IROHA_TEST_USE_DEFAULT_EXECUTOR (test: 2)

- test: crates/iroha_core/src/executor.rs:17910 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- test: crates/iroha_core/src/executor_contract_dispatch_tests.rs:258 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`

## IROHA_THROUGHPUT_ARTIFACT_DIR (test: 3)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3348 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4644 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:5228 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`

## IROHA_THROUGHPUT_DELAY_MS (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:471 — `if let Ok(delay) = std::env::var("IROHA_THROUGHPUT_DELAY_MS") {`

## IROHA_TORII_ALLOW_LIVE_ASSET_HOLDER_AGGREGATE (test: 1)

- test: crates/iroha_torii/src/routing.rs:63415 — `|| std::env::var("IROHA_TORII_ALLOW_LIVE_ASSET_HOLDER_AGGREGATE")`

## IROHA_TORII_OPENAPI_ACTUAL (test: 1)

- test: crates/iroha_torii/tests/router_feature_matrix.rs:79 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_EXPECTED (test: 2)

- test: crates/iroha_torii/tests/router_feature_matrix.rs:74 — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- test: crates/iroha_torii/tests/router_feature_matrix.rs:88 — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_TORII_OPENAPI_TOKENS (tool: 2)

- tool: xtask/src/main.rs:13681 — `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`
- tool: xtask/src/main.rs:13740 — `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IROHA_TORII_PUBLIC_DATASPACE_UPSTREAMS (prod: 1)

- prod: crates/iroha_torii/src/lib.rs:2189 — `let Ok(raw) = std::env::var("IROHA_TORII_PUBLIC_DATASPACE_UPSTREAMS") else {`

## IROHA_TORII_URL (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:246 — `torii_url: std::env::var("IROHA_TORII_URL")`

## IVM_BIN (test: 2)

- test: integration_tests/tests/kotodama_examples.rs:57 — `let ivm_bin = env::var("IVM_BIN")`
- test: integration_tests/tests/kotodama_examples.rs:145 — `let ivm_bin = env::var("IVM_BIN")`

## IVM_COMPILER_DEBUG (test: 1)

- test: crates/kotodama_lang/src/compiler.rs:15780 — `if cfg!(any(test, debug_assertions)) && std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_GENCODE (build: 1)

- build: crates/ivm/build.rs:565 — `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| DEFAULT_CUDA_GENCODE.to_string());`

## IVM_CUDA_NVCC (build: 1)

- build: crates/ivm/build.rs:559 — `let executable = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVCC_EXTRA (build: 1)

- build: crates/ivm/build.rs:566 — `let extra_flags = env::var("IVM_CUDA_NVCC_EXTRA")`

## IVM_CUDA_PTX_MODE (build: 1)

- build: crates/ivm/build.rs:535 — `match env::var("IVM_CUDA_PTX_MODE") {`

## IVM_CUDA_SELFTEST_TRACE (prod: 1)

- prod: crates/ivm/src/cuda.rs:101 — `if std::env::var_os("IVM_CUDA_SELFTEST_TRACE").is_some() {`

## IVM_DEBUG_AED_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:353 — `let aed_asset_raw = std::env::var("IVM_DEBUG_AED_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:228 — `let asset_raw = std::env::var("IVM_DEBUG_ASSET_DEFINITION")`

## IVM_DEBUG_CBDC_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:359 — `let cbdc_asset_raw = std::env::var("IVM_DEBUG_CBDC_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_DOMAIN (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:230 — `let domain_raw = std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank".to_owned());`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:366 — `std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank.universal".to_owned());`

## IVM_DEBUG_FROM_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:226 — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:351 — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`

## IVM_DEBUG_METAL_ENUM (debug: 1)

- debug: crates/ivm/src/vector.rs:423 — `std::env::var("IVM_DEBUG_METAL_ENUM")`

## IVM_DEBUG_METAL_SELFTEST (debug: 1)

- debug: crates/ivm/src/vector.rs:733 — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DEBUG_RATIO (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:367 — `let ratio_raw = std::env::var("IVM_DEBUG_RATIO").unwrap_or_else(|_| "76".to_owned());`

## IVM_DEBUG_TO_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:227 — `let to_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:352 — `let dst_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`

## IVM_DISABLE_CUDA (debug: 1, test: 2)

- debug: crates/ivm/src/cuda.rs:809 — `&& std::env::var("IVM_DISABLE_CUDA")`
- test: crates/ivm/tests/cuda_disable_on_mismatch.rs:40 — `original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),`
- test: crates/ivm/tests/cuda_env.rs:26 — `original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),`

## IVM_DISABLE_METAL (debug: 1)

- debug: crates/ivm/src/vector.rs:273 — `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELFTEST_FAIL (debug: 1, test: 2)

- debug: crates/ivm/src/cuda.rs:820 — `&& std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL")`
- test: crates/ivm/tests/cuda_disable_on_mismatch.rs:39 — `original_force_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),`
- test: crates/ivm/tests/cuda_env.rs:27 — `original_force_selftest_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),`

## IVM_FORCE_METAL_ENUM (debug: 1)

- debug: crates/ivm/src/vector.rs:399 — `std::env::var("IVM_FORCE_METAL_ENUM")`

## IVM_FORCE_METAL_SELFTEST_FAIL (debug: 1)

- debug: crates/ivm/src/vector.rs:722 — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IVM_TOOL_BIN (test: 1)

- test: integration_tests/tests/kotodama_examples.rs:99 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## IZANAMI_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs:7565 — `std::env::var("IZANAMI_ALLOW_NET")`

## IZANAMI_TUI_ALLOW_ZERO_SEED (prod: 1)

- prod: crates/izanami/src/tui.rs:174 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:45 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_REQUIRE (test: 3)

- test: crates/norito/accelerators/jsonstage1_cuda/src/lib.rs:396 — `std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some()`
- test: crates/norito/src/core/sequence_plan_helper_tests.rs:60 — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`
- test: crates/norito/src/lib.rs:5876 — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`

## JSONSTAGE1_CUDA_SKIP_BUILD (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:20 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_HEX (prod: 1)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:230 — `option_env!("KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_HEX");`

## KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256 (prod: 1)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:232 — `option_env!("KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256");`

## KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256 (prod: 1)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:234 — `option_env!("KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256");`

## KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256 (prod: 1)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:236 — `option_env!("KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256");`

## KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256 (prod: 1)

- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1341 — `option_env!("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256").unwrap_or("unknown"),`

## KAGEMUSHA_BUILD_SOURCE_COMMIT (prod: 2)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:226 — `const BUILD_SOURCE_COMMIT: Option<&str> = option_env!("KAGEMUSHA_BUILD_SOURCE_COMMIT");`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1339 — `option_env!("KAGEMUSHA_BUILD_SOURCE_COMMIT").unwrap_or("unknown"),`

## KAGEMUSHA_BUILD_SOURCE_DATE_EPOCH (prod: 1)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:228 — `const BUILD_SOURCE_DATE_EPOCH: Option<&str> = option_env!("KAGEMUSHA_BUILD_SOURCE_DATE_EPOCH");`

## KAGEMUSHA_BUILD_SOURCE_TREE_SHA256 (prod: 2)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:227 — `const BUILD_SOURCE_TREE_SHA256: Option<&str> = option_env!("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256");`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1340 — `option_env!("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256").unwrap_or("unknown"),`

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
- prod: crates/musubi/src/command.rs:3213 — `let root = std::env::var_os("LOCALAPPDATA")`

## MOCHI_CONFIG (prod: 1)

- prod: mochi/mochi-ui-egui/src/config.rs:447 — `if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## MOCHI_DATA_ROOT (prod: 1)

- prod: mochi/mochi-core/src/supervisor.rs:5097 — `std::env::var_os("MOCHI_DATA_ROOT")`

## MOCHI_DETACHED (prod: 1)

- prod: mochi/mochi-ui-egui/src/sandbox_cli.rs:538 — `if env::var_os("MOCHI_DETACHED").is_some() {`

## MOCHI_TEST_USE_INTERNAL_GENESIS (prod: 1)

- prod: mochi/mochi-core/src/supervisor.rs:4847 — `if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {`

## NORITO_CHECK_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs:12 — `if env::var_os("NORITO_CHECK_BINDINGS_SYNC").is_none() {`

## NORITO_CPU_INFO (tool: 1)

- tool: xtask/src/stage1_bench.rs:61 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_CUDA_REQUIRE (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:1100 — `if std::env::var_os("NORITO_CRC64_CUDA_REQUIRE").is_none() {`

## NORITO_CRC64_GPU_LIB (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:274 — `let raw = std::env::var_os("NORITO_CRC64_GPU_LIB")?;`

## NORITO_DISABLE_PACKED_STRUCT (test: 1)

- test: crates/norito/src/lib.rs:334 — `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## NORITO_GPU_CRC64_MIN_BYTES (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:60 — `let configured = std::env::var("NORITO_GPU_CRC64_MIN_BYTES")`

## NORITO_PAR_STAGE1_MIN (test: 1)

- test: crates/norito/src/lib.rs:6136 — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## NORITO_SKIP_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs:9 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_GPU_MIN_BYTES (test: 1)

- test: crates/norito/src/lib.rs:6165 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## NORITO_TRACE (test: 3)

- test: crates/norito/src/lib.rs:144 — `std::env::var_os("NORITO_TRACE").is_some()`
- test: crates/norito/src/lib.rs:149 — `*ENABLED.get_or_init(|| std::env::var_os("NORITO_TRACE").is_some())`
- test: crates/norito/src/lib.rs:162 — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## NOTIFY_SOCKET (test: 1)

- test: crates/irohad/src/runtime_provider_broker/launcher.rs:623 — `let notify_socket = std::env::var_os("NOTIFY_SOCKET")`

## NVCC (build: 1)

- build: crates/ivm/build.rs:560 — `.or_else(|_| env::var("NVCC"))`

## OUT_DIR (build: 8, prod: 17, test: 5)

- build: crates/connect_norito_bridge/build.rs:18 — `let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));`
- build: crates/fastpq_prover/build.rs:98 — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
- build: crates/iroha_data_model/build.rs:10 — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));`
- prod: crates/iroha_data_model/src/lib.rs:211 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
- build: crates/ivm/build.rs:321 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs:396 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs:462 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs:675 — `if let Some(out_dir) = env::var_os("OUT_DIR") {`
- prod: crates/ivm/src/cuda.rs:25 — `static PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- prod: crates/ivm/src/cuda.rs:26 — `static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));`
- prod: crates/ivm/src/cuda.rs:27 — `static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));`
- prod: crates/ivm/src/cuda.rs:28 — `static SHA_LEAVES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx"));`
- prod: crates/ivm/src/cuda.rs:29 — `static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));`
- prod: crates/ivm/src/cuda.rs:30 — `static SHA3_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx"));`
- prod: crates/ivm/src/cuda.rs:31 — `static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));`
- prod: crates/ivm/src/cuda.rs:32 — `static BN254_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx"));`
- prod: crates/ivm/src/cuda.rs:33 — `static SIG_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/signature.ptx"));`
- prod: crates/ivm/src/cuda.rs:34 — `static SHA_PAIRS_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx"));`
- prod: crates/ivm/src/cuda.rs:36 — `static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));`
- test: crates/ivm/src/gpu_manager.rs:336 — `static ADD_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- prod: crates/ivm/src/iso20022.rs:229 — `include!(concat!(env!("OUT_DIR"), "/iso20022_schema_v1.rs"));`
- prod: crates/ivm/src/ivm.rs:225 — `include!(concat!(env!("OUT_DIR"), "/syscall_signatures.rs"));`
- test: crates/ivm/src/ptx_tests.rs:6 — `let out_dir = env!("OUT_DIR");`
- test: crates/ivm/tests/ptx_kernels.rs:6 — `let out_dir = env!("OUT_DIR");`
- build: crates/kotodama_lang/build.rs:597 — `let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));`
- prod: crates/kotodama_lang/src/diagnostic.rs:75 — `env!("OUT_DIR"),`
- prod: crates/kotodama_lang/src/i18n/mod.rs:10 — `include_bytes!(concat!(env!("OUT_DIR"), "/kotodama_i18n_v1_offsets.bin"));`
- prod: crates/kotodama_lang/src/lexer.rs:114 — `include!(concat!(env!("OUT_DIR"), "/kotodama_v1_lexical.rs"));`
- test: crates/kotodama_lang/tests/compile_fail_goldens.rs:17 — `include!(concat!(env!("OUT_DIR"), "/kotodama_compile_fail_cases.rs"));`
- test: crates/kotodama_lang/tests/secret_security_diagnostics.rs:16 — `include!(concat!(env!("OUT_DIR"), "/kotodama_secret_reject_cases.rs"));`

## PATH (prod: 4, test: 1)

- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:1353 — `if env::var_os("PATH").as_deref() != Some(OsStr::new("/usr/bin:/bin")) {`
- prod: crates/irohad/src/soracloud_runtime.rs:17804 — `if let Some(path) = std::env::var_os("PATH") {`
- prod: crates/irohad/src/soracloud_runtime.rs:19608 — `let mut search_roots = std::env::var_os("PATH")`
- test: integration_tests/tests/kotodama_examples.rs:18 — `let path = env::var_os("PATH")?;`
- prod: mochi/mochi-core/src/supervisor.rs:1237 — `let path_var = env::var_os("PATH")?;`

## PATHEXT (prod: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:19647 — `std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());`

## PRINT_SORACLES_FIXTURES (test: 1)

- test: crates/iroha_data_model/src/oracle/mod.rs:3178 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORII_SPEC (test: 1)

- test: crates/iroha_torii/src/openapi.rs:2180 — `if std::env::var("PRINT_TORII_SPEC").is_ok() {`

## PROFILE (build: 2, test: 3)

- build: crates/iroha_sccp/build.rs:24 — `let profile = env::var("PROFILE").expect("Cargo must provide the exact build profile");`
- test: crates/iroha_test_network/src/lib.rs:1739 — `if let Ok(profile) = std::env::var("PROFILE") {`
- build: integration_tests/build.rs:54 — `let profile = if env::var("PROFILE").ok().as_deref() == Some("release") {`
- test: integration_tests/src/binary_resolver.rs:218 — `if let Ok(profile) = std::env::var("PROFILE")`
- test: integration_tests/src/kagami.rs:66 — `let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());`

## PYTHON3 (test: 1)

- test: crates/sorafs_car/tests/taikai_viewer_cli.rs:27 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONDONTWRITEBYTECODE (prod: 1)

- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:1365 — `if let Some(value) = env::var_os("PYTHONDONTWRITEBYTECODE")`

## PYTHONPATH (test: 1)

- test: crates/iroha_cli/tests/cli_smoke.rs:5135 — `match env::var("PYTHONPATH") {`

## REPO_PROOF_DIGEST_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2590 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2578 — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUSTC (build: 1)

- build: crates/iroha_sccp/build.rs:13 — `let rustc = env::var_os("RUSTC").expect("Cargo must provide RUSTC to the SCCP build script");`

## RUST_LOG (prod: 2, test: 3)

- test: crates/iroha_test_network/src/lib.rs:11102 — `let original = env::var("RUST_LOG").ok();`
- test: crates/iroha_test_network/src/lib.rs:11115 — `let original = env::var("RUST_LOG").ok();`
- prod: crates/izanami/src/chaos.rs:2397 — `if let Ok(filter) = std::env::var("RUST_LOG") {`
- prod: crates/izanami/src/config.rs:622 — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`
- test: integration_tests/tests/sumeragi_kagami_localnet.rs:133 — `if std::env::var_os("RUST_LOG").is_none() {`

## RUST_MIN_STACK (test: 1)

- test: crates/iroha_core/src/privacy_release_evidence/tests.rs:793 — `std::env::var("RUST_MIN_STACK").as_deref(),`

## SM_PERF_CPU_LABEL (prod: 2)

- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:657 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:693 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## SORAFS_NODE_SKIP_INGEST_TESTS (test: 1)

- test: crates/sorafs_node/tests/cli.rs:17 — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORAFS_TORII_SKIP_INGEST_TESTS (test: 1)

- test: crates/iroha_torii/tests/sorafs_discovery.rs:121 — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SOURCE_DATE_EPOCH (build: 1)

- build: crates/iroha_core/build.rs:143 — `env::var("SOURCE_DATE_EPOCH").ok().as_deref(),`

## SUMERAGI_BASELINE_ARTIFACT_DIR (prod: 1, test: 1)

- prod: crates/build-support/src/bin/sumeragi_baseline_report.rs:36 — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`
- test: integration_tests/tests/sumeragi_npos_performance.rs:741 — `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`

## SUMERAGI_DA_ARTIFACT_DIR (prod: 1)

- prod: crates/build-support/src/bin/sumeragi_da_report.rs:37 — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`

## SystemRoot (prod: 2)

- prod: crates/fastpq_prover/src/backend.rs:542 — `env::var_os("SystemRoot").map(PathBuf::from)`
- prod: crates/irohad/src/soracloud_runtime.rs:17801 — `if let Some(system_root) = std::env::var_os("SystemRoot") {`

## TARGET (build: 2, prod: 1, tool: 2)

- prod: crates/build-support/src/lib.rs:26 — `let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());`
- build: crates/iroha_sccp/build.rs:23 — `let target = env::var("TARGET").expect("Cargo must provide the exact target triple");`
- build: crates/ivm/build.rs:29 — `if let Ok(target) = env::var("TARGET") {`
- tool: xtask/src/poseidon_bench.rs:78 — `target: std::env::var("TARGET")`
- tool: xtask/src/stage1_bench.rs:55 — `target: std::env::var("TARGET")`

## TEST_LOG_FILTER (prod: 1)

- prod: crates/iroha_logger/src/lib.rs:114 — `filter: std::env::var("TEST_LOG_FILTER")`

## TEST_LOG_LEVEL (prod: 1)

- prod: crates/iroha_logger/src/lib.rs:110 — `level: std::env::var("TEST_LOG_LEVEL")`

## TEST_NETWORK_CARGO (test: 1)

- test: crates/iroha_test_network/src/lib.rs:2377 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TEST_NETWORK_IROHAD_FEATURES (test: 5)

- test: integration_tests/tests/nexus/cross_dataspace_zk_stark_localnet.rs:212 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/privacy_exact12_activation_network.rs:85 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/privacy_exact12_zk_x509_network.rs:103 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_ace_localnet.rs:43 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_stark_network.rs:70 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`

## TMPDIR (prod: 2)

- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:653 — `let temporary = env::var_os("TMPDIR")`
- prod: crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller.rs:1372 — `let temporary = env::var_os("TMPDIR")`

## TORII_MOCK_HARNESS_METRICS_PATH (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:92 — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:95 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_RETRY_TOTAL (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:271 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:98 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:90 — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## TORII_OPENAPI_TOKEN (tool: 2)

- tool: xtask/src/main.rs:13676 — `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`
- tool: xtask/src/main.rs:13736 — `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## UPDATE_FIXTURES (test: 2)

- test: crates/iroha_core/tests/pin_registry.rs:118 — `if env::var_os("UPDATE_FIXTURES").is_some() {`
- test: crates/iroha_core/tests/snapshots.rs:43 — `let update = env::var("UPDATE_FIXTURES")`

## USERPROFILE (prod: 1)

- prod: crates/iroha/src/config.rs:52 — `env::var_os("USERPROFILE").map(PathBuf::from)`

## VERGEN_CARGO_FEATURES (test: 2)

- test: crates/iroha_telemetry/src/metrics.rs:5023 — `cargo_features: option_env!("VERGEN_CARGO_FEATURES")`
- test: crates/irohad/src/main.rs:18743 — `const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {`

## VERGEN_CARGO_TARGET_TRIPLE (prod: 1, test: 1)

- test: crates/iroha_telemetry/src/metrics.rs:5026 — `target_triple: option_env!("VERGEN_CARGO_TARGET_TRIPLE")`
- prod: crates/iroha_telemetry/src/ws.rs:241 — `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");`

## VERGEN_GIT_SHA (prod: 2, test: 2)

- prod: crates/iroha_cli/src/main_shared.rs:62 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`
- test: crates/iroha_telemetry/src/metrics.rs:5017 — `git_commit_sha: option_env!("VERGEN_GIT_SHA")`
- prod: crates/iroha_telemetry/src/ws.rs:240 — `let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");`
- test: crates/irohad/src/main.rs:18739 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`

## XDG_CACHE_HOME (prod: 1)

- prod: crates/musubi/src/cache.rs:167 — `if let Some(root) = std::env::var_os("XDG_CACHE_HOME") {`

## XDG_STATE_HOME (prod: 1)

- prod: crates/musubi/src/command.rs:3224 — `let root = std::env::var_os("XDG_STATE_HOME")`

## XTASK_TEST_KAGAMI_BIN (test: 2)

- test: xtask/src/kagami_profiles.rs:2258 — `if std::env::var("XTASK_TEST_KAGAMI_BIN").is_err() {`
- test: xtask/src/kagami_profiles.rs:2261 — `let kagami_path = PathBuf::from(std::env::var("XTASK_TEST_KAGAMI_BIN").unwrap());`
