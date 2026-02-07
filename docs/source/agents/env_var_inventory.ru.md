---
lang: ru
direction: ltr
source: docs/source/agents/env_var_inventory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0a23896374a10f0862aeec69c607ba3022d3b337ae7f6bb54a61b8f7424410
source_last_modified: "2026-01-21T10:22:44.993254+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Инвентарь переключения среды

_Последнее обновление через `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`_

Всего ссылок: **505** · Уникальные переменные: **137**

## ACTIONS_ID_TOKEN_REQUEST_TOKEN (прод.: 1)

- продукт: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:313 — `let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {`

## ACTIONS_ID_TOKEN_REQUEST_URL (продукт: 1)

- продукт: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:310 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`

## BLOCK_DUMP_HEIGHTS (пример: 1)

- пример: crates/iroha_core/examples/block_dump.rs:62 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`

## BLOCK_DUMP_SUM_ASSET (пример: 1)

- пример: crates/iroha_core/examples/block_dump.rs:51 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`

## BLOCK_DUMP_VERBOSE (пример: 1)

- пример: crates/iroha_core/examples/block_dump.rs:50 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`

## ГРУЗ (производство: 3, испытание: 2)

- тест: crates/iroha_test_network/src/lib.rs:1026 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- тест: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:11 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`
- продукт: mochi/mochi-core/src/supervisor.rs:547 — `let cargo = env::var_os("CARGO")`
- продукт: mochi/mochi-core/src/supervisor.rs:602 — `let cargo = env::var_os("CARGO")`
- продукт: mochi/mochi-core/src/supervisor.rs:656 — `let cargo = env::var_os("CARGO")`

## CARGO_BIN_EXE_iroha (тест: 2)

- тест: crates/iroha_cli/tests/cli_smoke.rs:40 — `env!("CARGO_BIN_EXE_iroha")`
- тест: crates/iroha_cli/tests/taikai_policy.rs:20 — `env!("CARGO_BIN_EXE_iroha")`

## CARGO_BIN_EXE_iroha_monitor (тест: 4)

- тест: crates/iroha_monitor/tests/attach_render.rs:11 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- тест: crates/iroha_monitor/tests/http_limits.rs:10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- тест: crates/iroha_monitor/tests/invalid_credentials.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- тест: crates/iroha_monitor/tests/smoke.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## CARGO_BIN_EXE_kagami (тест: 2)

- тест: crates/iroha_kagami/tests/common/mod.rs:22 — `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- тест: crates/iroha_kagami/tests/pop_embed.rs:34 — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`

## CARGO_BIN_EXE_kagami_mock (тест: 1)

- тест: mochi/mochi-integration/tests/supervisor.rs:33 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## CARGO_BIN_EXE_koto_compile (тест: 3)

- тест: crates/ivm/tests/cli_smoke.rs:8 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- тест: crates/ivm/tests/cli_smoke.rs:56 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- тест: crates/ivm/tests/cli_smoke.rs:88 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`

## CARGO_BIN_EXE_sorafs_chunk_dump (тест: 1)

- тест: crates/sorafs_chunker/tests/one_gib.rs:103 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## CARGO_BIN_EXE_sorafs_cli (тест: 1)

- тест: crates/sorafs_car/tests/sorafs_cli.rs:42 — `let path = env::var("CARGO_BIN_EXE_sorafs_cli")`

## CARGO_BIN_EXE_sorafs_fetch (тест: 1)

- тест: crates/sorafs_car/src/bin/sorafs_fetch.rs:2831 — `if let Ok(path) = env::var("CARGO_BIN_EXE_sorafs_fetch") {`

## CARGO_BIN_EXE_taikai_car (тест: 1)

- тест: crates/sorafs_car/tests/sorafs_cli.rs:48 — `let path = env::var("CARGO_BIN_EXE_taikai_car")`

## CARGO_BIN_NAME (производитель: 3)

- продукт: crates/iroha_cli/src/main_shared.rs:63 — `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`
- продукт: crates/iroha_cli/src/main_shared.rs:72 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- продукт: crates/irohad/src/main.rs:3176 — `let build_line = BuildLine::from_bin_name(env!("CARGO_BIN_NAME"));`

## CARGO_BUILD_TARGET (инструмент: 2)

- инструмент: xtask/src/poseidon_bench.rs:88 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- инструмент: xtask/src/stage1_bench.rs:64 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_TARGET_ARCH (продукт: 2, инструмент: 2)- продукт: crates/iroha_crypto/src/bin/sm_perf_check.rs:632 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- продукт: crates/iroha_crypto/src/bin/sm_perf_check.rs:668 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- инструмент: xtask/src/poseidon_bench.rs:89 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- инструмент: xtask/src/stage1_bench.rs:65 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## CARGO_CFG_TARGET_OS (сборка: 1, продукт: 2, инструмент: 2)

- сборка: crates/fastpq_prover/build.rs:27 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- продукт: crates/iroha_crypto/src/bin/sm_perf_check.rs:633 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- продукт: crates/iroha_crypto/src/bin/sm_perf_check.rs:669 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- инструмент: xtask/src/poseidon_bench.rs:91 — `os: std::env::var("CARGO_CFG_TARGET_OS")`
- инструмент: xtask/src/stage1_bench.rs:67 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## CARGO_FEATURE_CUDA (сборка: 2)

