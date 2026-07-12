# Environment toggle inventory

_Last refreshed via `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`_

Total references: **714** · Unique variables: **195**

## ACTIONS_ID_TOKEN_REQUEST_TOKEN (prod: 1)

- prod: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:461 — `let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {`

## ACTIONS_ID_TOKEN_REQUEST_URL (prod: 1)

- prod: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:458 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`

## ANDROID_SDK_ROOT (prod: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:13631 — `if let Some(android_sdk_root) = std::env::var_os("ANDROID_SDK_ROOT") {`

## BLOCK_DUMP_HEIGHTS (example: 1)

- example: crates/iroha_core/examples/block_dump.rs:61 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`

## BLOCK_DUMP_SUM_ASSET (example: 1)

- example: crates/iroha_core/examples/block_dump.rs:50 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`

## BLOCK_DUMP_VERBOSE (example: 1)

- example: crates/iroha_core/examples/block_dump.rs:49 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`

## CARGO (prod: 3, test: 4)

- test: crates/iroha_test_network/src/lib.rs:1918 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- test: crates/norito_derive/tests/ui.rs:36 — `let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());`
- test: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:18 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`
- test: integration_tests/src/kagami.rs:137 — `let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());`
- prod: mochi/mochi-core/src/supervisor.rs:951 — `let cargo = env::var_os("CARGO")`
- prod: mochi/mochi-core/src/supervisor.rs:1006 — `let cargo = env::var_os("CARGO")`
- prod: mochi/mochi-core/src/supervisor.rs:1060 — `let cargo = env::var_os("CARGO")`

## CARGO_BIN_EXE_attachment_sanitizer (test: 9)

- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:98 — `let mut cmd = Command::new(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:138 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:163 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:314 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:339 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:362 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:379 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:396 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`
- test: crates/iroha_torii/tests/zk_attachments_subprocess.rs:414 — `let sanitizer_path = PathBuf::from(env!("CARGO_BIN_EXE_attachment_sanitizer"));`

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
- test: integration_tests/src/kagami.rs:47 — `if let Ok(path) = env::var("CARGO_BIN_EXE_kagami") {`

## CARGO_BIN_EXE_kagami_mock (test: 1)

- test: mochi/mochi-integration/tests/supervisor.rs:33 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## CARGO_BIN_EXE_koto (test: 3)

- test: crates/ivm/tests/cli_smoke.rs:6 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/ivm/tests/cli_smoke.rs:59 — `let bin = env!("CARGO_BIN_EXE_koto");`
- test: crates/ivm/tests/cli_smoke.rs:96 — `let bin = env!("CARGO_BIN_EXE_koto");`

## CARGO_BIN_EXE_sorafs_chunk_dump (test: 1)

- test: crates/sorafs_chunker/tests/one_gib.rs:105 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## CARGO_BIN_NAME (prod: 4)

- prod: crates/iroha_cli/src/main_shared.rs:69 — `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`
- prod: crates/iroha_cli/src/main_shared.rs:209 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- prod: crates/iroha_kagami/src/genesis/generate.rs:1883 — `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`
- prod: crates/irohad/src/main.rs:8310 — `resolve_build_line_from_env(env::var(BUILD_LINE_ENV).ok(), env!("CARGO_BIN_NAME"))`

## CARGO_BUILD_TARGET (tool: 2)

- tool: xtask/src/poseidon_bench.rs:89 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- tool: xtask/src/stage1_bench.rs:69 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_FEATURE (prod: 1)

- prod: crates/build-support/src/lib.rs:28 — `let parsed_features = env::var("CARGO_CFG_FEATURE")`

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
- build: crates/ivm/build.rs:45 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:37 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- tool: xtask/src/poseidon_bench.rs:92 — `os: std::env::var("CARGO_CFG_TARGET_OS")`
- tool: xtask/src/stage1_bench.rs:72 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## CARGO_FEATURE_CUDA (build: 2)

- build: crates/fastpq_prover/build.rs:27 — `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
- build: crates/ivm/build.rs:24 — `if env::var_os("CARGO_FEATURE_CUDA").is_some()`

## CARGO_FEATURE_CUDA_KERNEL (build: 2)

- build: crates/gpuzstd_cuda/build.rs:17 — `if env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_none() {`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:17 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (build: 1)

- build: crates/fastpq_prover/build.rs:28 — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## CARGO_FEATURE_FFI_EXPORT (prod: 1)

- prod: crates/build-support/src/lib.rs:201 — `let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();`

## CARGO_FEATURE_FFI_IMPORT (prod: 1)

- prod: crates/build-support/src/lib.rs:200 — `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`

## CARGO_MANIFEST_DIR (bench: 3, build: 4, example: 1, prod: 41, test: 231, tool: 4)

- prod: crates/build-support/src/lib.rs:84 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);`
- test: crates/connect_norito_bridge/src/lib.rs:35127 — `fs::read(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))`
- prod: crates/fastpq_prover/src/poseidon_manifest.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/fastpq_prover/tests/packing.rs:17 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:9 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- test: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:28 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- test: crates/fastpq_prover/tests/proof_fixture.rs:21 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/fastpq_prover/tests/trace_commitment.rs:23 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- test: crates/fastpq_prover/tests/transcript_replay.rs:91 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/client.rs:21179 — `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/src/sm.rs:219 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha/tests/sm_signing.rs:36 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/src/compute.rs:520 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_cli/src/main_shared.rs:1369 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/iroha_cli/src/soracloud.rs:14196 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/tests/cli_smoke.rs:174 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_cli/tests/cli_smoke.rs:6014 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_config/src/parameters/user.rs:8912 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/src/parameters/user.rs:25097 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/autoscale_config.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/fastpq_queue_overrides.rs:15 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:41 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:3164 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/fixtures.rs:4189 — `let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_config/tests/native_amx_config.rs:9 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/sccp_route_manifest_aliases.rs:12 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- test: crates/iroha_config/tests/trusted_peers_pop_validation.rs:15 — `let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");`
- bench: crates/iroha_core/benches/blocks/common.rs:302 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- bench: crates/iroha_core/benches/blocks/common/mod.rs:273 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- bench: crates/iroha_core/benches/validation.rs:101 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- build: crates/iroha_core/build.rs:30 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- example: crates/iroha_core/examples/generate_parity_fixtures.rs:19 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/src/block.rs:19496 — `let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");`
- test: crates/iroha_core/src/executor.rs:10690 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- test: crates/iroha_core/src/executor.rs:11054 — `let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/executor.rs:11367 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2321 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2325 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud.rs:25487 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud.rs:25496 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/src/smartcontracts/isi/soracloud.rs:42254 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_core/src/state.rs:29532 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- test: crates/iroha_core/src/state.rs:39901 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/src/streaming.rs:3614 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/src/tx.rs:12631 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_core/tests/executor_migration_introspect.rs:29 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/pin_registry.rs:116 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);`
- test: crates/iroha_core/tests/pin_registry.rs:1436 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/snapshots.rs:35 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_core/tests/sumeragi_doc_sync.rs:71 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/confidential_keyset_vectors.rs:57 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm2_fixture_vectors.rs:55 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_crypto/tests/sm_cli_matrix.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:27 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:31 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/axt_fixtures.rs:35 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/offline_v2_vectors.rs:36 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/offline_vectors.rs:48 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/qr_stream_fixtures.rs:13 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/bin/qr_stream_fixtures.rs:17 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/identifier.rs:839 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_data_model/src/lib.rs:213 — `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- prod: crates/iroha_data_model/src/lib.rs:217 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1017 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1127 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/nexus/manifest.rs:1137 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/src/qr_stream.rs:849 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_data_model/src/soranet/vpn.rs:2412 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- prod: crates/iroha_data_model/src/testing/axt.rs:17 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs:21 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_data_model/src/testing/axt.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/src/transaction/signed.rs:2938 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/account_address_vectors.rs:136 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/address_curve_registry.rs:33 — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:50 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/confidential_wallet_fixtures.rs:15 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_data_model/tests/consensus_roundtrip.rs:2666 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
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
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:54 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:58 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:62 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:66 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:70 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:74 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:78 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:82 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:86 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:90 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:94 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:98 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:102 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:106 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:110 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs:2503 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_genesis/src/lib.rs:1871 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_genesis/src/lib.rs:5749 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_genesis/src/lib.rs:5775 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:5834 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:5929 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_genesis/src/lib.rs:6835 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_genesis/src/lib.rs:6850 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- test: crates/iroha_i18n/src/lib.rs:496 — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- test: crates/iroha_js_host/src/lib.rs:22867 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_js_host/src/lib.rs:25111 — `let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_kagami/samples/codec/generate.rs:13 — `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- prod: crates/iroha_kagami/samples/codec/src/main.rs:35 — `let dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_kagami/src/codec.rs:407 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_kagami/src/genesis/sign.rs:1685 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_kagami/src/localnet.rs:530 — `env!("CARGO_MANIFEST_DIR"),`
- prod: crates/iroha_kagami/src/localnet.rs:3319 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_kagami/src/localnet.rs:3323 — `|| PathBuf::from(env!("CARGO_MANIFEST_DIR")),`
- test: crates/iroha_kagami/src/wizard.rs:1138 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");`
- test: crates/iroha_kagami/tests/codec.rs:14 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- test: crates/iroha_telemetry/tests/drill_log.rs:10 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/iroha_test_network/src/fslock_ports.rs:30 — `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- test: crates/iroha_test_network/src/fslock_ports.rs:32 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_test_network/src/lib.rs:794 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_test_samples/src/lib.rs:264 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_test_samples/src/lib.rs:301 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/iroha_torii/src/da/tests.rs:8601 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- test: crates/iroha_torii/src/identifier_resolution.rs:678 — `let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/offline_commands.rs:3711 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/offline_commands.rs:3719 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/offline_commands.rs:3740 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/src/soracloud.rs:14360 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs:24640 — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/sorafs/api.rs:43635 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/src/zk_attachments.rs:2114 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/tests/account_address_vectors.rs:135 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/iroha_torii/tests/accounts_portfolio.rs:104 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/iroha_torii/tests/sorafs_discovery.rs:1129 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/main.rs:7660 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/main.rs:7690 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/nexus/config.toml");`
- test: crates/irohad/src/soracloud_runtime.rs:14693 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/irohad/src/soracloud_runtime.rs:14700 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- build: crates/ivm/build.rs:33 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- prod: crates/ivm/src/bin/gen_abi_hash_doc.rs:27 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: crates/ivm/src/bin/gen_header_doc.rs:47 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: crates/ivm/src/bin/gen_pointer_types_doc.rs:22 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: crates/ivm/src/bin/gen_syscalls_doc.rs:412 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: crates/ivm/src/bin/ivm_prebuild.rs:25 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/ivm/src/bin/ivm_predecoder_export.rs:23 — `let _crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: crates/ivm/src/predecoder_fixtures.rs:205 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- test: crates/ivm/tests/axt_descriptor_builder.rs:23 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/ivm/tests/cli_smoke.rs:7 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/cli_smoke.rs:60 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/cli_smoke.rs:97 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- test: crates/ivm/tests/docs_consistency.rs:3 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/ivm_abi_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/ivm_header_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/kotodama.rs:3299 — `let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kotodama_lang/src/samples");`
- test: crates/ivm/tests/norito_portal_snippets_compile.rs:24 — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: crates/ivm/tests/pointer_types_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- test: crates/ivm/tests/pointer_types_doc_generated_ivm_md.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/ivm/tests/syscalls_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_doc_sync.rs:8 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/ivm/tests/syscalls_gas_names.rs:11 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- test: crates/kotodama_lang/src/doc_consistency.rs:8 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- build: crates/norito/build.rs:22 — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- prod: crates/norito/src/bin/norito_regen_goldens.rs:11 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/src/streaming.rs:11207 — `let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/aos_ncb_more_golden.rs:199 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- test: crates/norito/tests/json_golden_loader.rs:16 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:353 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:387 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:553 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- test: crates/norito/tests/ncb_enum_iter_samples.rs:664 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- test: crates/norito/tests/ncb_enum_large_fixture.rs:36 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs:557 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/bin/da_reconstruct.rs:871 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_car/src/bin/soranet_trustless_verifier.rs:191 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/src/reference.rs:278 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/capacity_simulation_toolkit.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:58 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:1026 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/fetch_cli.rs:1142 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/taikai_viewer_cli.rs:22 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_car/tests/trustless_verifier.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: crates/sorafs_chunker/src/bin/export_vectors.rs:169 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_chunker/tests/backpressure.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_chunker/tests/vectors.rs:10 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/bin/sorafs-validate.rs:3121 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference.rs:6374 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/src/reference_ffi.rs:1377 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/orderbook_fixtures.rs:16 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/pdp_fixtures.rs:15 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/por_fixtures.rs:12 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:19 — `cmd.current_dir(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_manifest/tests/replication_order_fixtures.rs:8 — `env!("CARGO_MANIFEST_DIR"),`
- test: crates/sorafs_manifest/tests/sorafs_validate_cli.rs:20 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_node/src/gateway.rs:2316 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");`
- test: crates/sorafs_node/src/gateway.rs:2584 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");`
- test: crates/sorafs_node/src/gateway.rs:2604 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");`
- test: crates/sorafs_node/tests/cli.rs:132 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_node/tests/gateway.rs:24 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_node/tests/gateway.rs:107 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/src/lib.rs:6364 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs:6497 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/src/lib.rs:8164 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: crates/sorafs_orchestrator/tests/orchestrator_parity.rs:181 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs:3795 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/sorafs_orchestrator/tests/sorafs_cli.rs:4900 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: crates/soranet_pq/tests/kat_vectors.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- build: integration_tests/build.rs:23 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));`
- test: integration_tests/src/bin/refresh_nexus_streaming_fixtures.rs:447 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: integration_tests/src/binary_resolver.rs:172 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")`
- test: integration_tests/src/kagami.rs:162 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/src/sorafs_gateway_capability_refusal.rs:157 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- test: integration_tests/src/sorafs_gateway_conformance.rs:1391 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/asset.rs:265 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/fast_dsl_build.rs:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/genesis_json.rs:19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/genesis_json.rs:38 — `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
- test: integration_tests/tests/iroha_cli.rs:70 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/ivm_header_decode.rs:48 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/ivm_header_smoke.rs:27 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:74 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:121 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/kotodama_examples.rs:170 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/cbdc_rollout_bundle.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/cbdc_whitelist.rs:27 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/global_commit.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/nexus/lane_registry.rs:13 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/norito_burn_fixture.rs:35 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/repo.rs:36 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: integration_tests/tests/streaming/mod.rs:442 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/compose.rs:1571 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1575 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1579 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1583 — `env!("CARGO_MANIFEST_DIR"),`
- test: mochi/mochi-core/src/compose.rs:1587 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-core/src/supervisor.rs:420 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- prod: mochi/mochi-core/src/supervisor.rs:942 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/torii.rs:6358 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/torii.rs:6373 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/torii.rs:6388 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-core/src/torii.rs:6403 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: mochi/mochi-integration/tests/supervisor.rs:165 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- prod: mochi/mochi-ui-egui/src/gui.rs:123 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:127 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:131 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:135 — `env!("CARGO_MANIFEST_DIR"),`
- prod: mochi/mochi-ui-egui/src/gui.rs:139 — `env!("CARGO_MANIFEST_DIR"),`
- test: tools/soranet-handshake-harness/tests/fixtures_verify.rs:6 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/interop_parity.rs:77 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- test: tools/soranet-handshake-harness/tests/perf_gate.rs:173 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- tool: xtask/src/bin/control_plane_mock.rs:362 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/main.rs:11573 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- tool: xtask/src/sorafs/gateway_fixture.rs:29 — `env!("CARGO_MANIFEST_DIR"),`
- tool: xtask/src/sorafs/gateway_fixture.rs:33 — `env!("CARGO_MANIFEST_DIR"),`
- test: xtask/tests/address_vectors.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/android_dashboard_parity_cli.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/codec_rans_tables.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/da_proof_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/iso_bridge_lint.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/ministry_agenda.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/sns_catalog_verify.rs:6 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
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
- test: xtask/tests/soranet_pop_template.rs:489 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_bundle_check.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- test: xtask/tests/streaming_entropy_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_NAME (test: 1)

- test: crates/norito_derive/tests/ui.rs:23 — `let crate_name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "norito_derive".to_owned());`

## CARGO_PKG_VERSION (prod: 16, test: 3, tool: 2)

- prod: crates/iroha/src/client.rs:2524 — `map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_cli/src/commands/sorafs.rs:5738 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_cli/src/main_shared.rs:209 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- prod: crates/iroha_cli/src/main_shared.rs:1006 — `let client_version = env!("CARGO_PKG_VERSION");`
- test: crates/iroha_cli/src/main_shared.rs:9129 — `&[("version", env!("CARGO_PKG_VERSION"))],`
- test: crates/iroha_cli/tests/cli_smoke.rs:648 — `let expected_version = env!("CARGO_PKG_VERSION");`
- prod: crates/iroha_core/src/sumeragi/rbc_store.rs:117 — `version: env!("CARGO_PKG_VERSION").to_owned(),`
- prod: crates/iroha_js_host/src/lib.rs:6025 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: crates/iroha_kagami/src/genesis/generate.rs:566 — `env!("CARGO_PKG_VERSION")`
- prod: crates/iroha_kagami/src/verify.rs:87 — `writeln!(writer, "kagami_version: {}", env!("CARGO_PKG_VERSION"))?;`
- test: crates/iroha_telemetry/src/metrics.rs:5499 — `version: env!("CARGO_PKG_VERSION").to_owned(),`
- prod: crates/iroha_telemetry/src/ws.rs:267 — `env!("CARGO_PKG_VERSION")`
- prod: crates/irohad/src/main.rs:655 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/irohad/src/main.rs:9285 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/ivm/src/bin/koto.rs:399 — `env!("CARGO_PKG_VERSION").as_bytes(),`
- prod: crates/kotodama_lang/src/compiler.rs:89 — `const COMPILER_FINGERPRINT: &str = concat!("kotodama_lang/", env!("CARGO_PKG_VERSION"));`
- prod: crates/musubi/src/cli.rs:77 — `version = env!("CARGO_PKG_VERSION"),`
- prod: crates/sorafs_car/src/bin/sorafs_fetch.rs:1095 — `Value::from(env!("CARGO_PKG_VERSION")),`
- prod: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:112 — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/sora-vpn-helper/src/main.rs:69 — `const VERSION: &str = env!("CARGO_PKG_VERSION");`
- tool: tools/telemetry-schema-diff/src/main.rs:252 — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## CARGO_PRIMARY_PACKAGE (build: 1)

