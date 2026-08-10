# Environment toggle inventory

_Last refreshed via `python3 scripts/inventory_env_toggles.py --json specs/agents/env_var_inventory.json --md specs/agents/env_var_inventory.md`_

Total references: **797** · Unique variables: **207**

## ANDROID_SDK_ROOT (prod: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:15388 — `if let Some(android_sdk_root) = std::env::var_os("ANDROID_SDK_ROOT") {`

## BLOCK_DUMP_HEIGHTS (example: 1)

- example: crates/iroha_core/examples/block_dump.rs:67 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`

## BLOCK_DUMP_SUM_ASSET (example: 1)

- example: crates/iroha_core/examples/block_dump.rs:56 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`

## BLOCK_DUMP_VERBOSE (example: 1)

- example: crates/iroha_core/examples/block_dump.rs:55 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`

## CARGO (prod: 3, test: 3)

- test: crates/iroha_test_network/src/lib.rs:2820 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- test: crates/norito_derive/tests/ui.rs:36 — `let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());`
- test: integration_tests/src/kagami.rs:162 — `let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());`
- prod: mochi/mochi-core/src/supervisor.rs:1418 — `let cargo = env::var_os("CARGO")`
- prod: mochi/mochi-core/src/supervisor.rs:1473 — `let cargo = env::var_os("CARGO")`
- prod: mochi/mochi-core/src/supervisor.rs:1527 — `let cargo = env::var_os("CARGO")`

## CARGO_BIN_EXE_attachment_sanitizer (test: 9)

- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:99 — `let mut cmd = Command::new(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:142 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:167 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:318 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:343 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:366 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:383 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:400 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:418 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`

## CARGO_BIN_EXE_iroha (test: 2)

- test: crates/iroha_cli/tests/cli_smoke.rs:51 — `env!("CARGO_BIN_EXE_iroha")`
- test: crates/iroha_cli/tests/taikai_policy.rs:22 — `env!("CARGO_BIN_EXE_iroha")`

## CARGO_BIN_EXE_iroha_monitor (test: 4)

- test: crates/iroha_monitor/tests/attach_render.rs:13 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/http_limits.rs:12 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/invalid_credentials.rs:11 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- test: crates/iroha_monitor/tests/smoke.rs:11 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## CARGO_BIN_EXE_kagami (test: 3)

- test: crates/iroha_kagami/tests/common/mod.rs:21 — `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- test: crates/iroha_kagami/tests/pop_embed.rs:36 — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- test: integration_tests/src/kagami.rs:70 — `if let Ok(path) = env::var("CARGO_BIN_EXE_kagami") {`

## CARGO_BIN_EXE_kagami_mock (test: 1)

- test: mochi/mochi-integration/tests/supervisor.rs:33 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## CARGO_BIN_EXE_koto (test: 4)

- test: crates/ivm/tests/cli_smoke.rs:6 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/ivm/tests/cli_smoke.rs:59 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/ivm/tests/cli_smoke.rs:96 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/ivm/tests/cli_smoke.rs:135 — `let bin = env!("CARGO_BIN_EXE_koto");`

## CARGO_BIN_EXE_sorafs_chunk_dump (test: 1)

- test: crates/sorafs_chunker/tests/one_gib.rs:105 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## CARGO_BIN_NAME (prod: 3)

- prod: crates/iroha_cli/src/main_shared.rs:70 — `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`
- prod: crates/iroha_cli/src/main_shared.rs:403 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- prod: crates/iroha_kagami/src/genesis/generate.rs:1040 — `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`

## CARGO_BUILD_TARGET (tool: 2)

- tool: xtask/src/poseidon_bench.rs:89 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- tool: xtask/src/stage1_bench.rs:69 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_FEATURE (prod: 1)

- prod: crates/build-support/src/lib.rs:35 — `let parsed_features = env::var("CARGO_CFG_FEATURE")`

## CARGO_CFG_TARGET_ARCH (prod: 2, tool: 2)

- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:707 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:743 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- tool: xtask/src/poseidon_bench.rs:90 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- tool: xtask/src/stage1_bench.rs:70 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## CARGO_CFG_TARGET_OS (build: 4, prod: 2, tool: 2)

- build: crates/fastpq_prover/build.rs:29 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/gpuzstd_cuda/build.rs:33 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:708 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:744 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- build: crates/ivm/build.rs:299 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:37 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- tool: xtask/src/poseidon_bench.rs:92 — `os: std::env::var("CARGO_CFG_TARGET_OS")`
- tool: xtask/src/stage1_bench.rs:72 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## CARGO_FEATURE_CUDA (build: 2)

- build: crates/fastpq_prover/build.rs:27 — `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
- build: crates/ivm/build.rs:35 — `if env::var_os("CARGO_FEATURE_CUDA").is_some()`

## CARGO_FEATURE_CUDA_KERNEL (build: 2)

- build: crates/gpuzstd_cuda/build.rs:17 — `if env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_none() {`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:17 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (build: 1)

- build: crates/fastpq_prover/build.rs:28 — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## CARGO_FEATURE_FFI_EXPORT (prod: 1)

- prod: crates/build-support/src/lib.rs:289 — `let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();`

## CARGO_FEATURE_FFI_IMPORT (prod: 1)

- prod: crates/build-support/src/lib.rs:288 — `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`

## CARGO_FEATURE_KAGEMUSHA_CANDIDATE_SOURCE_SEAL (build: 1)

- build: crates/iroha_core/build.rs:26 — `if env::var_os("CARGO_FEATURE_KAGEMUSHA_CANDIDATE_SOURCE_SEAL").is_some() {`

## CARGO_MANIFEST_DIR (bench: 3, build: 6, example: 1, prod: 56, test: 269, tool: 5)

- prod: crates/build-support/src/lib.rs:108 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);`
- prod: crates/connect_norito_bridge/src/bin/swift_parity_regen.rs:306 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/lib.rs:15732 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/connect_norito_bridge/src/lib.rs:16172 — `let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/connect_norito_bridge/src/sorafs_tests.rs:171 — `fs::read(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))`
- prod: crates/fastpq_prover/src/poseidon_manifest.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/fastpq_prover/tests/packing.rs:17 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:9 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:28 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- test: crates/fastpq_prover/tests/proof_fixture.rs:24 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/trace_commitment.rs:27 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- test: crates/fastpq_prover/tests/transcript_replay.rs:96 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/client.rs:27827 — `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/sm.rs:219 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/tests/sm_signing.rs:36 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/commands/sorafs.rs:14245 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_cli/src/compute.rs:520 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_cli/src/main_shared.rs:1580 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/iroha_cli/src/soracloud.rs:14906 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/tests/cli_smoke.rs:176 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/tests/cli_smoke.rs:5542 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_config/src/parameters/user.rs:4888 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/src/parameters/user.rs:31561 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/autoscale_config.rs:10 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/da_ingest_compute_limit.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/fastpq_queue_overrides.rs:15 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:45 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:3667 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:4601 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/pipeline_cycle_ceiling.rs:68 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sccp_route_manifest_aliases.rs:27 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_gateway_runtime_providers.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_governance_dag_runtime_signer.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_native_transaction_signers.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_por_replay_archive.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_provider_ingest_finalized_archive.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_reputation_finalized_archive.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_storage_pin_aliases.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sorafs_stream_token_runtime_signer.rs:11 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sumeragi_v2_merge_runtime_config.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/transaction_ingress_limits.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/trusted_peers_pop_validation.rs:15 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- bench: crates/iroha_core/benches/blocks/common.rs:333 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- bench: crates/iroha_core/benches/blocks/common/mod.rs:342 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- bench: crates/iroha_core/benches/validation.rs:107 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- build: crates/iroha_core/build.rs:50 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR")`
- build: crates/iroha_core/build.rs:149 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- example: crates/iroha_core/examples/generate_parity_fixtures.rs:22 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:1523 — `let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:1562 — `let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:2589 — `fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))?;`
- test: crates/iroha_core/src/block.rs:25150 — `let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/executor.rs:19146 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- prod: crates/iroha_core/src/executor_contract_dispatch_tests.rs:287 — `let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_core/src/smartcontracts/isi/asset/core_numeric_mutation_tests.rs:5 — `let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2681 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2685 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud.rs:25595 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud.rs:25604 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud.rs:25613 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud.rs:36837 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_core/src/state.rs:47812 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- test: crates/iroha_core/src/streaming.rs:3608 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/iroha_core/src/tx/sandbox_state_tests.rs:70 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/default_domain_independence.rs:35 — `let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/pin_registry.rs:135 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);`
- test: crates/iroha_core/tests/pin_registry.rs:1605 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/snapshots.rs:35 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/sumeragi_doc_sync.rs:71 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/confidential_keyset_vectors.rs:57 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm2_fixture_vectors.rs:55 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm_cli_matrix.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:28 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:32 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:36 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/cbsi_offline_vectors.rs:65 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/qr_stream_fixtures.rs:13 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/qr_stream_fixtures.rs:17 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs:45 — `const FIXTURE_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/sumeragi_v2");`
- test: crates/iroha_data_model/src/identifier.rs:839 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/isi/escrow.rs:836 — `let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_data_model/src/lib.rs:235 — `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- prod: crates/iroha_data_model/src/lib.rs:239 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1110 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1249 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1259 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/qr_stream.rs:849 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_data_model/src/soranet/vpn.rs:3005 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- prod: crates/iroha_data_model/src/testing/axt.rs:17 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs:21 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/cancel_asset_lock.rs:47 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed_norito_rpc_fixture_tests.rs:16 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/transaction/signed_norito_rpc_fixture_tests.rs:25 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/account_address_vectors.rs:136 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/address_curve_registry.rs:33 — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:50 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/consensus_roundtrip.rs:1356 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:29 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:33 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:37 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:41 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:46 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:50 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:54 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:58 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:62 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:203 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/runtime_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:55 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:59 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:63 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:67 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:71 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:75 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:79 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:88 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:92 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:96 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:100 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:104 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:108 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:112 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:116 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:2429 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model_derive/src/model.rs:624 — `let data_model_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_data_model");`
- prod: crates/iroha_genesis/src/genesis_tail_tests.rs:87 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- prod: crates/iroha_genesis/src/genesis_tail_tests.rs:102 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_genesis/src/lib.rs:2592 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);`
- test: crates/iroha_genesis/src/lib.rs:6393 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_genesis/src/lib.rs:6419 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:6478 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:6577 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:6657 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_i18n/src/lib.rs:496 — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- test: crates/iroha_js_host/src/lib.rs:15259 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_js_host/src/lib.rs:18383 — `let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_kagami/samples/codec/generate.rs:13 — `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- prod: crates/iroha_kagami/samples/codec/src/main.rs:35 — `let dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_kagami/src/codec.rs:407 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/genesis/generate.rs:921 — `let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1495 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1525 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1575 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1600 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1614 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:1646 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:2012 — `let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:3286 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_kagami/src/genesis/sign.rs:3295 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/src/genesis/sign.rs:3314 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- prod: crates/iroha_kagami/src/localnet.rs:615 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_kagami/src/localnet.rs:4402 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_kagami/src/localnet.rs:4406 — `|| PathBuf::from(env!("CARGO_MANIFEST_DIR")),`
- test: crates/iroha_kagami/src/wizard.rs:1224 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/tests/codec.rs:14 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- test: crates/iroha_swarm/tests/default_compose_soranet.rs:18 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults")`
- test: crates/iroha_telemetry/tests/drill_log.rs:10 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_test_network/src/bin/taira_privacy_release_runner.rs:5191 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/privacy/exact12_v1.tsv");`
- test: crates/iroha_test_network/src/fslock_ports.rs:30 — `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- test: crates/iroha_test_network/src/fslock_ports.rs:32 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_test_network/src/lib.rs:825 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_test_samples/src/lib.rs:264 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_test_samples/src/lib.rs:301 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/da/tests/receipt_outcome_tests.rs:320 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- test: crates/iroha_torii/src/identifier_resolution.rs:704 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/soracloud.rs:13521 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/admission.rs:603 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs:30048 — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs:43501 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/zk_attachments.rs:2374 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/tests/account_address_vectors.rs:135 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/tests/accounts_portfolio.rs:104 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/tests/sorafs_discovery.rs:1552 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verifier_key.rs:764 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verifier_key.rs:768 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verify.rs:944 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/src/vega/microsoft_mc/verify.rs:948 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs:4 — `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega/engine.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_engine_reachability.rs:5 — `const FACADE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:8 — `const CRATE_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:9 — `const VEGA_FACADE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vega.rs"));`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:11 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:15 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_zkp_halo2/tests/vega_microsoft_cross_conformance.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/irohad/src/main.rs:534 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main.rs:13477 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs:13511 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs:16866 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/irohad/src/runtime_provider_broker/runtime_operation_tests.rs:1133 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/irohad/src/runtime_provider_registry.rs:5298 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime.rs:18592 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime.rs:18599 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- build: crates/ivm/build.rs:47 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- build: crates/ivm/build.rs:182 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- prod: crates/ivm/src/bin/gen_abi_hash_doc.rs:22 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: crates/ivm/src/bin/gen_header_doc.rs:128 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_pointer_types_doc.rs:101 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/gen_syscalls_doc.rs:793 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/ivm_fixture_export.rs:91 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/ivm/src/bin/ivm_prebuild.rs:19 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/ivm/src/predecoder_fixtures.rs:227 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- test: crates/ivm/tests/axt_descriptor_builder.rs:23 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm/tests/cli_smoke.rs:7 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/cli_smoke.rs:60 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/cli_smoke.rs:97 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/cli_smoke.rs:136 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/tests/docs_consistency.rs:3 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/ivm_abi_doc_sync.rs:4 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/ivm_header_doc_sync.rs:46 — `let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama.rs:2665 — `let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kotodama_lang/src/samples");`
- test: crates/ivm/tests/kotodama_argument_record.rs:54 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama_documentation_examples.rs:23 — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/tests/numeric_v1_sdk_fixture.rs:16 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/numeric_v1_sdk_fixture.rs:24 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/pointer_types_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- test: crates/ivm/tests/pointer_types_doc_generated_ivm_md.rs:7 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/repository_ivm_artifacts.rs:21 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/syscalls_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_doc_sync.rs:8 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_gas_names.rs:11 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/kotodama_lang/src/diagnostic.rs:2072 — `let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/kotodama_lang/src/doc_consistency.rs:17 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/kotodama_lang/src/doc_consistency.rs:223 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/kotodama_lang/tests/documentation_fences.rs:13 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- build: crates/norito/build.rs:22 — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- prod: crates/norito/src/bin/norito_regen_goldens.rs:11 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/norito/src/streaming/repo_fixture_test.rs:5 — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/aos_ncb_more_golden.rs:199 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- test: crates/norito/tests/json_golden_loader.rs:16 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:353 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:387 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:553 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:664 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- test: crates/norito/tests/ncb_enum_large_fixture.rs:36 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs:687 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs:994 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/provider_admission_fixtures.rs:1081 — `let committed_dir = Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_car/src/bin/soranet_trustless_verifier.rs:166 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/reference.rs:316 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/capacity_simulation_toolkit.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/da_reconstruct_cli.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:62 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:1002 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:1130 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/taikai_viewer_cli.rs:22 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/trustless_verifier.rs:13 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_chunker/src/bin/export_vectors.rs:204 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_chunker/tests/backpressure.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_chunker/tests/vectors.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/bin/sorafs-validate.rs:3150 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference.rs:6892 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference_ffi.rs:1876 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/orderbook_fixtures.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/pdp_fixtures.rs:17 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/por_fixtures.rs:18 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:21 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/replication_order_fixtures.rs:8 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/sorafs_validate_cli.rs:27 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_node/tests/cli.rs:180 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/src/lib.rs:7241 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs:7375 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs:9239 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/tests/orchestrator_parity.rs:182 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs:3090 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs:4427 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/soranet_pq/tests/kat_vectors.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- build: integration_tests/build.rs:23 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));`
- test: integration_tests/src/bin/refresh_nexus_streaming_fixtures.rs:447 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: integration_tests/src/binary_resolver.rs:172 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")`
- test: integration_tests/src/kagami.rs:193 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/src/sorafs_gateway_capability_refusal.rs:157 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- test: integration_tests/src/sorafs_gateway_conformance.rs:1384 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/asset.rs:281 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/fast_dsl_build.rs:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/genesis_json.rs:19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/genesis_json.rs:38 — `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
- test: integration_tests/tests/ivm_header_decode.rs:48 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/ivm_header_smoke.rs:27 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:74 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:121 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:170 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/cbdc_rollout_bundle.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/cbdc_whitelist.rs:27 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/global_commit.rs:19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/lane_registry.rs:13 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/norito_burn_fixture.rs:35 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/repo.rs:37 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/streaming/mod.rs:442 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/compose.rs:1593 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1597 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1601 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1605 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1609 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/supervisor.rs:105 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-core/src/supervisor.rs:887 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- prod: mochi/mochi-core/src/supervisor.rs:1409 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- prod: mochi/mochi-core/src/torii/tests_part1.rs:1541 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: mochi/mochi-core/src/torii/tests_part1.rs:1556 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: mochi/mochi-core/src/torii/tests_part1.rs:1571 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: mochi/mochi-core/src/torii/tests_part1.rs:1586 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-integration/src/mock_torii.rs:734 — `let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- test: mochi/mochi-integration/tests/supervisor.rs:168 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- prod: mochi/mochi-ui-egui/src/gui.rs:127 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:131 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:135 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:139 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:143 — `env!("CARGO_MANIFEST_DIR"),`
- tool: tools/norito_codegen_exporter/src/norito_rpc.rs:93 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: tools/soranet-handshake-harness/tests/fixtures_verify.rs:6 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/interop_parity.rs:77 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/perf_gate.rs:173 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- tool: xtask/src/bin/control_plane_mock.rs:362 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/main.rs:14616 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/sorafs/gateway_fixture.rs:29 — `env!("CARGO_MANIFEST_DIR"),`
- tool: xtask/src/sorafs/gateway_fixture.rs:33 — `env!("CARGO_MANIFEST_DIR"),`
- test: xtask/tests/address_vectors.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/android_dashboard_parity_cli.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/codec_rans_tables.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/da_proof_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/iso_bridge_lint.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/ministry_agenda.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soradns_cli.rs:14 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/sorafs_fetch_fixture.rs:8 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_bug_bounty.rs:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_billing.rs:12 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_billing_m0.rs:30 — `let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_m1.rs:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_gateway_m2.rs:25 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:77 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:131 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:202 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:306 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:356 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/soranet_pop_template.rs:499 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_bundle_check.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_entropy_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_NET_OFFLINE (test: 1)

- test: integration_tests/tests/taira_public_localnet.rs:3074 — `let cargo_net_offline = std::env::var("CARGO_NET_OFFLINE")`

## CARGO_PKG_NAME (prod: 3, test: 1)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:705 — `crate_name: env!("CARGO_PKG_NAME"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:720 — `crate_name: env!("CARGO_PKG_NAME").to_owned(),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:787 — `|| identity.crate_name != env!("CARGO_PKG_NAME")`
- test: crates/norito_derive/tests/ui.rs:23 — `let crate_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "norito_derive".to_owned());`

## CARGO_PKG_VERSION (prod: 20, test: 3, tool: 2)

- prod: crates/iroha/src/client.rs:5873 — `map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_cli/src/commands/sorafs.rs:5768 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_cli/src/main_shared.rs:403 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- prod: crates/iroha_cli/src/main_shared.rs:1218 — `let client_version = env!("CARGO_PKG_VERSION");`
- test: crates/iroha_cli/src/main_shared.rs:10060 — `&[("version", env!("CARGO_PKG_VERSION"))],`
- test: crates/iroha_cli/tests/cli_smoke.rs:600 — `let expected_version = env!("CARGO_PKG_VERSION");`
- prod: crates/iroha_core/src/bin/pk2_bridge_finality_verify.rs:1280 — `let mut preimage = env!("CARGO_PKG_VERSION").as_bytes().to_vec();`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1840 — `env!("CARGO_PKG_VERSION"),`
- prod: crates/iroha_core/src/sumeragi/v2_runner.rs:5076 — `let mut build_preimage = env!("CARGO_PKG_VERSION").as_bytes().to_vec();`
- prod: crates/iroha_js_host/src/lib.rs:4933 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_kagami/src/genesis/generate.rs:483 — `env!("CARGO_PKG_VERSION")`
- prod: crates/iroha_kagami/src/verify.rs:83 — `writeln!(writer, "kagami_version: {}", env!("CARGO_PKG_VERSION"))?;`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:706 — `crate_version: env!("CARGO_PKG_VERSION"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:721 — `crate_version: env!("CARGO_PKG_VERSION").to_owned(),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:788 — `|| identity.crate_version != env!("CARGO_PKG_VERSION")`
- test: crates/iroha_telemetry/src/metrics.rs:5632 — `version: env!("CARGO_PKG_VERSION").to_owned(),`
- prod: crates/iroha_telemetry/src/ws.rs:267 — `env!("CARGO_PKG_VERSION")`
- prod: crates/irohad/src/main.rs:1227 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/irohad/src/main.rs:16389 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/kotodama_lang/src/compiler.rs:121 — `const COMPILER_FINGERPRINT: &str = concat!("kotodama_lang/", env!("CARGO_PKG_VERSION"));`
- prod: crates/musubi/src/command.rs:115 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/sorafs_car/src/bin/sorafs_fetch.rs:1164 — `Value::from(env!("CARGO_PKG_VERSION")),`
- prod: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:139 — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/sora-vpn-helper/src/main.rs:75 — `const VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/telemetry-schema-diff/src/main.rs:252 — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## CARGO_PRIMARY_PACKAGE (build: 1)

- build: crates/soranet_pq/build.rs:6 — `if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {`

## CARGO_TARGET_DIR (prod: 5, test: 7, tool: 2)

- prod: crates/iroha_kagami/src/localnet.rs:4428 — `let target_dir = resolve_target_dir(&repo_root, env::var("CARGO_TARGET_DIR").ok().as_deref());`
- test: crates/iroha_test_network/src/lib.rs:1169 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: crates/iroha_test_network/src/lib.rs:2176 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/binary_resolver.rs:81 — `if let Some(target_root) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)`
- test: integration_tests/src/binary_resolver.rs:234 — `if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs:110 — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs:138 — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/tests/taira_public_localnet.rs:3192 — `let target = std::env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:935 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:1446 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:1500 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:1554 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- tool: xtask/src/kagami_profiles.rs:1219 — `if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {`
- tool: xtask/src/mochi.rs:427 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## CONNECT_NORITO_SOURCE_REVISION (build: 1)

- build: crates/connect_norito_bridge/build.rs:47 — `env::var("CONNECT_NORITO_SOURCE_REVISION").unwrap_or_else(|_| "unknown".to_owned());`

## CREDENTIALS_DIRECTORY (prod: 1)

- prod: crates/irohad/src/bin/sorafs_external_software_signer.rs:92 — `let directory = env::var_os("CREDENTIALS_DIRECTORY")`

## CRYPTO_SM_INTRINSICS (bench: 1)

- bench: crates/iroha_crypto/benches/sm_perf.rs:191 — `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`

## CUDA_HOME (build: 5)

- build: crates/fastpq_prover/build.rs:215 — `env::var_os("CUDA_HOME")`
- build: crates/gpuzstd_cuda/build.rs:94 — `for root in env::var_os("CUDA_HOME")`
- build: crates/gpuzstd_cuda/build.rs:116 — `let root = env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:100 — `for root in env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:122 — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (build: 5)

- build: crates/fastpq_prover/build.rs:216 — `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/gpuzstd_cuda/build.rs:96 — `.chain(env::var_os("CUDA_PATH"))`
- build: crates/gpuzstd_cuda/build.rs:117 — `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:102 — `.chain(env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:123 — `.or_else(|| env::var_os("CUDA_PATH"))`

## CXX (build: 4)

- build: crates/fastpq_prover/build.rs:264 — `env::var_os("CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs:149 — `env::var_os("CXX").is_some()`
- build: crates/ivm/build.rs:445 — `env::var_os("CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:157 — `env::var_os("CXX").is_some()`

## DATASPACE_ADVERSARIAL_ARTIFACT_DIR (test: 1)

- test: integration_tests/tests/nexus/cross_lane.rs:1035 — `if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {`

## DOCS_RS (build: 1)

- build: crates/norito/build.rs:6 — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (bench: 1)

- bench: crates/norito/benches/enum_packed_bench.rs:75 — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_METAL_LIB (prod: 2)

- prod: crates/fastpq_prover/src/backend.rs:891 — `option_env!("FASTPQ_METAL_LIB")`
- prod: crates/fastpq_prover/src/metal.rs:2680 — `option_env!("FASTPQ_METAL_LIB")`

## FASTPQ_SKIP_GPU_BUILD (build: 1)

- build: crates/fastpq_prover/build.rs:47 — `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## FASTPQ_UPDATE_FIXTURES (test: 1)

- test: crates/fastpq_prover/tests/common/mod.rs:14 — `fixture_update_requested_from(std::env::var_os("FASTPQ_UPDATE_FIXTURES").as_deref())`

## GENESIS_DEBUG_MODE (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs:19 — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## GENESIS_DEBUG_PAYLOAD (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs:149 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## GITHUB_STEP_SUMMARY (prod: 2)

- prod: crates/iroha_crypto/src/bin/gost_perf_check.rs:32 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:205 — `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## GIT_COMMIT_HASH (prod: 3)

- prod: crates/iroha_core/src/bin/pk2_bridge_finality_verify.rs:1282 — `option_env!("GIT_COMMIT_HASH")`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1841 — `option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),`
- prod: crates/iroha_core/src/sumeragi/v2_runner.rs:5078 — `option_env!("GIT_COMMIT_HASH")`

## GPUZSTD_CUDA_ARCH (build: 1)

- build: crates/gpuzstd_cuda/build.rs:50 — `if let Some(arch_flag) = env::var_os("GPUZSTD_CUDA_ARCH") {`

## GPUZSTD_CUDA_REQUIRE (test: 3)

- test: crates/gpuzstd_cuda/src/lib.rs:325 — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some() {`
- test: crates/gpuzstd_cuda/src/lib.rs:756 — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_none() {`
- test: crates/norito/src/core/gpu_zstd.rs:723 — `std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some()`

## GPUZSTD_CUDA_SKIP_BUILD (build: 1)

- build: crates/gpuzstd_cuda/build.rs:20 — `if env::var_os("GPUZSTD_CUDA_SKIP_BUILD").is_some() {`

## HOME (prod: 6, tool: 1)

- prod: crates/iroha/src/config.rs:57 — `env::var_os("HOME").map(PathBuf::from)`
- prod: crates/irohad/src/soracloud_runtime.rs:15392 — `if let Some(home) = std::env::var_os("HOME") {`
- prod: crates/musubi/src/cache.rs:175 — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/musubi/src/cache.rs:192 — `std::env::var_os("HOME").map(PathBuf::from),`
- prod: crates/musubi/src/command.rs:3328 — `let root = std::env::var_os("HOME").map(PathBuf::from).map(|path| {`
- prod: crates/musubi/src/command.rs:3340 — `std::env::var_os("HOME")`
- tool: tools/sora-vpn-helper/src/main.rs:2416 — `if let Some(home) = env::var_os("HOME") {`

## HOST_CXX (build: 4)

- build: crates/fastpq_prover/build.rs:265 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs:150 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/ivm/build.rs:446 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:158 — `|| env::var_os("HOST_CXX").is_some()`

## IROHA_ACCOUNT_ID (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:267 — `account_id: std::env::var("IROHA_ACCOUNT_ID").ok(),`

## IROHA_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs:7851 — `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## IROHA_API_BASE (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:260 — `api_base: std::env::var("IROHA_API_BASE")`

## IROHA_CHAIN_ID (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:265 — `chain_id: std::env::var("IROHA_CHAIN_ID")`

## IROHA_CONF_GAS_SEED (test: 1)

- test: crates/iroha_test_samples/src/lib.rs:78 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_DA_SPOOL_DIR (test: 1)

- test: crates/iroha_core/src/state.rs:29179 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_DEBUG_GENESIS_PATH (test: 3)

- test: crates/iroha_genesis/src/lib.rs:4262 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/lib.rs:4323 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/lib.rs:4354 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`

## IROHA_DEBUG_SIGNED_GENESIS_PATH (test: 1)

- test: crates/iroha_genesis/src/lib.rs:4291 — `let path = env::var("IROHA_DEBUG_SIGNED_GENESIS_PATH")`

## IROHA_DPN_VALIDATOR_RELEASE_COMMIT (test: 1)

- test: crates/iroha_telemetry/src/metrics.rs:5636 — `dpn_validator_release_commit: option_env!("IROHA_DPN_VALIDATOR_RELEASE_COMMIT")`

## IROHA_DUMP_MANIFEST_JSON (test: 1)

- test: crates/iroha_data_model/src/nexus/manifest.rs:1245 — `if std::env::var_os("IROHA_DUMP_MANIFEST_JSON").is_some() {`

## IROHA_GENESIS_FILE (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs:21 — `let genesis_path = std::env::var("IROHA_GENESIS_FILE")`

## IROHA_GENESIS_PUBLIC_KEY (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs:23 — `let pub_key_str = std::env::var("IROHA_GENESIS_PUBLIC_KEY").unwrap_or_else(|_| {`

## IROHA_GIT_COMMIT_HASH (build: 1, prod: 6, test: 1)

- build: crates/iroha_core/build.rs:139 — `let commit = env::var("IROHA_GIT_COMMIT_HASH").ok()?;`
- prod: crates/iroha_core/src/bin/pk2_bridge_finality_verify.rs:58 — `const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");`
- prod: crates/iroha_core/src/bin/pk2_bridge_finality_verify.rs:2107 — `assert_eq!(embedded, option_env!("IROHA_GIT_COMMIT_HASH"));`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1842 — `option_env!("IROHA_GIT_COMMIT_HASH").unwrap_or("unknown"),`
- prod: crates/iroha_js_host/src/lib.rs:851 — `option_env!("IROHA_GIT_COMMIT_HASH")`
- test: crates/iroha_js_host/src/lib.rs:19716 — `option_env!("IROHA_GIT_COMMIT_HASH").unwrap_or("unknown")`
- prod: crates/iroha_kagami/src/main.rs:50 — `const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");`
- prod: crates/irohad/src/main.rs:164 — `const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");`

## IROHA_INROU_LINUX_KVM (test: 3)

- test: crates/irohad/src/soracloud_runtime.rs:19263 — `if std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:28901 — `|| std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:29062 — `|| std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1")`

## IROHA_INROU_LINUX_KVM_INITRD_IMAGE (test: 2)

- test: crates/irohad/src/soracloud_runtime.rs:28912 — `let initrd_image = std::env::var("IROHA_INROU_LINUX_KVM_INITRD_IMAGE")`
- test: crates/irohad/src/soracloud_runtime.rs:29073 — `let initrd_image = std::env::var("IROHA_INROU_LINUX_KVM_INITRD_IMAGE")`

## IROHA_INROU_PORTABLE (test: 3)

- test: crates/irohad/src/soracloud_runtime.rs:19325 — `if std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:28523 — `|| std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:28709 — `|| std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")`

## IROHA_INROU_PORTABLE_ACCEL (prod: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:678 — `portable_vm_accel_from(std::env::var("IROHA_INROU_PORTABLE_ACCEL").ok().as_deref())`

## IROHA_INROU_PORTABLE_INITRD_IMAGE (test: 1, tool: 1)

- test: crates/irohad/src/soracloud_runtime.rs:28534 — `let initrd_image = std::env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`
- tool: xtask/src/soracloud_inrou.rs:120 — `if let Ok(value) = env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`

## IROHA_INROU_PORTABLE_SMOKE_ENTRYPOINT (test: 1)

- test: crates/irohad/src/soracloud_runtime.rs:28720 — `let external_entrypoint = std::env::var("IROHA_INROU_PORTABLE_SMOKE_ENTRYPOINT")`

## IROHA_INROU_PORTABLE_SMOKE_HEALTHCHECK (test: 1)

- test: crates/irohad/src/soracloud_runtime.rs:28722 — `let external_healthcheck = std::env::var("IROHA_INROU_PORTABLE_SMOKE_HEALTHCHECK")`

## IROHA_KAGAMI_LOCALNET_KEEP (test: 1)

- test: integration_tests/tests/sumeragi_kagami_localnet.rs:82 — `if std::env::var_os("IROHA_KAGAMI_LOCALNET_KEEP").is_some() {`

## IROHA_MCP_URL (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:264 — `mcp_url: std::env::var("IROHA_MCP_URL").ok().or_else(|| {mcp_url}),`

## IROHA_METRICS_PANIC_ON_DUPLICATE (test: 2)

- test: crates/iroha_telemetry/src/metrics.rs:16672 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- test: crates/iroha_torii/tests/metrics_registry.rs:34 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## IROHA_PRIVATE_KEY (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:268 — `private_key: std::env::var("IROHA_PRIVATE_KEY").ok(),`

## IROHA_REALISTIC_30TPS_LOAD_KIND (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:552 — `let Some(raw) = std::env::var("IROHA_REALISTIC_30TPS_LOAD_KIND")`

## IROHA_REALISTIC_30TPS_LOG_LEVEL (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2911 — `std::env::var("IROHA_REALISTIC_30TPS_LOG_LEVEL").unwrap_or_else(|_| "WARN".into());`

## IROHA_RELEASE_SOURCE_MANIFEST_SHA256 (test: 1)

- test: integration_tests/tests/taira_public_localnet.rs:3058 — `let manifest = std::env::var("IROHA_RELEASE_SOURCE_MANIFEST_SHA256")`

## IROHA_RUN_IGNORED (test: 41)

- test: crates/iroha_core/tests/check_genesis_sig.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_finalize_real_vk.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_zk_ballot_lock_verified.rs:11 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_zk_ballot_real_vk.rs:11 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/zk_roots_get_cap.rs:45 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/zk_vote_get_tally.rs:49 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:36 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:57 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:83 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:571 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:992 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1075 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1193 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1337 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1463 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1589 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1724 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1826 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1959 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:2119 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_enact_handler.rs:23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_mode_mismatch_and_autoclose.rs:44 — `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints_router.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_read_endpoints_router.rs:42 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:19257 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:19319 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:28522 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:28708 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:28900 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:29061 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/ivm/tests/beep_test.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/kotodama_struct_fields.rs:13 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:50 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:303 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:483 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:561 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/pipeline_block_rejected.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/sorting.rs:46 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_RUN_ZK_WRAPPERS (test: 1)

- test: crates/ivm/tests/kotodama_wrappers.rs:4 — `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`

## IROHA_SCCP_BUILD_FEATURES (prod: 1)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:632 — `env!("IROHA_SCCP_BUILD_FEATURES")`

## IROHA_SCCP_BUILD_PROFILE (prod: 2)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:708 — `build_profile: env!("IROHA_SCCP_BUILD_PROFILE"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:723 — `build_profile: env!("IROHA_SCCP_BUILD_PROFILE").to_owned(),`

## IROHA_SCCP_BUILD_TARGET (prod: 2)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:709 — `target_triple: env!("IROHA_SCCP_BUILD_TARGET"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:724 — `target_triple: env!("IROHA_SCCP_BUILD_TARGET").to_owned(),`

## IROHA_SCCP_RUSTC_VERSION (prod: 2)

- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:710 — `rustc_version: env!("IROHA_SCCP_RUSTC_VERSION"),`
- prod: crates/iroha_sccp/src/bin/sccp_release_evidence.rs:725 — `rustc_version: env!("IROHA_SCCP_RUSTC_VERSION").to_owned(),`

## IROHA_SKIP_BIND_CHECKS (test: 1)

- test: crates/iroha_test_network/src/lib.rs:8132 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (test: 1)

- test: crates/iroha_crypto/tests/sm_cli_matrix.rs:47 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_STARTUP_TRACE (prod: 1)

- prod: crates/irohad/src/main.rs:167 — `env::var_os("IROHA_STARTUP_TRACE").is_some()`

## IROHA_SWIFT_UNSHIELD_ATTACHMENT_PATH (test: 1)

- test: crates/iroha_core/tests/swift_confidential_unshield_redeem.rs:21 — `let Ok(path) = env::var("IROHA_SWIFT_UNSHIELD_ATTACHMENT_PATH") else {`

## IROHA_TAIRA_EVIDENCE_PATH (test: 1)

- test: integration_tests/tests/taira_public_localnet.rs:3103 — `let raw = std::env::var("IROHA_TAIRA_EVIDENCE_PATH")`

## IROHA_TAIRA_KEEP_LOCALNET (test: 2)

- test: integration_tests/tests/taira_public_localnet.rs:3148 — `if std::env::var_os("IROHA_TAIRA_KEEP_LOCALNET").is_some() {`
- test: integration_tests/tests/taira_public_localnet.rs:3157 — `if std::env::var_os("IROHA_TAIRA_KEEP_LOCALNET").is_some() {`

## IROHA_TAIRA_SIM_SEED (test: 1)

- test: integration_tests/tests/taira_public_localnet.rs:723 — `let seed = std::env::var("IROHA_TAIRA_SIM_SEED")`

## IROHA_TEST_BUILD_PROFILE (test: 2)

- test: integration_tests/src/binary_resolver.rs:180 — `std::env::var("IROHA_TEST_BUILD_PROFILE").ok().as_deref(),`
- test: integration_tests/tests/taira_public_localnet.rs:3070 — `let build_profile = std::env::var("IROHA_TEST_BUILD_PROFILE")`

## IROHA_TEST_CLIENT_TTL_MS (test: 5)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2890 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4139 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4400 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4676 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:5350 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`

## IROHA_TEST_DUMP_GENESIS (test: 1)

- test: crates/iroha_test_network/src/lib.rs:15459 — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`

## IROHA_TEST_NETWORK_PARALLELISM (test: 1)

- test: integration_tests/tests/address_canonicalisation.rs:48 — `if let Ok(raw) = env::var("IROHA_TEST_NETWORK_PARALLELISM")`

## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (build: 1, test: 1)

- test: crates/iroha_test_network/src/config.rs:511 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- build: integration_tests/build.rs:76 — `if env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_SERIALIZE_NETWORKS (test: 2)

- test: integration_tests/tests/address_canonicalisation.rs:43 — `if let Ok(raw) = env::var("IROHA_TEST_SERIALIZE_NETWORKS")`
- test: integration_tests/tests/asset.rs:270 — `if std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none() {`

## IROHA_TEST_SKIP_BUILD (test: 1)

- test: integration_tests/src/binary_resolver.rs:51 — `std::env::var("IROHA_TEST_SKIP_BUILD").ok().as_deref(),`

## IROHA_TEST_USE_DEFAULT_EXECUTOR (prod: 1, test: 1)

- test: crates/iroha_core/src/executor.rs:19144 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- prod: crates/iroha_core/src/executor_contract_dispatch_tests.rs:286 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`

## IROHA_THROUGHPUT_ARTIFACT_DIR (test: 3)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3740 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:5296 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:5936 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`

## IROHA_THROUGHPUT_DELAY_MS (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:501 — `if let Ok(delay) = std::env::var("IROHA_THROUGHPUT_DELAY_MS") {`

## IROHA_THROUGHPUT_RBC_ENCODING (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4682 — `std::env::var("IROHA_THROUGHPUT_RBC_ENCODING").unwrap_or_else(|_| "plain".to_owned());`

## IROHA_TORII_ALLOW_LIVE_ASSET_HOLDER_AGGREGATE (test: 1)

- test: crates/iroha_torii/src/routing.rs:71681 — `|| std::env::var("IROHA_TORII_ALLOW_LIVE_ASSET_HOLDER_AGGREGATE")`

## IROHA_TORII_LOCAL_READ_FANOUT_COORDINATOR (prod: 1)

- prod: crates/iroha_torii/src/lib.rs:1963 — `std::env::var("IROHA_TORII_LOCAL_READ_FANOUT_COORDINATOR")`

## IROHA_TORII_OPENAPI_ACTUAL (test: 1)

- test: crates/iroha_torii/tests/router_feature_matrix.rs:88 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_EXPECTED (test: 2)

- test: crates/iroha_torii/tests/router_feature_matrix.rs:82 — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- test: crates/iroha_torii/tests/router_feature_matrix.rs:98 — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_TORII_OPENAPI_TOKENS (tool: 2)

- tool: xtask/src/main.rs:14261 — `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`
- tool: xtask/src/main.rs:14314 — `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IROHA_TORII_PUBLIC_DATASPACE_UPSTREAMS (prod: 1)

- prod: crates/iroha_torii/src/lib.rs:1922 — `let Ok(raw) = std::env::var("IROHA_TORII_PUBLIC_DATASPACE_UPSTREAMS") else {`

## IROHA_TORII_URL (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:262 — `torii_url: std::env::var("IROHA_TORII_URL")`

## IVM_BIN (test: 2)

- test: integration_tests/tests/kotodama_examples.rs:64 — `let ivm_bin = env::var("IVM_BIN")`
- test: integration_tests/tests/kotodama_examples.rs:160 — `let ivm_bin = env::var("IVM_BIN")`

## IVM_COMPILER_DEBUG (test: 1)

- test: crates/kotodama_lang/src/compiler.rs:20210 — `if cfg!(any(test, debug_assertions)) && std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_GENCODE (build: 1)

- build: crates/ivm/build.rs:302 — `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| DEFAULT_CUDA_GENCODE.to_string());`

## IVM_CUDA_NVCC (build: 1)

- build: crates/ivm/build.rs:296 — `let executable = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVCC_EXTRA (build: 1)

- build: crates/ivm/build.rs:303 — `let extra_flags = env::var("IVM_CUDA_NVCC_EXTRA")`

## IVM_CUDA_PTX_MODE (build: 1)

- build: crates/ivm/build.rs:269 — `match env::var("IVM_CUDA_PTX_MODE") {`

## IVM_CUDA_SELFTEST_TRACE (prod: 1)

- prod: crates/ivm/src/cuda.rs:115 — `if std::env::var_os("IVM_CUDA_SELFTEST_TRACE").is_some() {`

## IVM_DEBUG_AED_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:379 — `let aed_asset_raw = std::env::var("IVM_DEBUG_AED_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:244 — `let asset_raw = std::env::var("IVM_DEBUG_ASSET_DEFINITION")`

## IVM_DEBUG_CBDC_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:385 — `let cbdc_asset_raw = std::env::var("IVM_DEBUG_CBDC_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_DOMAIN (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:246 — `let domain_raw = std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank".to_owned());`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:392 — `std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank.universal".to_owned());`

## IVM_DEBUG_FROM_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:242 — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:377 — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`

## IVM_DEBUG_METAL_ENUM (debug: 1)

- debug: crates/ivm/src/vector.rs:475 — `std::env::var("IVM_DEBUG_METAL_ENUM")`

## IVM_DEBUG_METAL_SELFTEST (debug: 1)

- debug: crates/ivm/src/vector.rs:1204 — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DEBUG_RATIO (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:393 — `let ratio_raw = std::env::var("IVM_DEBUG_RATIO").unwrap_or_else(|_| "76".to_owned());`

## IVM_DEBUG_TO_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:243 — `let to_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:378 — `let dst_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`

## IVM_DISABLE_CUDA (debug: 1, test: 2)

- debug: crates/ivm/src/cuda.rs:880 — `&& std::env::var("IVM_DISABLE_CUDA")`
- test: crates/ivm/tests/cuda_disable_on_mismatch.rs:47 — `original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),`
- test: crates/ivm/tests/cuda_env.rs:30 — `original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),`

## IVM_DISABLE_METAL (debug: 1)

- debug: crates/ivm/src/vector.rs:301 — `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELFTEST_FAIL (debug: 1, test: 2)

- debug: crates/ivm/src/cuda.rs:891 — `&& std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL")`
- test: crates/ivm/tests/cuda_disable_on_mismatch.rs:46 — `original_force_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),`
- test: crates/ivm/tests/cuda_env.rs:31 — `original_force_selftest_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),`

## IVM_FORCE_METAL_ENUM (debug: 1)

- debug: crates/ivm/src/vector.rs:447 — `std::env::var("IVM_FORCE_METAL_ENUM")`

## IVM_FORCE_METAL_SELFTEST_FAIL (debug: 1)

- debug: crates/ivm/src/vector.rs:1191 — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IVM_TOOL_BIN (test: 1)

- test: integration_tests/tests/kotodama_examples.rs:111 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## IZANAMI_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs:7850 — `std::env::var("IZANAMI_ALLOW_NET")`

## IZANAMI_TUI_ALLOW_ZERO_SEED (prod: 1)

- prod: crates/izanami/src/tui.rs:188 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:52 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_REQUIRE (test: 3)

- test: crates/norito/accelerators/jsonstage1_cuda/src/lib.rs:421 — `std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some()`
- test: crates/norito/src/core.rs:2735 — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`
- test: crates/norito/src/lib.rs:6198 — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`

## JSONSTAGE1_CUDA_SKIP_BUILD (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:23 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE (build: 1, prod: 1)

- build: crates/iroha_core/build.rs:37 — `env::var("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE").unwrap_or_else(|_| {`
- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:230 — `option_env!("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE");`

## KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256 (prod: 2)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:232 — `option_env!("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256");`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1845 — `option_env!("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256").unwrap_or("unknown"),`

## KAGEMUSHA_BUILD_SOURCE_COMMIT (prod: 2)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:227 — `const BUILD_SOURCE_COMMIT: Option<&str> = option_env!("KAGEMUSHA_BUILD_SOURCE_COMMIT");`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1843 — `option_env!("KAGEMUSHA_BUILD_SOURCE_COMMIT").unwrap_or("unknown"),`

## KAGEMUSHA_BUILD_SOURCE_TREE_SHA256 (prod: 2)

- prod: crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs:228 — `const BUILD_SOURCE_TREE_SHA256: Option<&str> = option_env!("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256");`
- prod: crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs:1844 — `option_env!("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256").unwrap_or("unknown"),`

## KAGEMUSHA_SOURCE_SEAL_PYTHON (build: 1)

- build: crates/iroha_core/build.rs:54 — `let python = env::var("KAGEMUSHA_SOURCE_SEAL_PYTHON").unwrap_or_else(|_| "python3".to_owned());`

## KOTO_BIN (test: 2)

- test: integration_tests/tests/kotodama_examples.rs:54 — `let koto_bin = env::var("KOTO_BIN")`
- test: integration_tests/tests/kotodama_examples.rs:151 — `let koto_bin = env::var("KOTO_BIN")`

## LANG (test: 2)

- test: crates/ivm/tests/i18n.rs:14 — `let old_lang = env::var("LANG").ok();`
- test: crates/ivm/tests/i18n.rs:72 — `let old_lang = env::var("LANG").ok();`

## LC_ALL (test: 2)

- test: crates/ivm/tests/i18n.rs:15 — `let old_lc_all = env::var("LC_ALL").ok();`
- test: crates/ivm/tests/i18n.rs:73 — `let old_lc_all = env::var("LC_ALL").ok();`

## LC_MESSAGES (test: 2)

- test: crates/ivm/tests/i18n.rs:16 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`
- test: crates/ivm/tests/i18n.rs:74 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## LOCALAPPDATA (prod: 2)

- prod: crates/musubi/src/cache.rs:182 — `std::env::var_os("LOCALAPPDATA").map(PathBuf::from),`
- prod: crates/musubi/src/command.rs:3323 — `let root = std::env::var_os("LOCALAPPDATA")`

## MOCHI_CONFIG (prod: 1)

- prod: mochi/mochi-ui-egui/src/config.rs:484 — `if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## MOCHI_DATA_ROOT (prod: 1)

- prod: mochi/mochi-core/src/supervisor.rs:5539 — `std::env::var_os("MOCHI_DATA_ROOT")`

## MOCHI_DETACHED (prod: 1)

- prod: mochi/mochi-ui-egui/src/sandbox_cli.rs:582 — `if env::var_os("MOCHI_DETACHED").is_some() {`

## MOCHI_TEST_USE_INTERNAL_GENESIS (prod: 1)

- prod: mochi/mochi-core/src/supervisor.rs:5196 — `if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {`

## NORITO_CHECK_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs:17 — `if env::var_os("NORITO_CHECK_BINDINGS_SYNC").is_none() {`

## NORITO_CPU_INFO (tool: 1)

- tool: xtask/src/stage1_bench.rs:74 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_CUDA_REQUIRE (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:1247 — `if std::env::var_os("NORITO_CRC64_CUDA_REQUIRE").is_none() {`

## NORITO_CRC64_GPU_LIB (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:314 — `let raw = std::env::var_os("NORITO_CRC64_GPU_LIB")?;`

## NORITO_DISABLE_PACKED_STRUCT (test: 1)

- test: crates/norito/src/lib.rs:379 — `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## NORITO_GPU_CRC64_MIN_BYTES (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:68 — `let configured = std::env::var("NORITO_GPU_CRC64_MIN_BYTES")`

## NORITO_PAR_STAGE1_MIN (test: 1)

- test: crates/norito/src/lib.rs:6470 — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## NORITO_SKIP_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs:13 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_GPU_MIN_BYTES (test: 1)

- test: crates/norito/src/lib.rs:6504 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## NORITO_TRACE (test: 3)

- test: crates/norito/src/lib.rs:157 — `std::env::var_os("NORITO_TRACE").is_some()`
- test: crates/norito/src/lib.rs:162 — `*ENABLED.get_or_init(|| std::env::var_os("NORITO_TRACE").is_some())`
- test: crates/norito/src/lib.rs:178 — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## NOTIFY_SOCKET (test: 1)

- test: crates/irohad/src/runtime_provider_broker/launcher.rs:665 — `let notify_socket = std::env::var_os("NOTIFY_SOCKET")`

## NVCC (build: 1)

- build: crates/ivm/build.rs:297 — `.or_else(|_| env::var("NVCC"))`

## OUT_DIR (build: 7, prod: 14, test: 3)

- build: crates/connect_norito_bridge/build.rs:22 — `let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));`
- build: crates/fastpq_prover/build.rs:109 — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
- build: crates/iroha_data_model/build.rs:13 — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));`
- prod: crates/iroha_data_model/src/lib.rs:228 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
- build: crates/ivm/build.rs:118 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs:189 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs:419 — `if let Some(out_dir) = env::var_os("OUT_DIR") {`
- prod: crates/ivm/src/cuda.rs:30 — `static PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- prod: crates/ivm/src/cuda.rs:31 — `static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));`
- prod: crates/ivm/src/cuda.rs:32 — `static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));`
- prod: crates/ivm/src/cuda.rs:33 — `static SHA_LEAVES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx"));`
- prod: crates/ivm/src/cuda.rs:34 — `static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));`
- prod: crates/ivm/src/cuda.rs:35 — `static SHA3_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx"));`
- prod: crates/ivm/src/cuda.rs:36 — `static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));`
- prod: crates/ivm/src/cuda.rs:37 — `static BN254_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx"));`
- prod: crates/ivm/src/cuda.rs:38 — `static SIG_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/signature.ptx"));`
- prod: crates/ivm/src/cuda.rs:39 — `static SHA_PAIRS_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx"));`
- prod: crates/ivm/src/cuda.rs:41 — `static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));`
- test: crates/ivm/src/gpu_manager.rs:377 — `static ADD_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- prod: crates/ivm/src/ivm.rs:244 — `include!(concat!(env!("OUT_DIR"), "/syscall_signatures.rs"));`
- test: crates/ivm/src/ptx_tests.rs:7 — `let out_dir = env!("OUT_DIR");`
- test: crates/ivm/tests/ptx_kernels.rs:9 — `let out_dir = env!("OUT_DIR");`
- build: crates/kotodama_lang/build.rs:207 — `let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));`
- prod: crates/kotodama_lang/src/lexer.rs:122 — `include!(concat!(env!("OUT_DIR"), "/kotodama_v1_lexical.rs"));`

## PATH (prod: 2, test: 1, tool: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:15318 — `let mut search_roots = std::env::var_os("PATH")`
- test: integration_tests/tests/kotodama_examples.rs:21 — `let path = env::var_os("PATH")?;`
- prod: mochi/mochi-core/src/supervisor.rs:1382 — `let path_var = env::var_os("PATH")?;`
- tool: tools/sora-vpn-backend/src/main.rs:1294 — `let Some(path) = env::var_os("PATH") else {`

## PATHEXT (prod: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:15350 — `std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());`

## PRINT_SORACLES_FIXTURES (test: 1)

- test: crates/iroha_data_model/src/oracle/mod.rs:3635 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORII_SPEC (test: 1)

- test: crates/iroha_torii/src/openapi.rs:32957 — `if std::env::var("PRINT_TORII_SPEC").is_ok() {`

## PROFILE (build: 2, test: 4)

- build: crates/iroha_sccp/build.rs:29 — `let profile = env::var("PROFILE").expect("Cargo must provide the exact build profile");`
- test: crates/iroha_test_network/src/lib.rs:1871 — `if let Ok(profile) = std::env::var("PROFILE") {`
- build: integration_tests/build.rs:64 — `let profile = if env::var("PROFILE").ok().as_deref() == Some("release") {`
- test: integration_tests/src/binary_resolver.rs:247 — `if let Ok(profile) = std::env::var("PROFILE")`
- test: integration_tests/src/kagami.rs:76 — `let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());`
- test: integration_tests/tests/taira_public_localnet.rs:3073 — `std::env::var("PROFILE").wrap_err("PROFILE must be set by the release launcher")?;`

## PYTHON3 (test: 1)

- test: crates/sorafs_car/tests/taikai_viewer_cli.rs:30 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONPATH (test: 1)

- test: crates/iroha_cli/tests/cli_smoke.rs:5551 — `match env::var("PYTHONPATH") {`

## REPO_PROOF_DIGEST_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2815 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2803 — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUSTC (build: 1)

- build: crates/iroha_sccp/build.rs:17 — `let rustc = env::var_os("RUSTC").expect("Cargo must provide RUSTC to the SCCP build script");`

## RUST_LOG (prod: 2, test: 4)

- test: crates/iroha_test_network/src/lib.rs:12733 — `let original = env::var("RUST_LOG").ok();`
- test: crates/iroha_test_network/src/lib.rs:12750 — `let original = env::var("RUST_LOG").ok();`
- prod: crates/izanami/src/chaos.rs:2438 — `if let Ok(filter) = std::env::var("RUST_LOG") {`
- prod: crates/izanami/src/config.rs:652 — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`
- test: integration_tests/tests/sumeragi_kagami_localnet.rs:145 — `if std::env::var_os("RUST_LOG").is_none() {`
- test: integration_tests/tests/taira_public_localnet.rs:583 — `if std::env::var_os("RUST_LOG").is_none() {`

## RUST_MIN_STACK (prod: 1)

- prod: crates/iroha_core/src/privacy_release_evidence/tests.rs:789 — `std::env::var("RUST_MIN_STACK").as_deref(),`

## SM_PERF_CPU_LABEL (prod: 2)

- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:712 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:754 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## SORAFS_NODE_SKIP_INGEST_TESTS (test: 1)

- test: crates/sorafs_node/tests/cli.rs:19 — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORAFS_TORII_SKIP_INGEST_TESTS (test: 1)

- test: crates/iroha_torii/tests/sorafs_discovery.rs:126 — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORANET_VPN_INTERFACE (test: 1, tool: 1)

- tool: tools/sora-vpn-helper/src/main.rs:2448 — `env::var("SORANET_VPN_INTERFACE")`
- test: tools/sora-vpn-helper/src/main.rs:3372 — `let original = env::var_os("SORANET_VPN_INTERFACE");`

## SORANET_VPN_STATE_FILE (tool: 1)

- tool: tools/sora-vpn-helper/src/main.rs:2407 — `env::var("SORANET_VPN_STATE_FILE")`

## SUMERAGI_ADVERSARIAL_ARTIFACT_DIR (test: 1)

- test: integration_tests/tests/sumeragi_adversarial.rs:2343 — `let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {`

## SUMERAGI_BASELINE_ARTIFACT_DIR (prod: 1, test: 1)

- prod: crates/build-support/src/bin/sumeragi_baseline_report.rs:42 — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`
- test: integration_tests/tests/sumeragi_npos_performance.rs:1206 — `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`

## SUMERAGI_DA_ARTIFACT_DIR (prod: 1)

- prod: crates/build-support/src/bin/sumeragi_da_report.rs:43 — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`

## SystemRoot (prod: 1)

- prod: crates/fastpq_prover/src/backend.rs:581 — `env::var_os("SystemRoot").map(PathBuf::from)`

## TARGET (build: 2, prod: 1, tool: 2)

- prod: crates/build-support/src/lib.rs:30 — `let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());`
- build: crates/iroha_sccp/build.rs:28 — `let target = env::var("TARGET").expect("Cargo must provide the exact target triple");`
- build: crates/ivm/build.rs:28 — `if let Ok(target) = env::var("TARGET") {`
- tool: xtask/src/poseidon_bench.rs:88 — `target: std::env::var("TARGET")`
- tool: xtask/src/stage1_bench.rs:68 — `target: std::env::var("TARGET")`

## TEST_LOG_FILTER (prod: 1)

- prod: crates/iroha_logger/src/lib.rs:131 — `filter: std::env::var("TEST_LOG_FILTER")`

## TEST_LOG_LEVEL (prod: 1)

- prod: crates/iroha_logger/src/lib.rs:127 — `level: std::env::var("TEST_LOG_LEVEL")`

## TEST_NETWORK_CARGO (test: 1)

- test: crates/iroha_test_network/src/lib.rs:2560 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TEST_NETWORK_IROHAD_FEATURES (test: 5)

- test: integration_tests/tests/nexus/cross_dataspace_zk_stark_localnet.rs:232 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/privacy_exact12_activation_network.rs:63 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/privacy_exact12_zk_x509_network.rs:107 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_ace_localnet.rs:62 — `let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_stark_network.rs:67 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`

## TORII_KAGEMUSHA_COMMANDS_PRIVATE_KEY (prod: 1)

- prod: crates/iroha_config/src/parameters/user.rs:17191 — `std::env::var("TORII_KAGEMUSHA_COMMANDS_PRIVATE_KEY")`

## TORII_MOCK_HARNESS_METRICS_PATH (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:105 — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:108 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_RETRY_TOTAL (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:296 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:111 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:103 — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## TORII_OPENAPI_TOKEN (tool: 2)

- tool: xtask/src/main.rs:14256 — `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`
- tool: xtask/src/main.rs:14310 — `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## UPDATE_FIXTURES (test: 2)

- test: crates/iroha_core/tests/pin_registry.rs:134 — `if env::var_os("UPDATE_FIXTURES").is_some() {`
- test: crates/iroha_core/tests/snapshots.rs:48 — `let update = env::var("UPDATE_FIXTURES")`

## USERPROFILE (prod: 1)

- prod: crates/iroha/src/config.rs:55 — `env::var_os("USERPROFILE").map(PathBuf::from)`

## VERGEN_CARGO_FEATURES (test: 2)

- test: crates/iroha_telemetry/src/metrics.rs:5639 — `cargo_features: option_env!("VERGEN_CARGO_FEATURES")`
- test: crates/irohad/src/main.rs:21339 — `const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {`

## VERGEN_CARGO_TARGET_TRIPLE (prod: 1, test: 1)

- test: crates/iroha_telemetry/src/metrics.rs:5642 — `target_triple: option_env!("VERGEN_CARGO_TARGET_TRIPLE")`
- prod: crates/iroha_telemetry/src/ws.rs:262 — `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");`

## VERGEN_GIT_SHA (prod: 2, test: 2)

- prod: crates/iroha_cli/src/main_shared.rs:64 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`
- test: crates/iroha_telemetry/src/metrics.rs:5633 — `git_commit_sha: option_env!("VERGEN_GIT_SHA")`
- prod: crates/iroha_telemetry/src/ws.rs:261 — `let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");`
- test: crates/irohad/src/main.rs:21334 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`

## VERIFY_BATCH (bench: 1)

- bench: crates/ivm/benches/bench_voting.rs:226 — `let verify_batch = std::env::var("VERIFY_BATCH")`

## VERIFY_EVERY (bench: 1)

- bench: crates/ivm/benches/bench_voting.rs:214 — `let verify_every: u64 = std::env::var("VERIFY_EVERY")`

## VOTERS (bench: 1)

- bench: crates/ivm/benches/bench_voting.rs:208 — `let voters: u64 = std::env::var("VOTERS")`

## XDG_CACHE_HOME (prod: 1)

- prod: crates/musubi/src/cache.rs:188 — `if let Some(root) = std::env::var_os("XDG_CACHE_HOME") {`

## XDG_RUNTIME_DIR (tool: 1)

- tool: tools/sora-vpn-helper/src/main.rs:2413 — `if let Some(runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {`

## XDG_STATE_HOME (prod: 1)

- prod: crates/musubi/src/command.rs:3336 — `let root = std::env::var_os("XDG_STATE_HOME")`

## XTASK_TEST_KAGAMI_BIN (test: 2)

- test: xtask/src/kagami_profiles.rs:1949 — `if std::env::var("XTASK_TEST_KAGAMI_BIN").is_err() {`
- test: xtask/src/kagami_profiles.rs:1952 — `let kagami_path = PathBuf::from(std::env::var("XTASK_TEST_KAGAMI_BIN").unwrap());`