- сборка: crates/fastpq_prover/build.rs:25 — `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
- сборка: crates/ivm/build.rs:12 — `if env::var_os("CARGO_FEATURE_CUDA").is_some()`

## CARGO_FEATURE_CUDA_KERNEL (сборка: 1)

- сборка: crates/norito/accelerators/jsonstage1_cuda/build.rs:12 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (сборка: 1)

- сборка: crates/fastpq_prover/build.rs:26 — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## CARGO_FEATURE_FFI_EXPORT (производство: 1)

- продукт: crates/build-support/src/lib.rs:30 — `let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();`

## CARGO_FEATURE_FFI_IMPORT (производитель: 1)

- продукт: crates/build-support/src/lib.rs:29 — `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`

## CARGO_MANIFEST_DIR (стенд: 4, сборка: 5, пример: 1, продукция: 27, тест: 164, инструмент: 4)- продукт: crates/fastpq_prover/src/poseidon_manifest.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/fastpq_prover/tests/packing.rs:17 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:9 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- тест: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:28 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- тест: crates/fastpq_prover/tests/proof_fixture.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/fastpq_prover/tests/trace_commitment.rs:14 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- тест: crates/fastpq_prover/tests/transcript_replay.rs:56 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha/src/client.rs:8409 — `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha/src/sm.rs:201 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha/tests/sm_signing.rs:35 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_cli/src/compute.rs:518 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- продукт: crates/iroha_cli/src/main_shared.rs:766 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_cli/tests/cli_smoke.rs:111 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_cli/tests/cli_smoke.rs:5236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- продукт: crates/iroha_config/src/parameters/user.rs:3490 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_config/src/parameters/user.rs:11561 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_config/tests/fastpq_queue_overrides.rs:15 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_config/tests/fixtures.rs:36 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- скамейка: crates/iroha_core/benches/blocks/common.rs:261 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- скамейка: crates/iroha_core/benches/blocks/common/mod.rs:272 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- скамейка: crates/iroha_core/benches/validation.rs:96 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- сборка: crates/iroha_core/build.rs:19 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- пример: crates/iroha_core/examples/generate_parity_fixtures.rs:17 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_core/src/executor.rs:2385 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- тест: crates/iroha_core/src/executor.rs:2521 — `let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_core/src/executor.rs:2661 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- продукт: crates/iroha_core/src/smartcontracts/isi/offline.rs:2152 — `env!("CARGO_MANIFEST_DIR"),`
- продукт: crates/iroha_core/src/smartcontracts/isi/offline.rs:2156 — `env!("CARGO_MANIFEST_DIR"),`
- продукт: crates/iroha_core/src/smartcontracts/isi/offline.rs:2164 — `env!("CARGO_MANIFEST_DIR"),`
- продукт: crates/iroha_core/src/smartcontracts/isi/offline.rs:2171 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_core/src/smartcontracts/isi/offline.rs:4852 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_core/src/smartcontracts/isi/repo.rs:1870 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_core/src/smartcontracts/isi/repo.rs:1874 — `env!("CARGO_MANIFEST_DIR"),`
- продукт: crates/iroha_core/src/state.rs:9990 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- тест: crates/iroha_core/src/state.rs:12012 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_core/src/streaming.rs:2980 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_core/src/tx.rs:4424 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_core/tests/executor_migration_introspect.rs:25 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_core/tests/pin_registry.rs:1158 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_core/tests/snapshots.rs:29 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_core/tests/sumeragi_doc_sync.rs:57 — `Path::new(env!("CARGO_MANIFEST_DIR"))`- тест: crates/iroha_crypto/tests/confidential_keyset_vectors.rs:57 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_crypto/tests/sm2_fixture_vectors.rs:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_crypto/tests/sm_cli_matrix.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- сборка: crates/iroha_data_model/build.rs:11 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");`
- продукт: crates/iroha_data_model/src/lib.rs:186 — `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- продукт: crates/iroha_data_model/src/lib.rs:190 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/src/offline/poseidon.rs:449 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_data_model/src/soranet/vpn.rs:1217 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- тест: crates/iroha_data_model/tests/account_address_vectors.rs:147 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_data_model/tests/address_curve_registry.rs:33 — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:48 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_data_model/tests/confidential_wallet_fixtures.rs:15 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_data_model/tests/consensus_roundtrip.rs:1308 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_data_model/tests/offline_fixtures.rs:130 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:29 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:33 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:37 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:41 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:46 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:50 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:54 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:58 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:62 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:200 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_data_model/tests/runtime_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_genesis/src/lib.rs:1173 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- тест: crates/iroha_genesis/src/lib.rs:3847 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- тест: crates/iroha_genesis/src/lib.rs:4228 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- тест: crates/iroha_genesis/src/lib.rs:4243 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- тест: crates/iroha_i18n/src/lib.rs:495 — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- тест: crates/iroha_js_host/src/lib.rs:6943 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- продукт: crates/iroha_kagami/samples/codec/generate.rs:13 — `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- продукт: crates/iroha_kagami/samples/codec/src/main.rs:35 — `let dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_kagami/src/codec.rs:393 — `env!("CARGO_MANIFEST_DIR"),`
- продукт: crates/iroha_kagami/src/localnet.rs:604 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_kagami/tests/codec.rs:11 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- тест: crates/iroha_telemetry/tests/drill_log.rs:10 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/iroha_test_network/src/config.rs:724 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- тест: crates/iroha_test_network/src/fslock_ports.rs:23 — `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- тест: crates/iroha_test_network/src/fslock_ports.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_test_network/src/lib.rs:281 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_test_samples/src/lib.rs:204 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_test_samples/src/lib.rs:241 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_torii/src/da/tests.rs:3490 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- тест: crates/iroha_torii/src/sorafs/api.rs:4053 — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_torii/tests/account_address_vectors.rs:140 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/iroha_torii/tests/accounts_portfolio.rs:91 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/iroha_torii/tests/sorafs_discovery.rs:1056 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сборка: crates/ivm/build.rs:21 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- продукт: crates/ivm/src/bin/gen_abi_hash_doc.rs:28 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- продукт: crates/ivm/src/bin/gen_header_doc.rs:48 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- продукт: crates/ivm/src/bin/gen_pointer_types_doc.rs:23 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- продукт: crates/ivm/src/bin/gen_syscalls_doc.rs:24 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- продукт: crates/ivm/src/bin/ivm_prebuild.rs:16 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- продукт: crates/ivm/src/bin/ivm_predecoder_export.rs:23 — `let _crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- продукт: crates/ivm/src/predecoder_fixtures.rs:236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- тест: crates/ivm/tests/cli_smoke.rs:9 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- тест: crates/ivm/tests/cli_smoke.rs:57 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- тест: crates/ivm/tests/cli_smoke.rs:89 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- тест: crates/ivm/tests/docs_consistency.rs:3 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- тест: crates/ivm/tests/ivm_abi_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/ivm/tests/ivm_header_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/ivm/tests/norito_portal_snippets_compile.rs:19 — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/ivm/tests/pointer_types_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- тест: crates/ivm/tests/pointer_types_doc_generated_ivm_md.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/ivm/tests/syscalls_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- тест: crates/ivm/tests/syscalls_doc_sync.rs:8 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- тест: crates/ivm/tests/syscalls_gas_names.rs:11 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- скамейка: crates/norito/benches/parity_compare.rs:79 — `let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- сборка: crates/norito/build.rs:17 — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- продукт: crates/norito/src/bin/norito_regen_goldens.rs:9 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/norito/tests/aos_ncb_more_golden.rs:200 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- тест: crates/norito/tests/json_golden_loader.rs:14 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/norito/tests/ncb_enum_iter_samples.rs:353 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/norito/tests/ncb_enum_iter_samples.rs:387 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/norito/tests/ncb_enum_iter_samples.rs:554 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- тест: crates/norito/tests/ncb_enum_iter_samples.rs:665 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- тест: crates/norito/tests/ncb_enum_large_fixture.rs:37 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- тест: crates/sorafs_car/src/bin/da_reconstruct.rs:434 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_car/src/bin/da_reconstruct.rs:710 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- продукт: crates/sorafs_car/src/bin/soranet_trustless_verifier.rs:140 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- тест: crates/sorafs_car/tests/capacity_simulation_toolkit.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_car/tests/fetch_cli.rs:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_car/tests/fetch_cli.rs:1043 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_car/tests/fetch_cli.rs:1159 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_car/tests/taikai_car_cli.rs:227 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_car/tests/taikai_viewer_cli.rs:22 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_car/tests/trustless_verifier.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- продукт: crates/sorafs_chunker/src/bin/export_vectors.rs:175 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/sorafs_chunker/tests/backpressure.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_chunker/tests/vectors.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_manifest/tests/por_fixtures.rs:11 — `env!("CARGO_MANIFEST_DIR"),`
- тест: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:12 — `cmd.current_dir(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_manifest/tests/replication_order_fixtures.rs:8 — `env!("CARGO_MANIFEST_DIR"),`
- продукт: crates/sorafs_node/src/bin/sorafs_gateway.rs:55 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- продукт: crates/sorafs_node/src/bin/sorafs_gateway.rs:59 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0")`
- тест: crates/sorafs_node/src/gateway.rs:2006 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");`
- тест: crates/sorafs_node/tests/cli.rs:122 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_node/tests/gateway.rs:14 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_node/tests/gateway.rs:30 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/sorafs_orchestrator/src/lib.rs:6316 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/sorafs_orchestrator/src/lib.rs:6449 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/sorafs_orchestrator/src/lib.rs:8100 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: crates/sorafs_orchestrator/tests/orchestrator_parity.rs:180 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: crates/soranet_pq/tests/kat_vectors.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- сборка: Integration_tests/build.rs:17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/src/sorafs_gateway_capability_refusal.rs:157 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- тест: Integration_tests/src/sorafs_gateway_conformance.rs:1032 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/address_canonicalisation.rs:126 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/asset.rs:49 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/fast_dsl_build.rs:7 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/genesis_json.rs:16 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/genesis_json.rs:22 — `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
- тест: Integration_tests/tests/iroha_cli.rs:24 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/ivm_header_decode.rs:49 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/ivm_header_smoke.rs:24 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/kotodama_examples.rs:70 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/kotodama_examples.rs:123 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/kotodama_examples.rs:173 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/nexus/cbdc_rollout_bundle.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/nexus/cbdc_whitelist.rs:26 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/nexus/global_commit.rs:17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- тест: Integration_tests/tests/nexus/lane_registry.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/norito_burn_fixture.rs:19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/repo.rs:31 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: Integration_tests/tests/streaming/mod.rs:339 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- продукт: mochi/mochi-core/src/supervisor.rs:220 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- продукт: mochi/mochi-core/src/supervisor.rs:538 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- тест: mochi/mochi-core/src/torii.rs:4740 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: mochi/mochi-core/src/torii.rs:4755 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: mochi/mochi-core/src/torii.rs:4770 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: mochi/mochi-core/src/torii.rs:4785 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: mochi/mochi-integration/tests/supervisor.rs:168 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- тест: инструменты/soranet-handshake-harness/tests/fixtures_verify.rs:6 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: инструменты/soranet-handshake-harness/tests/interop_parity.rs:77 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: инструменты/soranet-handshake-harness/tests/perf_gate.rs:173 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- инструмент: xtask/src/bin/control_plane_mock.rs:362 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- инструмент: xtask/src/main.rs:11383 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
— инструмент: xtask/src/sorafs/gateway_fixture.rs:28 — `env!("CARGO_MANIFEST_DIR"),`
- инструмент: xtask/src/sorafs/gateway_fixture.rs:32 — `env!("CARGO_MANIFEST_DIR"),`
- тест: xtask/tests/address_vectors.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/android_dashboard_parity_cli.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/codec_rans_tables.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/da_proof_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/iso_bridge_lint.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/ministry_agenda.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/sns_catalog_verify.rs:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soradns_cli.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/sorafs_fetch_fixture.rs:9 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_bug_bounty.rs:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_gateway_billing.rs:12 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_gateway_billing_m0.rs:30 — `let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_gateway_m1.rs:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_gateway_m2.rs:25 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_pop_template.rs:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_pop_template.rs:77 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_pop_template.rs:131 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_pop_template.rs:202 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_pop_template.rs:306 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_pop_template.rs:356 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/soranet_pop_template.rs:489 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/streaming_bundle_check.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/streaming_entropy_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_VERSION (производитель: 11, тест: 1, инструмент: 1)- продукт: crates/iroha/src/client.rs:478 — `map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));`
- продукт: crates/iroha_cli/src/commands/sorafs.rs:1767 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- продукт: crates/iroha_cli/src/main_shared.rs:72 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- продукт: crates/iroha_cli/src/main_shared.rs:530 — `let client_version = env!("CARGO_PKG_VERSION");`
- тест: crates/iroha_cli/tests/cli_smoke.rs:359 — `let expected_version = env!("CARGO_PKG_VERSION");`
- продукт: crates/iroha_core/src/sumeragi/rbc_store.rs:40 — `version: env!("CARGO_PKG_VERSION").to_owned(),`
- продукт: crates/iroha_js_host/src/lib.rs:2998 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- продукт: crates/iroha_telemetry/src/ws.rs:243 — `env!("CARGO_PKG_VERSION")`
- продукт: crates/irohad/src/main.rs:502 — `version = env!("CARGO_PKG_VERSION"),`
- продукт: crates/irohad/src/main.rs:3986 — `version = env!("CARGO_PKG_VERSION"),`
- продукт: crates/sorafs_car/src/bin/sorafs_fetch.rs:1111 — `Value::from(env!("CARGO_PKG_VERSION")),`
- продукт: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:93 — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
- инструмент: инструменты/телеметрия-схема-дифф/src/main.rs:248 — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## CARGO_PRIMARY_PACKAGE (сборка: 1)