- build: crates/soranet_pq/build.rs:6 — `if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {`

## CARGO_TARGET_DIR (prod: 5, test: 6, tool: 2)

- prod: crates/iroha_kagami/src/localnet.rs:3345 — `let target_dir = resolve_target_dir(&repo_root, env::var("CARGO_TARGET_DIR").ok().as_deref());`
- test: crates/iroha_test_network/src/lib.rs:1032 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: crates/iroha_test_network/src/lib.rs:1369 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/binary_resolver.rs:81 — `if let Some(target_root) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)`
- test: integration_tests/src/binary_resolver.rs:234 — `if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs:85 — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- test: integration_tests/src/kagami.rs:113 — `if let Ok(path) = env::var("CARGO_TARGET_DIR") {`
- prod: mochi/mochi-core/src/supervisor.rs:468 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:979 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:1033 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- prod: mochi/mochi-core/src/supervisor.rs:1087 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- tool: xtask/src/kagami_profiles.rs:594 — `if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {`
- tool: xtask/src/mochi.rs:391 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## CARGO_WORKSPACE_DIR (test: 1)

- test: crates/iroha_core/src/state.rs:39904 — `if let Some(workspace_dir) = option_env!("CARGO_WORKSPACE_DIR") {`

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
- build: crates/ivm/build.rs:164 — `env::var_os("CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:157 — `env::var_os("CXX").is_some()`

## DATASPACE_ADVERSARIAL_ARTIFACT_DIR (test: 1)

- test: integration_tests/tests/nexus/cross_lane.rs:1021 — `if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {`

## DOCS_RS (build: 1)

- build: crates/norito/build.rs:6 — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (bench: 1)

- bench: crates/norito/benches/enum_packed_bench.rs:75 — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_METAL_LIB (prod: 2)

- prod: crates/fastpq_prover/src/backend.rs:891 — `option_env!("FASTPQ_METAL_LIB")`
- prod: crates/fastpq_prover/src/metal.rs:2680 — `option_env!("FASTPQ_METAL_LIB")`

## FASTPQ_SKIP_GPU_BUILD (build: 1)

- build: crates/fastpq_prover/build.rs:47 — `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## FASTPQ_UPDATE_FIXTURES (test: 6)

- test: crates/fastpq_prover/tests/backend_regression.rs:146 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- test: crates/fastpq_prover/tests/backend_regression.rs:196 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- test: crates/fastpq_prover/tests/proof_fixture.rs:143 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- test: crates/fastpq_prover/tests/trace_commitment.rs:32 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- test: crates/fastpq_prover/tests/trace_commitment.rs:175 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- test: crates/fastpq_prover/tests/transcript_replay.rs:165 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`

## GENESIS_DEBUG_MODE (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs:15 — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## GENESIS_DEBUG_PAYLOAD (test: 1)

- test: crates/iroha_test_network/examples/genesis_debug.rs:134 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## GITHUB_STEP_SUMMARY (prod: 2)

- prod: crates/iroha_crypto/src/bin/gost_perf_check.rs:32 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:205 — `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## GIT_COMMIT_HASH (prod: 1)

- prod: crates/iroha_core/src/sumeragi/rbc_store.rs:119 — `git_commit: option_env!("GIT_COMMIT_HASH").map(str::to_owned),`

## GPUZSTD_CUDA_ARCH (build: 1)

- build: crates/gpuzstd_cuda/build.rs:50 — `if let Some(arch_flag) = env::var_os("GPUZSTD_CUDA_ARCH") {`

## GPUZSTD_CUDA_REQUIRE (test: 3)

- test: crates/gpuzstd_cuda/src/lib.rs:325 — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some() {`
- test: crates/gpuzstd_cuda/src/lib.rs:756 — `if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_none() {`
- test: crates/norito/src/core/gpu_zstd.rs:723 — `std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some()`

## GPUZSTD_CUDA_SKIP_BUILD (build: 1)

- build: crates/gpuzstd_cuda/build.rs:20 — `if env::var_os("GPUZSTD_CUDA_SKIP_BUILD").is_some() {`

## HOME (prod: 2, tool: 1)

- prod: crates/iroha/src/config.rs:62 — `env::var_os("HOME").map(PathBuf::from)`
- prod: crates/irohad/src/soracloud_runtime.rs:13635 — `if let Some(home) = std::env::var_os("HOME") {`
- tool: tools/sora-vpn-helper/src/main.rs:2291 — `if let Some(home) = env::var_os("HOME") {`

## HOST_CXX (build: 4)

- build: crates/fastpq_prover/build.rs:265 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/gpuzstd_cuda/build.rs:150 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/ivm/build.rs:165 — `|| env::var_os("HOST_CXX").is_some()`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:158 — `|| env::var_os("HOST_CXX").is_some()`

## IROHA_ACCOUNT_ID (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:267 — `account_id: std::env::var("IROHA_ACCOUNT_ID").ok(),`

## IROHA_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs:8764 — `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## IROHA_API_BASE (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:260 — `api_base: std::env::var("IROHA_API_BASE")`

## IROHA_CHAIN_ID (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:265 — `chain_id: std::env::var("IROHA_CHAIN_ID")`

## IROHA_CONF_GAS_SEED (test: 1)

- test: crates/iroha_test_samples/src/lib.rs:78 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_DA_SPOOL_DIR (test: 1)

- test: crates/iroha_core/src/state.rs:22693 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_DEBUG_GENESIS_PATH (test: 3)

- test: crates/iroha_genesis/src/lib.rs:3661 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/lib.rs:3722 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`
- test: crates/iroha_genesis/src/lib.rs:3753 — `let path = env::var("IROHA_DEBUG_GENESIS_PATH")`

## IROHA_DEBUG_SIGNED_GENESIS_PATH (test: 1)

- test: crates/iroha_genesis/src/lib.rs:3690 — `let path = env::var("IROHA_DEBUG_SIGNED_GENESIS_PATH")`

## IROHA_DEBUG_WSV_COMPONENTS (prod: 2)

- prod: crates/iroha_core/src/state.rs:37979 — `if std::env::var_os("IROHA_DEBUG_WSV_COMPONENTS").is_some() {`
- prod: crates/iroha_core/src/sumeragi/main_loop/commit.rs:885 — `if std::env::var_os("IROHA_DEBUG_WSV_COMPONENTS").is_some() {`

## IROHA_DUMP_MANIFEST_JSON (test: 1)

- test: crates/iroha_data_model/src/nexus/manifest.rs:1123 — `if std::env::var_os("IROHA_DUMP_MANIFEST_JSON").is_some() {`

## IROHA_GENESIS_FILE (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs:21 — `let genesis_path = std::env::var("IROHA_GENESIS_FILE")`

## IROHA_GENESIS_PUBLIC_KEY (test: 1)

- test: crates/iroha_core/tests/check_genesis_sig.rs:23 — `let pub_key_str = std::env::var("IROHA_GENESIS_PUBLIC_KEY").unwrap_or_else(|_| {`

## IROHA_GIT_COMMIT_HASH (build: 1)

- build: crates/iroha_core/build.rs:20 — `let commit = env::var("IROHA_GIT_COMMIT_HASH").ok()?;`

## IROHA_HARD_FORK_SNAPSHOT_BOOTSTRAP (prod: 1)

- prod: crates/sorafs_node/src/lib.rs:1436 — `hard_fork_snapshot_bootstrap_enabled_from(env::var_os("IROHA_HARD_FORK_SNAPSHOT_BOOTSTRAP"))`

## IROHA_INROU_LINUX_KVM (test: 3)

- test: crates/irohad/src/soracloud_runtime.rs:15303 — `if std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:23424 — `|| std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:23585 — `|| std::env::var("IROHA_INROU_LINUX_KVM").ok().as_deref() != Some("1")`

## IROHA_INROU_LINUX_KVM_INITRD_IMAGE (test: 2)

- test: crates/irohad/src/soracloud_runtime.rs:23435 — `let initrd_image = std::env::var("IROHA_INROU_LINUX_KVM_INITRD_IMAGE")`
- test: crates/irohad/src/soracloud_runtime.rs:23596 — `let initrd_image = std::env::var("IROHA_INROU_LINUX_KVM_INITRD_IMAGE")`

## IROHA_INROU_PORTABLE (test: 3)

- test: crates/irohad/src/soracloud_runtime.rs:15365 — `if std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:23030 — `|| std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:23216 — `|| std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")`

## IROHA_INROU_PORTABLE_ACCEL (prod: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:461 — `portable_vm_accel_from(std::env::var("IROHA_INROU_PORTABLE_ACCEL").ok().as_deref())`

## IROHA_INROU_PORTABLE_INITRD_IMAGE (test: 2, tool: 1)

- test: crates/irohad/src/soracloud_runtime.rs:23041 — `let initrd_image = std::env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`
- test: crates/irohad/src/soracloud_runtime.rs:23227 — `let initrd_image = std::env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`
- tool: xtask/src/soracloud_inrou.rs:120 — `if let Ok(value) = env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")`

## IROHA_INROU_PORTABLE_SMOKE_ENTRYPOINT (test: 1)

- test: crates/irohad/src/soracloud_runtime.rs:23241 — `let external_entrypoint = std::env::var("IROHA_INROU_PORTABLE_SMOKE_ENTRYPOINT")`

## IROHA_INROU_PORTABLE_SMOKE_HEALTHCHECK (test: 1)

- test: crates/irohad/src/soracloud_runtime.rs:23243 — `let external_healthcheck = std::env::var("IROHA_INROU_PORTABLE_SMOKE_HEALTHCHECK")`

## IROHA_KAGAMI_LOCALNET_KEEP (test: 1)

- test: integration_tests/tests/sumeragi_kagami_localnet.rs:79 — `if std::env::var_os("IROHA_KAGAMI_LOCALNET_KEEP").is_some() {`

## IROHA_MCP_URL (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:264 — `mcp_url: std::env::var("IROHA_MCP_URL").ok().or_else(|| {mcp_url}),`

## IROHA_METRICS_PANIC_ON_DUPLICATE (test: 2)

- test: crates/iroha_telemetry/src/metrics.rs:16215 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- test: crates/iroha_torii/tests/metrics_registry.rs:34 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## IROHA_PRIVATE_KEY (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:268 — `private_key: std::env::var("IROHA_PRIVATE_KEY").ok(),`

## IROHA_REALISTIC_30TPS_LOAD_KIND (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:527 — `let Some(raw) = std::env::var("IROHA_REALISTIC_30TPS_LOAD_KIND")`

## IROHA_REALISTIC_30TPS_LOG_LEVEL (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2889 — `std::env::var("IROHA_REALISTIC_30TPS_LOG_LEVEL").unwrap_or_else(|_| "WARN".into());`

## IROHA_RUN_IGNORED (test: 51)

- test: crates/iroha_core/tests/check_genesis_sig.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_finalize_real_vk.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_zk_ballot_lock_verified.rs:11 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/gov_zk_ballot_real_vk.rs:11 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/zk_roots_get_cap.rs:53 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_core/tests/zk_vote_get_tally.rs:49 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_crypto/src/merkle.rs:1492 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_crypto/src/merkle.rs:1507 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_crypto/tests/merkle_norito_roundtrip.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_crypto/tests/merkle_norito_roundtrip.rs:42 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:36 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:57 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_data_model/tests/model_derive_repro.rs:83 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:640 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1033 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1122 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1230 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1365 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1498 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1649 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1810 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:1919 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:2055 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:2206 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_call_integration.rs:2386 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_deploy_integration.rs:41 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_deploy_integration.rs:142 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/contracts_deploy_integration.rs:388 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_council_persist_integration.rs:64 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_council_vrf.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_enact_handler.rs:34 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_mode_mismatch_and_autoclose.rs:44 — `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_protected_endpoints_router.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/iroha_torii/tests/gov_read_endpoints_router.rs:42 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:15297 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:15359 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/irohad/src/soracloud_runtime.rs:23029 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:23215 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:23423 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/irohad/src/soracloud_runtime.rs:23584 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")`
- test: crates/ivm/tests/beep_test.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/kotodama_struct_fields.rs:12 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:50 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:303 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:461 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/permissions.rs:527 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/pipeline_block_rejected.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- test: integration_tests/tests/sorting.rs:46 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_RUN_ZK_WRAPPERS (test: 1)

- test: crates/ivm/tests/kotodama_wrappers.rs:2 — `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`

## IROHA_SKIP_BIND_CHECKS (test: 1)

- test: crates/iroha_test_network/src/lib.rs:6004 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (test: 1)

- test: crates/iroha_crypto/tests/sm_cli_matrix.rs:47 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_STARTUP_TRACE (prod: 2)

- prod: crates/iroha_core/src/sumeragi/mod.rs:54 — `std::env::var_os("IROHA_STARTUP_TRACE").is_some()`
- prod: crates/irohad/src/main.rs:123 — `env::var_os("IROHA_STARTUP_TRACE").is_some()`

## IROHA_TAIRA_KEEP_LOCALNET (test: 2)

- test: integration_tests/tests/taira_public_localnet.rs:2438 — `if std::env::var_os("IROHA_TAIRA_KEEP_LOCALNET").is_some() {`
- test: integration_tests/tests/taira_public_localnet.rs:2447 — `if std::env::var_os("IROHA_TAIRA_KEEP_LOCALNET").is_some() {`

## IROHA_TAIRA_SIM_SEED (test: 1)

- test: integration_tests/tests/taira_public_localnet.rs:513 — `let seed = std::env::var("IROHA_TAIRA_SIM_SEED")`

## IROHA_TEST_BUILD_PROFILE (test: 1)

- test: integration_tests/src/binary_resolver.rs:180 — `std::env::var("IROHA_TEST_BUILD_PROFILE").ok().as_deref(),`

## IROHA_TEST_CLIENT_TTL_MS (test: 5)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:2858 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4309 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4598 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4885 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:5578 — `let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");`

## IROHA_TEST_DUMP_GENESIS (test: 1)

- test: crates/iroha_test_network/src/lib.rs:12649 — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`

## IROHA_TEST_LOG_LEVEL (test: 1)

- test: integration_tests/tests/sumeragi_da.rs:2720 — `let log_level = std::env::var("IROHA_TEST_LOG_LEVEL").unwrap_or_else(|_| "WARN".to_string());`

## IROHA_TEST_NETWORK_PARALLELISM (test: 1)

- test: integration_tests/tests/address_canonicalisation.rs:48 — `if let Ok(raw) = env::var("IROHA_TEST_NETWORK_PARALLELISM")`

## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (build: 1, test: 1)

- test: crates/iroha_test_network/src/config.rs:377 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- build: integration_tests/build.rs:76 — `if env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_REAL_NETWORK (prod: 1)

- prod: crates/iroha_core/src/sumeragi/main_loop/tests.rs:4432 — `std::env::var_os("IROHA_TEST_REAL_NETWORK").is_some()`

## IROHA_TEST_SERIALIZE_NETWORKS (test: 2)

- test: integration_tests/tests/address_canonicalisation.rs:43 — `if let Ok(raw) = env::var("IROHA_TEST_SERIALIZE_NETWORKS")`
- test: integration_tests/tests/asset.rs:254 — `if std::env::var_os("IROHA_TEST_SERIALIZE_NETWORKS").is_none() {`

## IROHA_TEST_SKIP_BUILD (test: 2)

- test: crates/iroha_test_network/src/lib.rs:1913 — `std::env::var("IROHA_TEST_SKIP_BUILD")`
- test: integration_tests/src/binary_resolver.rs:51 — `std::env::var("IROHA_TEST_SKIP_BUILD").ok().as_deref(),`

## IROHA_TEST_USE_DEFAULT_EXECUTOR (test: 3)

- test: crates/iroha_core/src/executor.rs:10688 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- test: crates/iroha_core/src/executor.rs:11053 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- test: crates/iroha_core/src/state.rs:39897 — `if std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR").is_some() {`

## IROHA_THROUGHPUT_ARTIFACT_DIR (test: 3)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:3744 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:5524 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`
- test: integration_tests/tests/sumeragi_localnet_smoke.rs:6170 — `if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {`

## IROHA_THROUGHPUT_DELAY_MS (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:482 — `if let Ok(delay) = std::env::var("IROHA_THROUGHPUT_DELAY_MS") {`

## IROHA_THROUGHPUT_RBC_ENCODING (test: 1)

- test: integration_tests/tests/sumeragi_localnet_smoke.rs:4891 — `std::env::var("IROHA_THROUGHPUT_RBC_ENCODING").unwrap_or_else(|_| "plain".to_owned());`

## IROHA_TORII_ALLOW_LIVE_ASSET_HOLDER_AGGREGATE (test: 1)

- test: crates/iroha_torii/src/routing.rs:89279 — `|| std::env::var("IROHA_TORII_ALLOW_LIVE_ASSET_HOLDER_AGGREGATE")`

## IROHA_TORII_LOCAL_READ_FANOUT_COORDINATOR (prod: 1)

- prod: crates/iroha_torii/src/lib.rs:1591 — `std::env::var("IROHA_TORII_LOCAL_READ_FANOUT_COORDINATOR")`

## IROHA_TORII_OPENAPI_ACTUAL (test: 1)

- test: crates/iroha_torii/tests/router_feature_matrix.rs:96 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_EXPECTED (test: 2)

- test: crates/iroha_torii/tests/router_feature_matrix.rs:90 — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- test: crates/iroha_torii/tests/router_feature_matrix.rs:106 — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_TORII_OPENAPI_TOKENS (tool: 2)

- tool: xtask/src/main.rs:11214 — `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`
- tool: xtask/src/main.rs:11267 — `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IROHA_TORII_PUBLIC_DATASPACE_UPSTREAMS (prod: 1)

- prod: crates/iroha_torii/src/lib.rs:1550 — `let Ok(raw) = std::env::var("IROHA_TORII_PUBLIC_DATASPACE_UPSTREAMS") else {`

## IROHA_TORII_URL (prod: 1)

- prod: mochi/mochi-core/src/bootstrap.rs:262 — `torii_url: std::env::var("IROHA_TORII_URL")`

## IVM_BIN (test: 2)

- test: integration_tests/tests/kotodama_examples.rs:64 — `let ivm_bin = env::var("IVM_BIN")`
- test: integration_tests/tests/kotodama_examples.rs:160 — `let ivm_bin = env::var("IVM_BIN")`

## IVM_COMPILER_DEBUG (test: 1)

- test: crates/kotodama_lang/src/compiler.rs:16981 — `if cfg!(any(test, debug_assertions)) && std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_GENCODE (build: 1)

- build: crates/ivm/build.rs:55 — `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| "arch=compute_61,code=sm_61".to_string());`

## IVM_CUDA_NVCC (build: 1)

- build: crates/ivm/build.rs:42 — `let nvcc = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVCC_EXTRA (build: 1)

- build: crates/ivm/build.rs:56 — `let extra_flags: Vec<String> = env::var("IVM_CUDA_NVCC_EXTRA")`

## IVM_CUDA_SELFTEST_TRACE (prod: 1)

- prod: crates/ivm/src/cuda.rs:115 — `if std::env::var_os("IVM_CUDA_SELFTEST_TRACE").is_some() {`

## IVM_DEBUG_AED_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:360 — `let aed_asset_raw = std::env::var("IVM_DEBUG_AED_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:228 — `let asset_raw = std::env::var("IVM_DEBUG_ASSET_DEFINITION")`

## IVM_DEBUG_CBDC_ASSET_DEFINITION (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:366 — `let cbdc_asset_raw = std::env::var("IVM_DEBUG_CBDC_ASSET_DEFINITION").unwrap_or_else(|_| {`

## IVM_DEBUG_DOMAIN (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:230 — `let domain_raw = std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank".to_owned());`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:373 — `std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "centralbank.universal".to_owned());`

## IVM_DEBUG_FROM_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:226 — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:358 — `std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");`

## IVM_DEBUG_IR (test: 1)

- test: crates/ivm/tests/debug_contains.rs:15 — `if std::env::var_os("IVM_DEBUG_IR").is_some() {`

## IVM_DEBUG_METAL_ENUM (debug: 1)

- debug: crates/ivm/src/vector.rs:477 — `std::env::var("IVM_DEBUG_METAL_ENUM")`

## IVM_DEBUG_METAL_SELFTEST (debug: 1)

- debug: crates/ivm/src/vector.rs:1208 — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DEBUG_RATIO (test: 1)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:374 — `let ratio_raw = std::env::var("IVM_DEBUG_RATIO").unwrap_or_else(|_| "76".to_owned());`

## IVM_DEBUG_TO_ACCOUNT (test: 2)

- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:227 — `let to_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`
- test: crates/iroha_core/tests/ivm_pointer_abi_apply.rs:359 — `let dst_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");`

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

- debug: crates/ivm/src/vector.rs:1195 — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IVM_TOOL_BIN (test: 1)

- test: integration_tests/tests/kotodama_examples.rs:111 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## IZANAMI_ALLOW_NET (test: 1)

- test: crates/izanami/src/chaos.rs:8763 — `std::env::var("IZANAMI_ALLOW_NET")`

## IZANAMI_TUI_ALLOW_ZERO_SEED (prod: 1)

- prod: crates/izanami/src/tui.rs:185 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:52 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_REQUIRE (test: 3)

- test: crates/norito/accelerators/jsonstage1_cuda/src/lib.rs:421 — `std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some()`
- test: crates/norito/src/core.rs:2160 — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`
- test: crates/norito/src/lib.rs:5202 — `if std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some() {`

## JSONSTAGE1_CUDA_SKIP_BUILD (build: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:23 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI6_ARCHIVES (test: 1)

- test: crates/iroha_data_model/src/offline/mod.rs:10999 — `if std::env::var_os("KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI6_ARCHIVES").is_some() {`

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

## MOCHI_CONFIG (prod: 1)

- prod: mochi/mochi-ui-egui/src/config.rs:484 — `if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## MOCHI_DATA_ROOT (prod: 1)

- prod: mochi/mochi-core/src/supervisor.rs:3761 — `std::env::var_os("MOCHI_DATA_ROOT")`

## MOCHI_DETACHED (prod: 1)

- prod: mochi/mochi-ui-egui/src/gui.rs:1052 — `if std::env::var_os("MOCHI_DETACHED").is_some() {`

## MOCHI_TEST_USE_INTERNAL_GENESIS (prod: 1)

- prod: mochi/mochi-core/src/supervisor.rs:3517 — `if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {`

## NORITO_CHECK_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs:17 — `if env::var_os("NORITO_CHECK_BINDINGS_SYNC").is_none() {`

## NORITO_CPU_INFO (tool: 1)

- tool: xtask/src/stage1_bench.rs:74 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_CUDA_REQUIRE (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:1247 — `if std::env::var_os("NORITO_CRC64_CUDA_REQUIRE").is_none() {`

## NORITO_CRC64_GPU_LIB (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:314 — `let raw = std::env::var_os("NORITO_CRC64_GPU_LIB")?;`

## NORITO_DISABLE_PACKED_STRUCT (test: 1)

- test: crates/norito/src/lib.rs:356 — `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## NORITO_GPU_CRC64_MIN_BYTES (test: 1)

- test: crates/norito/src/core/simd_crc64.rs:68 — `let configured = std::env::var("NORITO_GPU_CRC64_MIN_BYTES")`

## NORITO_PAR_STAGE1_MIN (test: 1)

- test: crates/norito/src/lib.rs:5474 — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## NORITO_SKIP_BINDINGS_SYNC (build: 1)

- build: crates/norito/build.rs:13 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_GPU_MIN_BYTES (test: 1)

- test: crates/norito/src/lib.rs:5508 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## NORITO_TRACE (test: 3)

- test: crates/norito/src/lib.rs:134 — `std::env::var_os("NORITO_TRACE").is_some()`
- test: crates/norito/src/lib.rs:139 — `*ENABLED.get_or_init(|| std::env::var_os("NORITO_TRACE").is_some())`
- test: crates/norito/src/lib.rs:155 — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## NVCC (build: 1)

- build: crates/ivm/build.rs:43 — `.or_else(|_| env::var("NVCC"))`

## OUT_DIR (build: 4, prod: 12, test: 3)

- build: crates/fastpq_prover/build.rs:109 — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
- build: crates/iroha_data_model/build.rs:13 — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));`
- prod: crates/iroha_data_model/src/lib.rs:206 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
- build: crates/ivm/build.rs:39 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- build: crates/ivm/build.rs:138 — `if let Some(out_dir) = env::var_os("OUT_DIR") {`
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
- test: crates/ivm/src/ptx_tests.rs:7 — `let out_dir = match std::env::var("OUT_DIR") {`
- test: crates/ivm/tests/ptx_kernels.rs:5 — `let out_dir = env!("OUT_DIR");`

## PATH (prod: 4, test: 1, tool: 1)

- prod: crates/iroha_torii/src/zk_attachments.rs:952 — `let search_path = env::var_os("PATH");`
- prod: crates/iroha_torii/src/zk_attachments.rs:1036 — `let path = env::var_os("PATH");`
- prod: crates/irohad/src/soracloud_runtime.rs:13561 — `let mut search_roots = std::env::var_os("PATH")`
- test: integration_tests/tests/kotodama_examples.rs:21 — `let path = env::var_os("PATH")?;`
- prod: mochi/mochi-core/src/supervisor.rs:915 — `let path_var = env::var_os("PATH")?;`
- tool: tools/sora-vpn-backend/src/main.rs:1294 — `let Some(path) = env::var_os("PATH") else {`

## PATHEXT (prod: 1)

- prod: crates/irohad/src/soracloud_runtime.rs:13593 — `std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());`

## PRINT_SORACLES_FIXTURES (test: 1)

- test: crates/iroha_data_model/src/oracle/mod.rs:3568 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORII_SPEC (test: 1)

- test: crates/iroha_torii/src/openapi.rs:13426 — `if std::env::var("PRINT_TORII_SPEC").is_ok() {`

## PROFILE (build: 1, prod: 1, test: 3)

- prod: crates/iroha_core/src/sumeragi/rbc_store.rs:118 — `profile: option_env!("PROFILE").unwrap_or("unknown").to_owned(),`
- test: crates/iroha_test_network/src/lib.rs:1060 — `if let Ok(profile) = std::env::var("PROFILE") {`
- build: integration_tests/build.rs:64 — `let profile = if env::var("PROFILE").ok().as_deref() == Some("release") {`
- test: integration_tests/src/binary_resolver.rs:247 — `if let Ok(profile) = std::env::var("PROFILE")`
- test: integration_tests/src/kagami.rs:53 — `let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());`

## PYTHON3 (test: 1)

- test: crates/sorafs_car/tests/taikai_viewer_cli.rs:30 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONPATH (test: 1)

- test: crates/iroha_cli/tests/cli_smoke.rs:6023 — `match env::var("PYTHONPATH") {`

## REPO_PROOF_DIGEST_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2468 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (test: 1)

- test: crates/iroha_core/src/smartcontracts/isi/repo.rs:2456 — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUST_LOG (prod: 2, test: 4)

- test: crates/iroha_test_network/src/lib.rs:10469 — `let original = env::var("RUST_LOG").ok();`
- test: crates/iroha_test_network/src/lib.rs:10486 — `let original = env::var("RUST_LOG").ok();`
- prod: crates/izanami/src/chaos.rs:2759 — `if let Ok(filter) = std::env::var("RUST_LOG") {`
- prod: crates/izanami/src/config.rs:686 — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`
- test: integration_tests/tests/sumeragi_kagami_localnet.rs:142 — `if std::env::var_os("RUST_LOG").is_none() {`
- test: integration_tests/tests/taira_public_localnet.rs:377 — `if std::env::var_os("RUST_LOG").is_none() {`

## SCCP_DEBUG_SOURCE_DEPLOYMENT_JSON (example: 1)

- example: crates/iroha_sccp/examples/debug_bundle.rs:146 — `std::env::var("SCCP_DEBUG_SOURCE_DEPLOYMENT_JSON"),`

## SCCP_DEBUG_SOURCE_MATERIAL_JSON (example: 1)

- example: crates/iroha_sccp/examples/debug_bundle.rs:145 — `std::env::var("SCCP_DEBUG_SOURCE_MATERIAL_JSON"),`

## SM_PERF_CPU_LABEL (prod: 2)

- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:712 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:754 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## SORAFS_NODE_SKIP_INGEST_TESTS (test: 1)

- test: crates/sorafs_node/tests/cli.rs:18 — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORAFS_TORII_SKIP_INGEST_TESTS (test: 1)

- test: crates/iroha_torii/tests/sorafs_discovery.rs:102 — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORANET_VPN_INTERFACE (test: 1, tool: 1)

- tool: tools/sora-vpn-helper/src/main.rs:2323 — `env::var("SORANET_VPN_INTERFACE")`
- test: tools/sora-vpn-helper/src/main.rs:3153 — `let original = env::var_os("SORANET_VPN_INTERFACE");`

## SORANET_VPN_STATE_FILE (tool: 1)

- tool: tools/sora-vpn-helper/src/main.rs:2282 — `env::var("SORANET_VPN_STATE_FILE")`

## SUMERAGI_ADVERSARIAL_ARTIFACT_DIR (test: 1)

- test: integration_tests/tests/sumeragi_adversarial.rs:2421 — `let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {`

## SUMERAGI_BASELINE_ARTIFACT_DIR (prod: 1, test: 1)

- prod: crates/build-support/src/bin/sumeragi_baseline_report.rs:42 — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`
- test: integration_tests/tests/sumeragi_npos_performance.rs:1739 — `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`

## SUMERAGI_DA_ARTIFACT_DIR (prod: 1, test: 1)

- prod: crates/build-support/src/bin/sumeragi_da_report.rs:43 — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`
- test: integration_tests/tests/sumeragi_da.rs:3435 — `let Ok(dir) = std::env::var("SUMERAGI_DA_ARTIFACT_DIR") else {`

## SWIFT_OFFLINE_PROOF (test: 1)

- test: crates/iroha_core/src/zk.rs:41242 — `let Some(path) = std::env::var_os("SWIFT_OFFLINE_PROOF") else {`

## SystemRoot (prod: 1)

- prod: crates/fastpq_prover/src/backend.rs:581 — `env::var_os("SystemRoot").map(PathBuf::from)`

## TARGET (build: 1, prod: 1, tool: 2)

- prod: crates/build-support/src/lib.rs:23 — `let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());`
- build: crates/ivm/build.rs:17 — `if let Ok(target) = env::var("TARGET") {`
- tool: xtask/src/poseidon_bench.rs:88 — `target: std::env::var("TARGET")`
- tool: xtask/src/stage1_bench.rs:68 — `target: std::env::var("TARGET")`

## TEST_LOG_FILTER (prod: 1)

- prod: crates/iroha_logger/src/lib.rs:131 — `filter: std::env::var("TEST_LOG_FILTER")`

## TEST_LOG_LEVEL (prod: 1)

- prod: crates/iroha_logger/src/lib.rs:127 — `level: std::env::var("TEST_LOG_LEVEL")`

## TEST_NETWORK_CARGO (test: 1)

- test: crates/iroha_test_network/src/lib.rs:1701 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TEST_NETWORK_IROHAD_FEATURES (test: 3)

- test: integration_tests/tests/nexus/cross_dataspace_zk_stark_localnet.rs:232 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_ace_localnet.rs:50 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`
- test: integration_tests/tests/zk_stark_network.rs:64 — `std::env::var("TEST_NETWORK_IROHAD_FEATURES")`

## TORII_MOCK_HARNESS_METRICS_PATH (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:106 — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:109 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_RETRY_TOTAL (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:297 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:112 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (tool: 1)

- tool: xtask/src/bin/torii_mock_harness.rs:104 — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## TORII_OFFLINE_ISSUER_PRIVATE_KEY (prod: 1)

- prod: crates/iroha_config/src/parameters/user.rs:22092 — `std::env::var("TORII_OFFLINE_ISSUER_PRIVATE_KEY")`

## TORII_OPENAPI_TOKEN (tool: 2)

- tool: xtask/src/main.rs:11209 — `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`
- tool: xtask/src/main.rs:11263 — `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## UPDATE_FIXTURES (test: 2)

- test: crates/iroha_core/tests/pin_registry.rs:115 — `if env::var_os("UPDATE_FIXTURES").is_some() {`
- test: crates/iroha_core/tests/snapshots.rs:48 — `let update = env::var("UPDATE_FIXTURES")`

## USERPROFILE (prod: 1)

- prod: crates/iroha/src/config.rs:60 — `env::var_os("USERPROFILE").map(PathBuf::from)`

## VERGEN_CARGO_FEATURES (prod: 1, test: 1)

- test: crates/iroha_telemetry/src/metrics.rs:5503 — `cargo_features: option_env!("VERGEN_CARGO_FEATURES")`
- prod: crates/irohad/src/main.rs:11394 — `const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {`

## VERGEN_CARGO_TARGET_TRIPLE (prod: 1, test: 1)

- test: crates/iroha_telemetry/src/metrics.rs:5506 — `target_triple: option_env!("VERGEN_CARGO_TARGET_TRIPLE")`
- prod: crates/iroha_telemetry/src/ws.rs:262 — `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");`

## VERGEN_GIT_SHA (prod: 3, test: 1)

- prod: crates/iroha_cli/src/main_shared.rs:63 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`
- test: crates/iroha_telemetry/src/metrics.rs:5500 — `git_commit_sha: option_env!("VERGEN_GIT_SHA")`
- prod: crates/iroha_telemetry/src/ws.rs:261 — `let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");`
- prod: crates/irohad/src/main.rs:11389 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`

## VERIFY_BATCH (bench: 1)

- bench: crates/ivm/benches/bench_voting.rs:226 — `let verify_batch = std::env::var("VERIFY_BATCH")`

## VERIFY_EVERY (bench: 1)

- bench: crates/ivm/benches/bench_voting.rs:214 — `let verify_every: u64 = std::env::var("VERIFY_EVERY")`

## VOTERS (bench: 1)

- bench: crates/ivm/benches/bench_voting.rs:208 — `let voters: u64 = std::env::var("VOTERS")`

## XDG_RUNTIME_DIR (tool: 1)

- tool: tools/sora-vpn-helper/src/main.rs:2288 — `if let Some(runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {`

## XTASK_TEST_KAGAMI_BIN (test: 2)

- test: xtask/src/kagami_profiles.rs:795 — `if std::env::var("XTASK_TEST_KAGAMI_BIN").is_err() {`
- test: xtask/src/kagami_profiles.rs:798 — `let kagami_path = PathBuf::from(std::env::var("XTASK_TEST_KAGAMI_BIN").unwrap());`