- сборка: crates/soranet_pq/build.rs:6 — `if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {`

## CARGO_TARGET_DIR (продукт: 3, тест: 2, инструмент: 1)

- тест: crates/iroha_test_network/src/lib.rs:524 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- тест: crates/iroha_test_network/src/lib.rs:759 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- продукт: mochi/mochi-core/src/supervisor.rs:575 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- продукт: mochi/mochi-core/src/supervisor.rs:629 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- продукт: mochi/mochi-core/src/supervisor.rs:683 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- инструмент: xtask/src/mochi.rs:383 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## CARGO_WORKSPACE_DIR (тест: 1)

- тест: crates/iroha_core/src/state.rs:12015 — `if let Some(workspace_dir) = option_env!("CARGO_WORKSPACE_DIR") {`

## CRYPTO_SM_INTRINSICS (скамейка: 1)

- скамейка: crates/iroha_crypto/benches/sm_perf.rs:183 — `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`

## CUDA_HOME (сборка: 2)

- сборка: crates/fastpq_prover/build.rs:198 — `env::var_os("CUDA_HOME")`
- сборка: crates/norito/accelerators/jsonstage1_cuda/build.rs:63 — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (сборка: 2)

- сборка: crates/fastpq_prover/build.rs:199 — `.or_else(|| env::var_os("CUDA_PATH"))`
- сборка: crates/norito/accelerators/jsonstage1_cuda/build.rs:64 — `.or_else(|| env::var_os("CUDA_PATH"))`

## DATASPACE_ADVERSARIAL_ARTIFACT_DIR (тест: 1)

- тест: Integration_tests/tests/nexus/cross_lane.rs:686 — `if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {`

## DOCS_RS (сборка: 1)

- сборка: crates/norito/build.rs:8 — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (скамья: 1)

- скамейка: crates/norito/benches/enum_packed_bench.rs:75 — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_DEBUG_FUSED (прод.: 1)

- продукт: crates/fastpq_prover/src/trace.rs:98 — `Some(*DEBUG_FUSED_ENV.get_or_init(|| env::var_os("FASTPQ_DEBUG_FUSED").is_some()))`

## FASTPQ_EXPECTED_KIB (тест: 1)

- тест: crates/fastpq_prover/tests/perf_production.rs:88 — `env::var("FASTPQ_EXPECTED_KIB")`

## FASTPQ_EXPECTED_MS (тест: 1)

- тест: crates/fastpq_prover/tests/perf_production.rs:80 — `env::var("FASTPQ_EXPECTED_MS")`

## FASTPQ_PROOF_ROWS (тест: 1)

- тест: crates/fastpq_prover/tests/perf_production.rs:72 — `env::var("FASTPQ_PROOF_ROWS")`

## FASTPQ_SKIP_GPU_BUILD (сборка: 1)

- сборка: crates/fastpq_prover/build.rs:45 — `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## FASTPQ_UPDATE_FIXTURES (тест: 6)- тест: crates/fastpq_prover/tests/backend_reгрессия.rs:47 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
— тест: crates/fastpq_prover/tests/backend_reгрессия.rs:69 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
— тест: crates/fastpq_prover/tests/proof_fixture.rs:43 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- тест: crates/fastpq_prover/tests/trace_commitment.rs:23 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- тест: crates/fastpq_prover/tests/trace_commitment.rs:111 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- тест: crates/fastpq_prover/tests/transcript_replay.rs:67 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`

## GENESIS_DEBUG_MODE (тест: 1)

- тест: crates/iroha_test_network/examples/genesis_debug.rs:15 — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## GENESIS_DEBUG_PAYLOAD (тест: 1)

- тест: crates/iroha_test_network/examples/genesis_debug.rs:122 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## GITHUB_STEP_SUMMARY (продукция: 2)

- продукт: crates/iroha_crypto/src/bin/gost_perf_check.rs:22 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- продукт: crates/iroha_crypto/src/bin/sm_perf_check.rs:205 — `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## GIT_COMMIT_HASH (прод.: 1)

- продукт: crates/iroha_core/src/sumeragi/rbc_store.rs:42 — `git_commit: option_env!("GIT_COMMIT_HASH").map(str::to_owned),`

## ДОМ (продукция: 1)

- продукт: crates/iroha/src/config.rs:57 — `env::var_os("HOME").map(PathBuf::from)`

## IROHA_ALLOW_NET (тест: 1)

- тест: crates/izanami/src/chaos.rs:374 — `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## IROHA_CONF_GAS_SEED (тест: 1)

- тест: crates/iroha_test_samples/src/lib.rs:57 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_DA_SPOOL_DIR (тест: 1)

- тест: crates/iroha_core/src/state.rs:9096 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_METRICS_PANIC_ON_DUPLICATE (тест: 2)

- тест: crates/iroha_telemetry/src/metrics.rs:11397 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- тест: crates/iroha_torii/tests/metrics_registry.rs:33 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## IROHA_RUN_IGNORED (тест: 71)- тест: crates/iroha_core/tests/gov_auto_close_approve.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_finalize_real_vk.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_min_duration.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_mode_mismatch.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_mode_mismatch_zk.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_plain_ballot.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_plain_conviction.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_plain_disabled.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_plain_missing_ref.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_plain_revote_monotonic.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_protected_gate.rs:64 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_referendum_open_close.rs:24 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_thresholds.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_thresholds.rs:82 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_thresholds_positive.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_unlock_sweep.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_zk_ballot_lock_verified.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_zk_ballot_real_vk.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/gov_zk_referendum_window_guard.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/zk_roots_get_cap.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_core/tests/zk_vote_get_tally.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_crypto/src/merkle.rs:1317 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_crypto/src/merkle.rs:1332 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_crypto/tests/merkle_norito_roundtrip.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_crypto/tests/merkle_norito_roundtrip.rs:42 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/src/block/header.rs:624 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/src/isi/mod.rs:1888 — `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- тест: crates/iroha_data_model/src/isi/register.rs:324 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/src/proof.rs:798 — `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- тест: crates/iroha_data_model/src/transaction/signed.rs:1048 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/instruction_registry_lazy_init.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/instruction_registry_reset.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/model_derive_repro.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/model_derive_repro.rs:37 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/model_derive_repro.rs:58 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/model_derive_repro.rs:84 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/registry_decode_roundtrip.rs:9 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/trait_objects.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_data_model/tests/trait_objects.rs:27 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`- тест: crates/iroha_data_model/tests/zk_envelope_roundtrip.rs:6 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/contracts_activate_integration.rs:23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/contracts_activate_integration.rs:174 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/contracts_call_integration.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/contracts_deploy_integration.rs:23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/contracts_instance_activate_integration.rs:19 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/contracts_instances_list_router.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/gov_council_persist_integration.rs:22 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/gov_council_vrf.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/gov_enact_handler.rs:12 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/gov_instances_list.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/gov_mode_mismatch_and_autoclose.rs:42 — `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- тест: crates/iroha_torii/tests/gov_protected_endpoints.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/gov_protected_endpoints_router.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/iroha_torii/tests/gov_read_endpoints_router.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/ivm/tests/beep_test.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/ivm/tests/kotodama_struct_fields.rs:11 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:50 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/events/notification.rs:81 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/extra_functional/unstable_network.rs:753 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/extra_functional/unstable_network.rs:769 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/extra_functional/unstable_network.rs:785 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/extra_functional/unstable_network.rs:801 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/extra_functional/unstable_network.rs:817 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/permissions.rs:202 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/permissions.rs:265 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/permissions.rs:328 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/pipeline_block_rejected.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/sorting.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/triggers/by_call_trigger.rs:139 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: Integration_tests/tests/triggers/time_trigger.rs:149 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_RUN_ZK_WRAPPERS (тест: 1)

- тест: crates/ivm/tests/kotodama_wrappers.rs:2 — `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`

## IROHA_SKIP_BIND_CHECKS (тест: 1)

- тест: crates/iroha_test_network/src/lib.rs:3098 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (тест: 1)

- тест: crates/iroha_crypto/tests/sm_cli_matrix.rs:47 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_TEST_DUMP_GENESIS (тест: 1)

- тест: crates/iroha_test_network/src/lib.rs:6661 — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (сборка: 1, тест: 1)

- тест: crates/iroha_test_network/src/config.rs:155 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- сборка: Integration_tests/build.rs:205 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_SKIP_BUILD (тест: 1)

- тест: crates/iroha_test_network/src/lib.rs:1244 — `std::env::var("IROHA_TEST_SKIP_BUILD")`

## IROHA_TEST_TARGET_DIR (тест: 2)

- тест: crates/iroha_test_network/src/lib.rs:521 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`
- тест: crates/iroha_test_network/src/lib.rs:765 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`

## IROHA_TEST_USE_DEFAULT_EXECUTOR (тест: 3)

- тест: crates/iroha_core/src/executor.rs:2383 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- тест: crates/iroha_core/src/executor.rs:2520 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- тест: crates/iroha_core/src/state.rs:12008 — `if std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR").is_some() {`

## IROHA_TORII_OPENAPI_ACTUAL (тест: 1)

- тест: crates/iroha_torii/tests/router_feature_matrix.rs:94 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_EXPECTED (тест: 2)

- тест: crates/iroha_torii/tests/router_feature_matrix.rs:88 — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- тест: crates/iroha_torii/tests/router_feature_matrix.rs:104 — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_TORII_OPENAPI_TOKENS (инструмент: 2)

- инструмент: xtask/src/main.rs:11025 — `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`
- инструмент: xtask/src/main.rs:11077 — `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IVM_BIN (тест: 2)

- тест: Integration_tests/tests/kotodama_examples.rs:60 — `let ivm_bin = env::var("IVM_BIN")`
- тест: Integration_tests/tests/kotodama_examples.rs:163 — `let ivm_bin = env::var("IVM_BIN")`

## IVM_COMPILER_DEBUG (прод.: 1)

- продукт: crates/kotodama_lang/src/compiler.rs:4094 — `let compiler_debug = if std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_GENCODE (сборка: 1)

- сборка: crates/ivm/build.rs:35 — `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| "arch=compute_61,code=sm_61".to_string());`

## IVM_CUDA_NVCC (сборка: 1)

- сборка: crates/ivm/build.rs:30 — `let nvcc = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVCC_EXTRA (сборка: 1)

- сборка: crates/ivm/build.rs:36 — `let extra_flags: Vec<String> = env::var("IVM_CUDA_NVCC_EXTRA")`

## IVM_DEBUG_IR (тест: 1)

- тест: crates/ivm/tests/debug_contains.rs:15 — `if std::env::var_os("IVM_DEBUG_IR").is_some() {`

## IVM_DEBUG_METAL_ENUM (отладка: 1)

- отладка: crates/ivm/src/vector.rs:474 — `std::env::var("IVM_DEBUG_METAL_ENUM")`

## IVM_DEBUG_METAL_SELFTEST (отладка: 1)

- отладка: crates/ivm/src/vector.rs:1205 — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DISABLE_CUDA (отладка: 1)

- отладка: crates/ivm/src/cuda.rs:315 — `&& std::env::var("IVM_DISABLE_CUDA")`

## IVM_DISABLE_METAL (отладка: 1)

- отладка: crates/ivm/src/vector.rs:299 — `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELFTEST_FAIL (отладка: 1)

- отладка: crates/ivm/src/cuda.rs:326 — `&& std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL")`

## IVM_FORCE_METAL_ENUM (отладка: 1)

- отладка: crates/ivm/src/vector.rs:444 — `std::env::var("IVM_FORCE_METAL_ENUM")`

## IVM_FORCE_METAL_SELFTEST_FAIL (отладка: 1)

- отладка: crates/ivm/src/vector.rs:1192 — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IVM_TOOL_BIN (тест: 1)

- тест: Integration_tests/tests/kotodama_examples.rs:113 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## IZANAMI_ALLOW_NET (тест: 1)

- тест: crates/izanami/src/chaos.rs:373 — `std::env::var("IZANAMI_ALLOW_NET")`

## IZANAMI_TUI_ALLOW_ZERO_SEED (прод.: 1)

- продукт: crates/izanami/src/tui.rs:134 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (сборка: 1)

- сборка: crates/norito/accelerators/jsonstage1_cuda/build.rs:40 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_SKIP_BUILD (сборка: 1)

- сборка: crates/norito/accelerators/jsonstage1_cuda/build.rs:18 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KOTO_BIN (тест: 2)- тест: Integration_tests/tests/kotodama_examples.rs:50 — `let koto_bin = env::var("KOTO_BIN")`
- тест: Integration_tests/tests/kotodama_examples.rs:154 — `let koto_bin = env::var("KOTO_BIN")`

##ЯЗЫК (тест: 3)

- тест: crates/ivm/src/bin/koto_lint.rs:702 — `let previous = env::var("LANG").ok();`
- тест: crates/ivm/tests/i18n.rs:11 — `let old_lang = env::var("LANG").ok();`
- тест: crates/ivm/tests/i18n.rs:69 — `let old_lang = env::var("LANG").ok();`

## LC_ALL (тест: 2)

- тест: crates/ivm/tests/i18n.rs:12 — `let old_lc_all = env::var("LC_ALL").ok();`
- тест: crates/ivm/tests/i18n.rs:70 — `let old_lc_all = env::var("LC_ALL").ok();`

## LC_MESSAGES (тест: 2)

- тест: crates/ivm/tests/i18n.rs:13 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`
- тест: crates/ivm/tests/i18n.rs:71 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## MAX_DEGREE (прод.: 1)

- продукт: crates/iroha_core/src/zk.rs:106 — `let current = std::env::var("MAX_DEGREE")`

## MOCHI_CONFIG (прод.: 1)

- продукт: mochi/mochi-ui-egui/src/config.rs:325 — `if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## MOCHI_DATA_ROOT (прод.: 1)

- продукт: mochi/mochi-core/src/supervisor.rs:2068 — `std::env::var_os("MOCHI_DATA_ROOT")`

## MOCHI_TEST_USE_INTERNAL_GENESIS (производитель: 1)

- продукт: mochi/mochi-core/src/supervisor.rs:1994 — `if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {`

## NORITO_BENCH_SUMMARY (скамья: 1)

- скамейка: crates/norito/benches/parity_compare.rs:70 — `if std::env::var("NORITO_BENCH_SUMMARY").ok().as_deref() == Some("1") {`

## NORITO_CPU_INFO (инструмент: 1)

- инструмент: xtask/src/stage1_bench.rs:69 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_GPU_LIB (производитель: 1)

- продукт: crates/norito/src/core/simd_crc64.rs:215 — `std::env::var("NORITO_CRC64_GPU_LIB").ok(),`

## NORITO_DISABLE_PACKED_STRUCT (производство: 1, тестирование: 1)

- продукт: crates/iroha_js_host/src/lib.rs:183 — `let env_set = std::env::var_os("NORITO_DISABLE_PACKED_STRUCT").is_some();`
- тест: crates/norito/src/lib.rs:322 — `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## NORITO_GPU_CRC64_MIN_BYTES (прод.: 1)

- продукт: crates/norito/src/core/simd_crc64.rs:83 — `std::env::var("NORITO_GPU_CRC64_MIN_BYTES").ok(),`

## NORITO_PAR_STAGE1_MIN (тест: 1)

- тест: crates/norito/src/lib.rs:4847 — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## NORITO_SKIP_BINDINGS_SYNC (сборка: 1)

- сборка: crates/norito/build.rs:12 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_GPU_MIN_BYTES (тест: 1)

- тест: crates/norito/src/lib.rs:4881 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## NORITO_TRACE (тест: 2)

- тест: crates/norito/src/lib.rs:123 — `std::env::var_os("NORITO_TRACE").is_some()`
- тест: crates/norito/src/lib.rs:138 — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## NO_PROXY (прод.: 1)

- продукт: crates/iroha_p2p/src/transport.rs:469 — `let no_proxy = env::var("NO_PROXY")`

## NVCC (сборка: 1)

- сборка: crates/ivm/build.rs:31 — `.or_else(|_| env::var("NVCC"))`

## OUT_DIR (сборка: 4, продукция: 12, тест: 2)- сборка: crates/fastpq_prover/build.rs:99 — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
- сборка: crates/iroha_data_model/build.rs:12 — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));`
- продукт: crates/iroha_data_model/src/lib.rs:179 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
- сборка: crates/ivm/build.rs:27 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- сборка: crates/ivm/build.rs:120 — `if let Some(out_dir) = env::var_os("OUT_DIR") {`
- продукт: crates/ivm/src/cuda.rs:12 — `static PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- продукт: crates/ivm/src/cuda.rs:13 — `static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));`
- продукт: crates/ivm/src/cuda.rs:14 — `static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));`
- продукт: crates/ivm/src/cuda.rs:15 — `static SHA_LEAVES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx"));`
- продукт: crates/ivm/src/cuda.rs:16 — `static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));`
- продукт: crates/ivm/src/cuda.rs:17 — `static SHA3_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx"));`
- продукт: crates/ivm/src/cuda.rs:18 — `static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));`
- продукт: crates/ivm/src/cuda.rs:19 — `static BN254_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx"));`
- продукт: crates/ivm/src/cuda.rs:20 — `static SIG_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/signature.ptx"));`
- продукт: crates/ivm/src/cuda.rs:21 — `static SHA_PAIRS_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx"));`
- продукт: crates/ivm/src/cuda.rs:22 — `static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));`
- тест: crates/ivm/src/ptx_tests.rs:7 — `let out_dir = match std::env::var("OUT_DIR") {`
- тест: crates/ivm/tests/ptx_kernels.rs:5 — `let out_dir = env!("OUT_DIR");`

## P2P_TURN (прод.: 1)

- продукт: crates/iroha_p2p/src/transport.rs:296 — `let endpoint = std::env::var("P2P_TURN")`

## PATH (производство: 2, тестирование: 1)

- продукт: crates/iroha_cli/src/commands/sorafs.rs:7276 — `if let Some(path_var) = env::var_os("PATH") {`
- тест: Integration_tests/tests/kotodama_examples.rs:17 — `let path = env::var_os("PATH")?;`
- продукт: mochi/mochi-core/src/supervisor.rs:511 — `let path_var = env::var_os("PATH")?;`

## PRINT_SORACLES_FIXTURES (тест: 1)

- тест: crates/iroha_data_model/src/oracle/mod.rs:3249 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORII_SPEC (тест: 1)

- тест: crates/iroha_torii/src/openapi.rs:4380 — `if std::env::var("PRINT_TORII_SPEC").is_ok() {`

## ПРОФИЛЬ (сборка: 1, продукт: 1, тест: 1)

- продукт: crates/iroha_core/src/sumeragi/rbc_store.rs:41 — `profile: option_env!("PROFILE").unwrap_or("unknown").to_owned(),`
- тест: crates/iroha_test_network/src/lib.rs:995 — `let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());`
- сборка: Integration_tests/build.rs:180 — `let profile = match env::var("PROFILE").unwrap_or_default().as_str() {`

## PYTHON3 (тест: 2)

- тест: crates/sorafs_car/tests/taikai_car_cli.rs:235 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`
- тест: crates/sorafs_car/tests/taikai_viewer_cli.rs:30 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONPATH (тест: 1)

- тест: crates/iroha_cli/tests/cli_smoke.rs:5245 — `match env::var("PYTHONPATH") {`

## RBC_SESSION_PATH (тест: 1)

- тест: crates/iroha_core/src/sumeragi/rbc_store.rs:823 — `let path = std::env::var("RBC_SESSION_PATH").expect("set RBC_SESSION_PATH");`

## REPO_PROOF_DIGEST_OUT (тест: 1)

- тест: crates/iroha_core/src/smartcontracts/isi/repo.rs:2017 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (тест: 1)

- тест: crates/iroha_core/src/smartcontracts/isi/repo.rs:2005 — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUST_LOG (прод: 1, тест: 2)

- тест: crates/iroha_test_network/src/lib.rs:5414 — `let original = env::var("RUST_LOG").ok();`
- тест: crates/iroha_test_network/src/lib.rs:5431 — `let original = env::var("RUST_LOG").ok();`
- продукт: crates/izanami/src/config.rs:269 — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`

## SM_PERF_CPU_LABEL (прод.: 2)

- продукт: crates/iroha_crypto/src/bin/sm_perf_check.rs:637 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- продукт: crates/iroha_crypto/src/bin/sm_perf_check.rs:679 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## SORAFS_NODE_SKIP_INGEST_TESTS (тест: 1)

- тест: crates/sorafs_node/tests/cli.rs:17 — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORAFS_TORII_SKIP_INGEST_TESTS (тест: 1)

- тест: crates/iroha_torii/tests/sorafs_discovery.rs:95 — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`## SUMERAGI_ADVERSARIAL_ARTIFACT_DIR (тест: 1)

- тест: Integration_tests/tests/sumeragi_adversarial.rs:1222 — `let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {`

## SUMERAGI_BASELINE_ARTIFACT_DIR (продукт: 1, тест: 1)

- продукт: crates/build-support/src/bin/sumeragi_baseline_report.rs:40 — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`
- тест: Integration_tests/tests/sumeragi_npos_ Performance.rs:1396 — `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`

## SUMERAGI_DA_ARTIFACT_DIR (производство: 1, тестирование: 1)

- продукт: crates/build-support/src/bin/sumeragi_da_report.rs:41 — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`
- тест: Integration_tests/tests/sumeragi_da.rs:1599 — `let Ok(dir) = std::env::var("SUMERAGI_DA_ARTIFACT_DIR") else {`

## SystemRoot (прод: 1)

- продукт: crates/fastpq_prover/src/backend.rs:535 — `env::var_os("SystemRoot").map(PathBuf::from)`

## ЦЕЛЬ (инструмент: 2)

- инструмент: xtask/src/poseidon_bench.rs:87 — `target: std::env::var("TARGET")`
- инструмент: xtask/src/stage1_bench.rs:63 — `target: std::env::var("TARGET")`

## TEST_LOG_FILTER (произведение: 1)

- продукт: crates/iroha_logger/src/lib.rs:89 — `filter: std::env::var("TEST_LOG_FILTER")`

## TEST_LOG_LEVEL (прод.: 1)

- продукт: crates/iroha_logger/src/lib.rs:85 — `level: std::env::var("TEST_LOG_LEVEL")`

## TEST_NETWORK_CARGO (тест: 1)

- тест: crates/iroha_test_network/src/lib.rs:865 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TORII_DEBUG_SORT (прод.: 1)

- продукт: crates/iroha_torii/src/routing.rs:12530 — `std::env::var("TORII_DEBUG_SORT").ok().is_some(),`

## TORII_MOCK_HARNESS_METRICS_PATH (инструмент: 1)

— инструмент: xtask/src/bin/torii_mock_harness.rs:106 — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (инструмент: 1)

- инструмент: xtask/src/bin/torii_mock_harness.rs:109 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_RETRY_TOTAL (инструмент: 1)

- инструмент: xtask/src/bin/torii_mock_harness.rs:297 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (инструмент: 1)

- инструмент: xtask/src/bin/torii_mock_harness.rs:112 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (инструмент: 1)

— инструмент: xtask/src/bin/torii_mock_harness.rs:104 — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## TORII_OPENAPI_TOKEN (инструмент: 2)

- инструмент: xtask/src/main.rs:11020 — `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`
- инструмент: xtask/src/main.rs:11073 — `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## UPDATE_FIXTURES (тест: 1)

- тест: crates/iroha_core/tests/snapshots.rs:42 — `let update = env::var("UPDATE_FIXTURES")`

## ПРОФИЛЬ ПОЛЬЗОВАТЕЛЯ (прод.: 1)

- продукт: crates/iroha/src/config.rs:55 — `env::var_os("USERPROFILE").map(PathBuf::from)`

## VERGEN_CARGO_FEATURES (продукция: 1)

- продукт: crates/irohad/src/main.rs:4850 — `const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {`

## VERGEN_CARGO_TARGET_TRIPLE (производство: 1)

- продукт: crates/iroha_telemetry/src/ws.rs:238 — `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");`

## VERGEN_GIT_SHA (прод.: 4)

- продукт: crates/iroha_cli/src/main_shared.rs:57 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`
- продукт: crates/iroha_telemetry/src/ws.rs:237 — `let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");`
- продукт: crates/iroha_torii/src/routing.rs:32515 — `git_sha: option_env!("VERGEN_GIT_SHA")`
- продукт: crates/irohad/src/main.rs:4845 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`

## VERIFY_BATCH (тест: 1)

- скамейка: crates/ivm/benches/bench_voting.rs:225 — `let verify_batch = std::env::var("VERIFY_BATCH")`

## VERIFY_EVERY (скамейка: 1)

- скамейка: crates/ivm/benches/bench_voting.rs:213 — `let verify_every: u64 = std::env::var("VERIFY_EVERY")`

## ИЗБИРАТЕЛЕЙ (скамейка: 1)

- скамейка: crates/ivm/benches/bench_voting.rs:207 — `let voters: u64 = std::env::var("VOTERS")`

## no_proxy (прод.: 1)

- продукт: crates/iroha_p2p/src/transport.rs:471 — `.or_else(|| env::var("no_proxy").ok());`