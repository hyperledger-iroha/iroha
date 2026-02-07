---
lang: kk
direction: ltr
source: docs/source/agents/env_var_inventory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0a23896374a10f0862aeec69c607ba3022d3b337ae7f6bb54a61b8f7424410
source_last_modified: "2026-01-21T19:17:13.235848+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Қоршаған орта инвентаризациясы

_Соңғы рет `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md` арқылы жаңартылды

Жалпы сілтемелер: **505** · Бірегей айнымалылар: **137**

## ӘРЕКЕТТЕР_ID_TOKEN_REQUEST_TOKEN (өнім: 1)

- өнім: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:313 — `let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {`

## ӘРЕКЕТТЕР_ID_TOKEN_REQUEST_URL (өнім: 1)

- өнім: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:310 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`

## БЛОК_ТӨГІМДЕРІ_БИІКТЕРІ (мысал: 1)

- мысал: crates/iroha_core/examples/block_dump.rs:62 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`

## BLOCK_DUMP_SUM_ASSET (мысал: 1)

- мысал: жәшіктер/iroha_core/examples/block_dump.rs:51 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`

## BLOCK_DUMP_VERBOSE (мысалы: 1)

- мысал: crates/iroha_core/examples/block_dump.rs:50 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`

## ЖҮК (өнім: 3, сынақ: 2)

- сынақ: crates/iroha_test_network/src/lib.rs:1026 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- сынақ: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:11 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`
- өнім: mochi/mochi-core/src/supervisor.rs:547 — `let cargo = env::var_os("CARGO")`
- өнім: mochi/mochi-core/src/supervisor.rs:602 — `let cargo = env::var_os("CARGO")`
- өнім: mochi/mochi-core/src/supervisor.rs:656 — `let cargo = env::var_os("CARGO")`

## CARGO_BIN_EXE_iroha (сынақ: 2)

- сынақ: crates/iroha_cli/tests/cli_smoke.rs:40 — `env!("CARGO_BIN_EXE_iroha")`
- сынақ: crates/iroha_cli/tests/taikai_policy.rs:20 — `env!("CARGO_BIN_EXE_iroha")`

## CARGO_BIN_EXE_iroha_мониторы (сынақ: 4)

- сынақ: crates/iroha_monitor/tests/attach_render.rs:11 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- сынақ: crates/iroha_monitor/tests/http_limits.rs:10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- сынақ: crates/iroha_monitor/tests/invalid_credentials.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- сынақ: жәшіктер/iroha_monitor/tests/smoke.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## CARGO_BIN_EXE_kagami (тест: 2)

- сынақ: crates/iroha_kagami/tests/common/mod.rs:22 — `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- сынақ: crates/iroha_kagami/tests/pop_embed.rs:34 — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`

## CARGO_BIN_EXE_kagami_mock (сынақ: 1)

- тест: mochi/mochi-integration/tests/supervisor.rs:33 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## CARGO_BIN_EXE_koto_compile (сынақ: 3)

- сынақ: crates/ivm/tests/cli_smoke.rs:8 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- сынақ: crates/ivm/tests/cli_smoke.rs:56 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- сынақ: crates/ivm/tests/cli_smoke.rs:88 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`

## CARGO_BIN_EXE_sorafs_chunk_dump (сынақ: 1)

- сынақ: crates/sorafs_chunker/tests/one_gib.rs:103 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## CARGO_BIN_EXE_sorafs_cli (сынақ: 1)

- сынақ: crates/sorafs_car/tests/sorafs_cli.rs:42 — `let path = env::var("CARGO_BIN_EXE_sorafs_cli")`

## CARGO_BIN_EXE_sorafs_алу (сынақ: 1)

- сынақ: crates/sorafs_car/src/bin/sorafs_fetch.rs:2831 — `if let Ok(path) = env::var("CARGO_BIN_EXE_sorafs_fetch") {`

## CARGO_BIN_EXE_taikai_car (сынақ: 1)

- сынақ: crates/sorafs_car/tests/sorafs_cli.rs:48 — `let path = env::var("CARGO_BIN_EXE_taikai_car")`

## ЖҮК_БАҚ_АТЫ (өнім: 3)

- өнім: crates/iroha_cli/src/main_shared.rs:63 — `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`
- өнім: crates/iroha_cli/src/main_shared.rs:72 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- өнім: crates/irohad/src/main.rs:3176 — `let build_line = BuildLine::from_bin_name(env!("CARGO_BIN_NAME"));`

## ЖҮКТІ_ҚҰРУ_МАҚСАТ (құрал: 2)

- құрал: xtask/src/poseidon_bench.rs:88 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- құрал: xtask/src/stage1_bench.rs:64 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_TARGET_ARCH (өнім: 2, құрал: 2)- өнім: crates/iroha_crypto/src/bin/sm_perf_check.rs:632 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- өнім: crates/iroha_crypto/src/bin/sm_perf_check.rs:668 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- құрал: xtask/src/poseidon_bench.rs:89 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- құрал: xtask/src/stage1_bench.rs:65 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## CARGO_CFG_TARGET_OS (құрылым: 1, өнім: 2, құрал: 2)

- құрастыру: жәшіктер/fastpq_prover/build.rs:27 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- өнім: crates/iroha_crypto/src/bin/sm_perf_check.rs:633 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- өнім: crates/iroha_crypto/src/bin/sm_perf_check.rs:669 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- құрал: xtask/src/poseidon_bench.rs:91 — `os: std::env::var("CARGO_CFG_TARGET_OS")`
- құрал: xtask/src/stage1_bench.rs:67 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## ЖҮК_МҮМКІНДІГІ_CUDA (құрылым: 2)

- құрастыру: жәшіктер/fastpq_prover/build.rs:25 — `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
- құрастыру: crates/ivm/build.rs:12 — `if env::var_os("CARGO_FEATURE_CUDA").is_some()`

## CARGO_FEATURE_CUDA_CERNEL (құрылым: 1)

- құрастыру: crates/norito/accelerators/jsonstage1_cuda/build.rs:12 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (құрылым: 1)

- құрастыру: жәшіктер/fastpq_prover/build.rs:26 — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## ЖҮКТІ_МҮМКІНДІК_ФФИ_ЭКСПОРТ (өнім: 1)

- өнім: crates/build-support/src/lib.rs:30 — `let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();`

## ЖҮКТІ_МҮМКІНДІК_ФФИ_ИМПОРТ (өнім: 1)

- өнім: crates/build-support/src/lib.rs:29 — `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`

## CARGO_MANIFEST_DIR (стенд: 4, құрастыру: 5, мысал: 1, өнім: 27, сынақ: 164, құрал: 4)- өнім: crates/fastpq_prover/src/poseidon_manifest.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/fastpq_prover/tests/packing.rs:17 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:9 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- сынақ: crates/fastpq_prover/tests/poseidon_manifest_consistency.rs:28 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- сынақ: crates/fastpq_prover/tests/proof_fixture.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/fastpq_prover/tests/trace_commitment.rs:14 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- сынақ: crates/fastpq_prover/tests/transcript_replay.rs:56 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha/src/client.rs:8409 — `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha/src/sm.rs:201 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha/tests/sm_signing.rs:35 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_cli/src/compute.rs:518 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- өнім: crates/iroha_cli/src/main_shared.rs:766 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_cli/tests/cli_smoke.rs:111 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_cli/tests/cli_smoke.rs:5236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- өнім: crates/iroha_config/src/parameters/user.rs:3490 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_config/src/parameters/user.rs:11561 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_config/tests/fastpq_queue_overrides.rs:15 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_config/tests/fixtures.rs:36 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- орындық: жәшіктер/iroha_core/benches/blocks/common.rs:261 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- орындық: жәшіктер/iroha_core/benches/blocks/common/mod.rs:272 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- орындық: жәшіктер/iroha_core/benches/validation.rs:96 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- құрастыру: crates/iroha_core/build.rs:19 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- мысал: crates/iroha_core/examples/generate_parity_fixtures.rs:17 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_core/src/executor.rs:2385 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- сынақ: crates/iroha_core/src/executor.rs:2521 — `let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_core/src/executor.rs:2661 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- өнім: crates/iroha_core/src/smartcontracts/isi/offline.rs:2152 — `env!("CARGO_MANIFEST_DIR"),`
- өнім: crates/iroha_core/src/smartcontracts/isi/offline.rs:2156 — `env!("CARGO_MANIFEST_DIR"),`
- өнім: crates/iroha_core/src/smartcontracts/isi/offline.rs:2164 — `env!("CARGO_MANIFEST_DIR"),`
- өнім: crates/iroha_core/src/smartcontracts/isi/offline.rs:2171 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_core/src/smartcontracts/isi/offline.rs:4852 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_core/src/smartcontracts/isi/repo.rs:1870 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_core/src/smartcontracts/isi/repo.rs:1874 — `env!("CARGO_MANIFEST_DIR"),`
- өнім: crates/iroha_core/src/state.rs:9990 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- сынақ: crates/iroha_core/src/state.rs:12012 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_core/src/streaming.rs:2980 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_core/src/tx.rs:4424 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_core/tests/executor_migration_introspect.rs:25 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_core/tests/pin_registry.rs:1158 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_core/tests/snapshots.rs:29 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_core/tests/sumeragi_doc_sync.rs:57 — `Path::new(env!("CARGO_MANIFEST_DIR"))`- сынақ: crates/iroha_crypto/tests/confidential_keyset_vectors.rs:57 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_crypto/tests/sm2_fixture_vectors.rs:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_crypto/tests/sm_cli_matrix.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- құрастыру: жәшіктер/iroha_data_model/build.rs:11 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");`
- өнім: crates/iroha_data_model/src/lib.rs:186 — `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- өнім: crates/iroha_data_model/src/lib.rs:190 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/src/offline/poseidon.rs:449 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_data_model/src/soranet/vpn.rs:1217 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- сынақ: жәшіктер/iroha_data_model/tests/account_address_vectors.rs:147 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_data_model/tests/address_curve_registry.rs:33 — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:48 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_data_model/tests/confidential_wallet_fixtures.rs:15 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_data_model/tests/consensus_roundtrip.rs:1308 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_data_model/tests/offline_fixtures.rs:130 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:29 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:33 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:37 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:41 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:46 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:50 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:54 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:58 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:62 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_data_model/tests/oracle_reference_fixtures.rs:200 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_data_model/tests/runtime_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_genesis/src/lib.rs:1173 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- сынақ: crates/iroha_genesis/src/lib.rs:3847 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- сынақ: crates/iroha_genesis/src/lib.rs:4228 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- сынақ: crates/iroha_genesis/src/lib.rs:4243 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- сынақ: crates/iroha_i18n/src/lib.rs:495 — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- сынақ: crates/iroha_js_host/src/lib.rs:6943 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- өнім: crates/iroha_kagami/samples/codec/generate.rs:13 — `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- өнім: crates/iroha_kagami/samples/codec/src/main.rs:35 — `let dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_kagami/src/codec.rs:393 — `env!("CARGO_MANIFEST_DIR"),`
- өнім: crates/iroha_kagami/src/localnet.rs:604 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_kagami/tests/codec.rs:11 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- сынақ: crates/iroha_telemetry/tests/drill_log.rs:10 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/iroha_test_network/src/config.rs:724 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- сынақ: crates/iroha_test_network/src/fslock_ports.rs:23 — `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- сынақ: crates/iroha_test_network/src/fslock_ports.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_test_network/src/lib.rs:281 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_test_samples/src/lib.rs:204 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_test_samples/src/lib.rs:241 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_torii/src/da/tests.rs:3490 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- сынақ: crates/iroha_torii/src/sorafs/api.rs:4053 — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_torii/tests/account_address_vectors.rs:140 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/iroha_torii/tests/accounts_portfolio.rs:91 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/iroha_torii/tests/sorafs_discovery.rs:1056 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- құрастыру: crates/ivm/build.rs:21 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- өнім: crates/ivm/src/bin/gen_abi_hash_doc.rs:28 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- өнім: crates/ivm/src/bin/gen_header_doc.rs:48 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- өнім: crates/ivm/src/bin/gen_pointer_types_doc.rs:23 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- өнім: crates/ivm/src/bin/gen_syscalls_doc.rs:24 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- өнім: crates/ivm/src/bin/ivm_prebuild.rs:16 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- өнім: crates/ivm/src/bin/ivm_predecoder_export.rs:23 — `let _crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- өнім: crates/ivm/src/predecoder_fixtures.rs:236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- сынақ: crates/ivm/tests/cli_smoke.rs:9 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- сынақ: crates/ivm/tests/cli_smoke.rs:57 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- сынақ: crates/ivm/tests/cli_smoke.rs:89 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- сынақ: crates/ivm/tests/docs_consistency.rs:3 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- сынақ: crates/ivm/tests/ivm_abi_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/ivm/tests/ivm_header_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/ivm/tests/norito_portal_snippets_compile.rs:19 — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/ivm/tests/pointer_types_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- сынақ: crates/ivm/tests/pointer_types_doc_generated_ivm_md.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/ivm/tests/syscalls_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- сынақ: crates/ivm/tests/syscalls_doc_sync.rs:8 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- сынақ: crates/ivm/tests/syscalls_gas_names.rs:11 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- орындық: жәшіктер/norito/benches/parity_compare.rs:79 — `let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- құрастыру: жәшіктер/norito/build.rs:17 — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- өнім: crates/norito/src/bin/norito_regen_goldens.rs:9 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/norito/tests/aos_ncb_more_golden.rs:200 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- сынақ: crates/norito/tests/json_golden_loader.rs:14 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/norito/tests/ncb_enum_iter_samples.rs:353 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/norito/tests/ncb_enum_iter_samples.rs:387 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/norito/tests/ncb_enum_iter_samples.rs:554 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- сынақ: crates/norito/tests/ncb_enum_iter_samples.rs:665 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- сынақ: crates/norito/tests/ncb_enum_large_fixture.rs:37 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- сынақ: crates/sorafs_car/src/bin/da_reconstruct.rs:434 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_car/src/bin/da_reconstruct.rs:710 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- өнім: crates/sorafs_car/src/bin/soranet_trustless_verifier.rs:140 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- сынақ: crates/sorafs_car/tests/capacity_simulation_toolkit.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_car/tests/fetch_cli.rs:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_car/tests/fetch_cli.rs:1043 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_car/tests/fetch_cli.rs:1159 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_car/tests/taikai_car_cli.rs:227 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_car/tests/taikai_viewer_cli.rs:22 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_car/tests/trustless_verifier.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- өнім: crates/sorafs_chunker/src/bin/export_vectors.rs:175 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/sorafs_chunker/tests/backpressure.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_chunker/tests/vectors.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_manifest/tests/por_fixtures.rs:11 — `env!("CARGO_MANIFEST_DIR"),`
- сынақ: crates/sorafs_manifest/tests/provider_admission_fixtures.rs:12 — `cmd.current_dir(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_manifest/tests/replication_order_fixtures.rs:8 — `env!("CARGO_MANIFEST_DIR"),`
- өнім: crates/sorafs_node/src/bin/sorafs_gateway.rs:55 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- өнім: crates/sorafs_node/src/bin/sorafs_gateway.rs:59 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0")`
- сынақ: crates/sorafs_node/src/gateway.rs:2006 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");`
- сынақ: crates/sorafs_node/tests/cli.rs:122 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_node/tests/gateway.rs:14 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_node/tests/gateway.rs:30 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/sorafs_orchestrator/src/lib.rs:6316 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/sorafs_orchestrator/src/lib.rs:6449 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/sorafs_orchestrator/src/lib.rs:8100 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: crates/sorafs_orchestrator/tests/orchestrator_parity.rs:180 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: crates/soranet_pq/tests/kat_vectors.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
- құрастыру: integration_tests/build.rs:17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/src/sorafs_gateway_capability_refusal.rs:157 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- сынақ: integration_tests/src/sorafs_gateway_conformance.rs:1032 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/address_canonicalisation.rs:126 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: integration_tests/tests/asset.rs:49 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/fast_dsl_build.rs:7 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/genesis_json.rs:16 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/genesis_json.rs:22 — `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
- тест: integration_tests/tests/iroha_cli.rs:24 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/ivm_header_decode.rs:49 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/ivm_header_smoke.rs:24 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: integration_tests/tests/kotodama_examples.rs:70 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: integration_tests/tests/kotodama_examples.rs:123 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: integration_tests/tests/kotodama_examples.rs:173 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/nexus/cbdc_rollout_bundle.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/nexus/cbdc_whitelist.rs:26 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: integration_tests/tests/nexus/global_commit.rs:17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- сынақ: integration_tests/tests/nexus/lane_registry.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: integration_tests/tests/norito_burn_fixture.rs:19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: integration_tests/tests/repo.rs:31 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: integration_tests/tests/streaming/mod.rs:339 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- өнім: mochi/mochi-core/src/supervisor.rs:220 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- өнім: mochi/mochi-core/src/supervisor.rs:538 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- сынақ: mochi/mochi-core/src/torii.rs:4740 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: mochi/mochi-core/src/torii.rs:4755 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: mochi/mochi-core/src/torii.rs:4770 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: mochi/mochi-core/src/torii.rs:4785 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: mochi/mochi-integration/tests/supervisor.rs:168 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- сынақ: tools/soranet-handshake-harness/tests/fixtures_verify.rs:6 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: tools/soranet-handshake-harness/tests/interop_parity.rs:77 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- сынақ: tools/soranet-handshake-harness/tests/perf_gate.rs:173 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- құрал: xtask/src/bin/control_plane_mock.rs:362 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- құрал: xtask/src/main.rs:11383 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- құрал: xtask/src/sorafs/gateway_fixture.rs:28 — `env!("CARGO_MANIFEST_DIR"),`
- құрал: xtask/src/sorafs/gateway_fixture.rs:32 — `env!("CARGO_MANIFEST_DIR"),`
- тест: xtask/tests/address_vectors.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/android_dashboard_parity_cli.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/codec_rans_tables.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/da_proof_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/iso_bridge_lint.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/ministry_agenda.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/sns_catalog_verify.rs:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soradns_cli.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/sorafs_fetch_fixture.rs:9 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_bug_bounty.rs:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_gateway_billing.rs:12 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_gateway_billing_m0.rs:30 — `let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_gateway_m1.rs:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_gateway_m2.rs:25 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_pop_template.rs:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_pop_template.rs:77 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_pop_template.rs:131 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_pop_template.rs:202 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_pop_template.rs:306 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_pop_template.rs:356 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/soranet_pop_template.rs:489 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- сынақ: xtask/tests/streaming_bundle_check.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/tests/streaming_entropy_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_VERSION (өнім: 11, сынақ: 1, құрал: 1)- өнім: crates/iroha/src/client.rs:478 — `map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));`
- өнім: crates/iroha_cli/src/commands/sorafs.rs:1767 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- өнім: crates/iroha_cli/src/main_shared.rs:72 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- өнім: crates/iroha_cli/src/main_shared.rs:530 — `let client_version = env!("CARGO_PKG_VERSION");`
- сынақ: crates/iroha_cli/tests/cli_smoke.rs:359 — `let expected_version = env!("CARGO_PKG_VERSION");`
- өнім: crates/iroha_core/src/sumeragi/rbc_store.rs:40 — `version: env!("CARGO_PKG_VERSION").to_owned(),`
- өнім: crates/iroha_js_host/src/lib.rs:2998 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- өнім: crates/iroha_telemetry/src/ws.rs:243 — `env!("CARGO_PKG_VERSION")`
- өнім: crates/irohad/src/main.rs:502 — `version = env!("CARGO_PKG_VERSION"),`
- өнім: crates/irohad/src/main.rs:3986 — `version = env!("CARGO_PKG_VERSION"),`
- өнім: crates/sorafs_car/src/bin/sorafs_fetch.rs:1111 — `Value::from(env!("CARGO_PKG_VERSION")),`
- өнім: crates/sorafs_orchestrator/src/bin/sorafs_cli.rs:93 — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
- құрал: tools/telemetry-schema-diff/src/main.rs:248 — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## ЖҮК_БАСТАУЫ_ПАКЕТІ (құрылымы: 1)

- құрастыру: жәшіктер/soranet_pq/build.rs:6 — `if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {`

## CARGO_TARGET_DIR (өнім: 3, сынақ: 2, құрал: 1)

- сынақ: crates/iroha_test_network/src/lib.rs:524 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- сынақ: crates/iroha_test_network/src/lib.rs:759 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- өнім: mochi/mochi-core/src/supervisor.rs:575 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- өнім: mochi/mochi-core/src/supervisor.rs:629 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- өнім: mochi/mochi-core/src/supervisor.rs:683 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- құрал: xtask/src/mochi.rs:383 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## ЖҮК_ЖҰМЫС_КЕҢІСІ_DIR (сынақ: 1)

- сынақ: crates/iroha_core/src/state.rs:12015 — `if let Some(workspace_dir) = option_env!("CARGO_WORKSPACE_DIR") {`

## CRYPTO_SM_INTRINSICS (орындық: 1)

- орындық: crates/iroha_crypto/benches/sm_perf.rs:183 — `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`

## CUDA_HOME (құрылым: 2)

- құрастыру: жәшіктер/fastpq_prover/build.rs:198 — `env::var_os("CUDA_HOME")`
- құрастыру: crates/norito/accelerators/jsonstage1_cuda/build.rs:63 — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (құрылым: 2)

- құрастыру: жәшіктер/fastpq_prover/build.rs:199 — `.or_else(|| env::var_os("CUDA_PATH"))`
- құрастыру: crates/norito/accelerators/jsonstage1_cuda/build.rs:64 — `.or_else(|| env::var_os("CUDA_PATH"))`

## DATASPACE_ADVERSARIAL_ARTIFACT_DIR (сынақ: 1)

- сынақ: integration_tests/tests/nexus/cross_lane.rs:686 — `if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {`

## DOCS_RS (құрылым: 1)

- құрастыру: жәшіктер/norito/build.rs:8 — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (орындық: 1)

- орындық: crates/norito/benches/enum_packed_bench.rs:75 — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_DEBUG_FUSED (өнім: 1)

- өнім: crates/fastpq_prover/src/trace.rs:98 — `Some(*DEBUG_FUSED_ENV.get_or_init(|| env::var_os("FASTPQ_DEBUG_FUSED").is_some()))`

## FASTPQ_EXPECTED_KIB (сынақ: 1)

- сынақ: crates/fastpq_prover/tests/perf_production.rs:88 — `env::var("FASTPQ_EXPECTED_KIB")`

## FASTPQ_EXPECTED_MS (сынақ: 1)

- сынақ: crates/fastpq_prover/tests/perf_production.rs:80 — `env::var("FASTPQ_EXPECTED_MS")`

## FASTPQ_PROOF_ROWS (сынақ: 1)

- сынақ: crates/fastpq_prover/tests/perf_production.rs:72 — `env::var("FASTPQ_PROOF_ROWS")`

## FASTPQ_SKIP_GPU_BUILD (құрылым: 1)

- құрастыру: жәшіктер/fastpq_prover/build.rs:45 — `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## FASTPQ_UPDATE_FIXTURES (сынақ: 6)- сынақ: crates/fastpq_prover/tests/backend_regression.rs:47 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- сынақ: crates/fastpq_prover/tests/backend_regression.rs:69 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- сынақ: crates/fastpq_prover/tests/proof_fixture.rs:43 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- сынақ: crates/fastpq_prover/tests/trace_commitment.rs:23 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- сынақ: crates/fastpq_prover/tests/trace_commitment.rs:111 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- сынақ: crates/fastpq_prover/tests/transcript_replay.rs:67 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`

## GENESIS_DEBUG_MODE (сынақ: 1)

- сынақ: crates/iroha_test_network/examples/genesis_debug.rs:15 — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## GENESIS_DEBUG_PAY ЖҮКТЕМЕСІ (сынақ: 1)

- сынақ: crates/iroha_test_network/examples/genesis_debug.rs:122 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## GITHUB_STEP_SUMMARY (өнім: 2)

- өнім: crates/iroha_crypto/src/bin/gost_perf_check.rs:22 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- өнім: crates/iroha_crypto/src/bin/sm_perf_check.rs:205 — `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## GIT_COMMIT_HASH (өнім: 1)

- өнім: crates/iroha_core/src/sumeragi/rbc_store.rs:42 — `git_commit: option_env!("GIT_COMMIT_HASH").map(str::to_owned),`

## ҮЙ (өнім: 1)

- өнім: crates/iroha/src/config.rs:57 — `env::var_os("HOME").map(PathBuf::from)`

## IROHA_ALLOW_NET (сынақ: 1)

- сынақ: crates/izanami/src/chaos.rs:374 — `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## IROHA_CONF_GAS_SEED (сынақ: 1)

- сынақ: crates/iroha_test_samples/src/lib.rs:57 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_DA_SPOOL_DIR (сынақ: 1)

- сынақ: crates/iroha_core/src/state.rs:9096 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_METRICS_PANIC_ON_DUPLICATE (сынақ: 2)

- сынақ: crates/iroha_telemetry/src/metrics.rs:11397 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- сынақ: crates/iroha_torii/tests/metrics_registry.rs:33 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## IROHA_RUN_NORED (сынақ: 71)- сынақ: crates/iroha_core/tests/gov_auto_close_approve.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_finalize_real_vk.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_min_duration.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_mode_mismatch.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_mode_mismatch_zk.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_plain_ballot.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_plain_conviction.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_plain_disabled.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_plain_missing_ref.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_plain_revote_monotonic.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_protected_gate.rs:64 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_referendum_open_close.rs:24 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_thresholds.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_thresholds.rs:82 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_thresholds_positive.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_unlock_sweep.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_zk_ballot_lock_verified.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_zk_ballot_real_vk.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/gov_zk_referendum_window_guard.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/zk_roots_get_cap.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_core/tests/zk_vote_get_tally.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_crypto/src/merkle.rs:1317 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_crypto/src/merkle.rs:1332 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_crypto/tests/merkle_norito_roundtrip.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_crypto/tests/merkle_norito_roundtrip.rs:42 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_data_model/src/block/header.rs:624 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_data_model/src/isi/mod.rs:1888 — `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- сынақ: crates/iroha_data_model/src/isi/register.rs:324 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_data_model/src/proof.rs:798 — `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- сынақ: crates/iroha_data_model/src/transaction/signed.rs:1048 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_data_model/tests/instruction_registry_lazy_init.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_data_model/tests/instruction_registry_reset.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: жәшіктер/iroha_data_model/tests/model_derive_repro.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: жәшіктер/iroha_data_model/tests/model_derive_repro.rs:37 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: жәшіктер/iroha_data_model/tests/model_derive_repro.rs:58 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: жәшіктер/iroha_data_model/tests/model_derive_repro.rs:84 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_data_model/tests/registry_decode_roundtrip.rs:9 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_data_model/tests/trait_objects.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_data_model/tests/trait_objects.rs:27 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`- сынақ: crates/iroha_data_model/tests/zk_envelope_roundtrip.rs:6 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/contracts_activate_integration.rs:23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/contracts_activate_integration.rs:174 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/contracts_call_integration.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/conttracts_deploy_integration.rs:23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/contracts_instance_activate_integration.rs:19 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/contracts_instances_list_router.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/gov_council_persist_integration.rs:22 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/gov_council_vrf.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/gov_enact_handler.rs:12 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/gov_instances_list.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/gov_mode_mismatch_and_autoclose.rs:42 — `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- сынақ: crates/iroha_torii/tests/gov_protected_endpoints.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/gov_protected_endpoints_router.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/iroha_torii/tests/gov_read_endpoints_router.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/ivm/tests/beep_test.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/ivm/tests/kotodama_struct_fields.rs:11 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: crates/ivm/tests/zk_roots_and_vote_syscalls.rs:50 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: интеграциялық_тесттер/тесттер/оқиғалар/notification.rs:81 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: integration_tests/tests/extra_functional/unstable_network.rs:753 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: integration_tests/tests/extra_functional/unstable_network.rs:769 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: integration_tests/tests/extra_functional/unstable_network.rs:785 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: integration_tests/tests/extra_functional/unstable_network.rs:801 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: integration_tests/tests/extra_functional/unstable_network.rs:817 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: integration_tests/tests/permissions.rs:202 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: integration_tests/tests/permissions.rs:265 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: integration_tests/tests/permissions.rs:328 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: integration_tests/tests/pipeline_block_rejected.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: integration_tests/tests/sorting.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: integration_tests/tests/triggers/by_call_trigger.rs:139 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- сынақ: integration_tests/tests/triggers/time_trigger.rs:149 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_RUN_ZK_WRAPPERS (сынақ: 1)

- сынақ: crates/ivm/tests/kotodama_wrappers.rs:2 — `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`

## IROHA_SKIP_BIND_CHECKS (сынақ: 1)

- сынақ: crates/iroha_test_network/src/lib.rs:3098 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (сынақ: 1)

- сынақ: crates/iroha_crypto/tests/sm_cli_matrix.rs:47 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_TEST_DUMP_GENESIS (сынақ: 1)

- сынақ: crates/iroha_test_network/src/lib.rs:6661 — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (құрастыру: 1, сынақ: 1)

- сынақ: crates/iroha_test_network/src/config.rs:155 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- құрастыру: integration_tests/build.rs:205 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_SKIP_BUILD (сынақ: 1)

- сынақ: crates/iroha_test_network/src/lib.rs:1244 — `std::env::var("IROHA_TEST_SKIP_BUILD")`

## IROHA_TEST_TARGET_DIR (сынақ: 2)

- сынақ: crates/iroha_test_network/src/lib.rs:521 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`
- сынақ: crates/iroha_test_network/src/lib.rs:765 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`

## IROHA_TEST_USE_DEFAULT_EXECUTOR (сынақ: 3)

- сынақ: crates/iroha_core/src/executor.rs:2383 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- сынақ: crates/iroha_core/src/executor.rs:2520 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- сынақ: crates/iroha_core/src/state.rs:12008 — `if std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR").is_some() {`

## IROHA_TORII_OPENAPI_ACTUAL (сынақ: 1)

- сынақ: crates/iroha_torii/tests/router_feature_matrix.rs:94 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_КҮТІЛГЕН (сынақ: 2)

- сынақ: crates/iroha_torii/tests/router_feature_matrix.rs:88 — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- сынақ: crates/iroha_torii/tests/router_feature_matrix.rs:104 — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_TORII_OPENAPI_TOKENS (құрал: 2)

- құрал: xtask/src/main.rs:11025 — `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`
- құрал: xtask/src/main.rs:11077 — `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IVM_BIN (сынақ: 2)

- сынақ: integration_tests/tests/kotodama_examples.rs:60 — `let ivm_bin = env::var("IVM_BIN")`
- тест: integration_tests/tests/kotodama_examples.rs:163 — `let ivm_bin = env::var("IVM_BIN")`

## IVM_COMPILER_DEBUG (өнім: 1)

- өнім: crates/kotodama_lang/src/compiler.rs:4094 — `let compiler_debug = if std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_GENCODE (құрылым: 1)

- құрастыру: crates/ivm/build.rs:35 — `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| "arch=compute_61,code=sm_61".to_string());`

## IVM_CUDA_NVCC (құрылым: 1)

- құрастыру: crates/ivm/build.rs:30 — `let nvcc = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVCC_EXTRA (құрылым: 1)

- құрастыру: crates/ivm/build.rs:36 — `let extra_flags: Vec<String> = env::var("IVM_CUDA_NVCC_EXTRA")`

## IVM_DEBUG_IR (сынақ: 1)

- сынақ: crates/ivm/tests/debug_contains.rs:15 — `if std::env::var_os("IVM_DEBUG_IR").is_some() {`

## IVM_DEBUG_METAL_ENNUM (отлад: 1)

- жөндеу: crates/ivm/src/vector.rs:474 — `std::env::var("IVM_DEBUG_METAL_ENUM")`

## IVM_DEBUG_METAL_SELFTEST (отлад: 1)

- жөндеу: crates/ivm/src/vector.rs:1205 — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DISABLE_CUDA (отлад: 1)

- жөндеу: crates/ivm/src/cuda.rs:315 — `&& std::env::var("IVM_DISABLE_CUDA")`

## IVM_DISABLE_METAL (отлад: 1)

- жөндеу: crates/ivm/src/vector.rs:299 — `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELFTEST_FAIL (отлад: 1)

- жөндеу: crates/ivm/src/cuda.rs:326 — `&& std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL")`

## IVM_FORCE_METAL_ENNUM (отлад: 1)

- жөндеу: crates/ivm/src/vector.rs:444 — `std::env::var("IVM_FORCE_METAL_ENUM")`

## IVM_FORCE_METAL_SELFTEST_FAIL (отлад: 1)

- жөндеу: crates/ivm/src/vector.rs:1192 — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IVM_TOOL_BIN (сынақ: 1)

- тест: integration_tests/tests/kotodama_examples.rs:113 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## IZANAMI_ALLOW_NET (сынақ: 1)

- сынақ: crates/izanami/src/chaos.rs:373 — `std::env::var("IZANAMI_ALLOW_NET")`

## IZANAMI_TUI_НӨЛ_ТҰҚЫМҒА_РҰҚСАТ БЕРУ (өнім: 1)

- өнім: crates/izanami/src/tui.rs:134 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (құрылым: 1)

- құрастыру: crates/norito/accelerators/jsonstage1_cuda/build.rs:40 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_SKIP_BUILD (құрылым: 1)

- құрастыру: crates/norito/accelerators/jsonstage1_cuda/build.rs:18 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KOTO_BIN (сынақ: 2)- тест: integration_tests/tests/kotodama_examples.rs:50 — `let koto_bin = env::var("KOTO_BIN")`
- тест: integration_tests/tests/kotodama_examples.rs:154 — `let koto_bin = env::var("KOTO_BIN")`

## LANG (сынақ: 3)

- сынақ: crates/ivm/src/bin/koto_lint.rs:702 — `let previous = env::var("LANG").ok();`
- сынақ: жәшіктер/ivm/tests/i18n.rs:11 — `let old_lang = env::var("LANG").ok();`
- сынақ: жәшіктер/ivm/tests/i18n.rs:69 — `let old_lang = env::var("LANG").ok();`

## LC_ALL (сынақ: 2)

- сынақ: жәшіктер/ivm/tests/i18n.rs:12 — `let old_lc_all = env::var("LC_ALL").ok();`
- сынақ: жәшіктер/ivm/tests/i18n.rs:70 — `let old_lc_all = env::var("LC_ALL").ok();`

## LC_MESSAGES (сынақ: 2)

- сынақ: жәшіктер/ivm/tests/i18n.rs:13 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`
- сынақ: жәшіктер/ivm/tests/i18n.rs:71 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## MAX_DEGREE (өнім: 1)

- өнім: crates/iroha_core/src/zk.rs:106 — `let current = std::env::var("MAX_DEGREE")`

## MOCHI_CONFIG (өнім: 1)

- өнім: mochi/mochi-ui-egui/src/config.rs:325 — `if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## MOCHI_DATA_ROOT (өнім: 1)

- өнім: mochi/mochi-core/src/supervisor.rs:2068 — `std::env::var_os("MOCHI_DATA_ROOT")`

## MOCHI_TEST_ПАЙДАЛАНУ_ІШКІ_ГЕНЕСІС (өнім: 1)

- өнім: mochi/mochi-core/src/supervisor.rs:1994 — `if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {`

## NORITO_BENCH_ҚЫСҚАУ (орындық: 1)

- орындық: жәшіктер/norito/benches/parity_compare.rs:70 — `if std::env::var("NORITO_BENCH_SUMMARY").ok().as_deref() == Some("1") {`

## NORITO_CPU_INFO (құрал: 1)

- құрал: xtask/src/stage1_bench.rs:69 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_GPU_LIB (өнім: 1)

- өнім: crates/norito/src/core/simd_crc64.rs:215 — `std::env::var("NORITO_CRC64_GPU_LIB").ok(),`

## NORITO_DISABLE_PACKED_STRUCT (өнім: 1, сынақ: 1)

- өнім: crates/iroha_js_host/src/lib.rs:183 — `let env_set = std::env::var_os("NORITO_DISABLE_PACKED_STRUCT").is_some();`
- сынақ: crates/norito/src/lib.rs:322 — `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## NORITO_GPU_CRC64_MIN_BYTES (өнім: 1)

- өнім: crates/norito/src/core/simd_crc64.rs:83 — `std::env::var("NORITO_GPU_CRC64_MIN_BYTES").ok(),`

## NORITO_PAR_STAGE1_MIN (сынақ: 1)

- сынақ: crates/norito/src/lib.rs:4847 — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## NORITO_SKIP_BINDINGS_SYNC (құрылым: 1)

- құрастыру: жәшіктер/norito/build.rs:12 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_GPU_MIN_BYTES (сынақ: 1)

- сынақ: crates/norito/src/lib.rs:4881 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## NORITO_TRACE (сынақ: 2)

- сынақ: crates/norito/src/lib.rs:123 — `std::env::var_os("NORITO_TRACE").is_some()`
- сынақ: crates/norito/src/lib.rs:138 — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## NO_PROXY (өнім: 1)

- өнім: crates/iroha_p2p/src/transport.rs:469 — `let no_proxy = env::var("NO_PROXY")`

## NVCC (құрылым: 1)

- құрастыру: crates/ivm/build.rs:31 — `.or_else(|_| env::var("NVCC"))`

## OUT_DIR (құрылым: 4, өнім: 12, сынақ: 2)- құрастыру: жәшіктер/fastpq_prover/build.rs:99 — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
- құрастыру: жәшіктер/iroha_data_model/build.rs:12 — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));`
- өнім: crates/iroha_data_model/src/lib.rs:179 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
- құрастыру: crates/ivm/build.rs:27 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- құрастыру: crates/ivm/build.rs:120 — `if let Some(out_dir) = env::var_os("OUT_DIR") {`
- өнім: crates/ivm/src/cuda.rs:12 — `static PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- өнім: crates/ivm/src/cuda.rs:13 — `static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));`
- өнім: crates/ivm/src/cuda.rs:14 — `static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));`
- өнім: crates/ivm/src/cuda.rs:15 — `static SHA_LEAVES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx"));`
- өнім: crates/ivm/src/cuda.rs:16 — `static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));`
- өнім: crates/ivm/src/cuda.rs:17 — `static SHA3_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx"));`
- өнім: crates/ivm/src/cuda.rs:18 — `static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));`
- өнім: crates/ivm/src/cuda.rs:19 — `static BN254_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx"));`
- өнім: crates/ivm/src/cuda.rs:20 — `static SIG_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/signature.ptx"));`
- өнім: crates/ivm/src/cuda.rs:21 — `static SHA_PAIRS_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx"));`
- өнім: crates/ivm/src/cuda.rs:22 — `static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));`
- сынақ: crates/ivm/src/ptx_tests.rs:7 — `let out_dir = match std::env::var("OUT_DIR") {`
- сынақ: crates/ivm/tests/ptx_kernels.rs:5 — `let out_dir = env!("OUT_DIR");`

## P2P_TURN (өнім: 1)

- өнім: crates/iroha_p2p/src/transport.rs:296 — `let endpoint = std::env::var("P2P_TURN")`

## PATH (өнім: 2, сынақ: 1)

- өнім: crates/iroha_cli/src/commands/sorafs.rs:7276 — `if let Some(path_var) = env::var_os("PATH") {`
- тест: integration_tests/tests/kotodama_examples.rs:17 — `let path = env::var_os("PATH")?;`
- өнім: mochi/mochi-core/src/supervisor.rs:511 — `let path_var = env::var_os("PATH")?;`

## PRINT_SORACLES_FIXTURES (сынақ: 1)

- сынақ: crates/iroha_data_model/src/oracle/mod.rs:3249 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORII_SPEC (сынақ: 1)

- сынақ: crates/iroha_torii/src/openapi.rs:4380 — `if std::env::var("PRINT_TORII_SPEC").is_ok() {`

## PROFILE (құрылым: 1, өнім: 1, сынақ: 1)

- өнім: crates/iroha_core/src/sumeragi/rbc_store.rs:41 — `profile: option_env!("PROFILE").unwrap_or("unknown").to_owned(),`
- сынақ: crates/iroha_test_network/src/lib.rs:995 — `let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());`
- құрастыру: integration_tests/build.rs:180 — `let profile = match env::var("PROFILE").unwrap_or_default().as_str() {`

## PYTHON3 (сынақ: 2)

- сынақ: crates/sorafs_car/tests/taikai_car_cli.rs:235 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`
- сынақ: crates/sorafs_car/tests/taikai_viewer_cli.rs:30 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONPATH (сынақ: 1)

- сынақ: crates/iroha_cli/tests/cli_smoke.rs:5245 — `match env::var("PYTHONPATH") {`

## RBC_SESSION_PATH (сынақ: 1)

- сынақ: crates/iroha_core/src/sumeragi/rbc_store.rs:823 — `let path = std::env::var("RBC_SESSION_PATH").expect("set RBC_SESSION_PATH");`

## REPO_PROOF_DIGEST_OUT (сынақ: 1)

- сынақ: crates/iroha_core/src/smartcontracts/isi/repo.rs:2017 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (сынақ: 1)

- сынақ: crates/iroha_core/src/smartcontracts/isi/repo.rs:2005 — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUST_LOG (өнім: 1, сынақ: 2)

- сынақ: crates/iroha_test_network/src/lib.rs:5414 — `let original = env::var("RUST_LOG").ok();`
- сынақ: crates/iroha_test_network/src/lib.rs:5431 — `let original = env::var("RUST_LOG").ok();`
- өнім: crates/izanami/src/config.rs:269 — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`

## SM_PERF_CPU_LABEL (өнім: 2)

- өнім: crates/iroha_crypto/src/bin/sm_perf_check.rs:637 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- өнім: crates/iroha_crypto/src/bin/sm_perf_check.rs:679 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## SORAFS_NODE_SKIP_INGEST_TESTS (сынақ: 1)

- сынақ: жәшіктер/sorafs_node/tests/cli.rs:17 — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORAFS_TORII_SKIP_INGEST_TESTS (сынақ: 1)

- сынақ: crates/iroha_torii/tests/sorafs_discovery.rs:95 — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`## SUMERAGI_ADVERSARIAL_ARTIFACT_DIR (сынақ: 1)

- тест: integration_tests/tests/sumeragi_adversarial.rs:1222 — `let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {`

## SUMERAGI_BASELINE_ARTIFACT_DIR (өнім: 1, сынақ: 1)

- өнім: crates/build-support/src/bin/sumeragi_baseline_report.rs:40 — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`
- сынақ: integration_tests/tests/sumeragi_npos_performance.rs:1396 — `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`

## SUMERAGI_DA_ARTIFACT_DIR (өнім: 1, сынақ: 1)

- өнім: crates/build-support/src/bin/sumeragi_da_report.rs:41 — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`
- тест: интеграциялық_тесттер/тесттер/sumeragi_da.rs:1599 — `let Ok(dir) = std::env::var("SUMERAGI_DA_ARTIFACT_DIR") else {`

## SystemRoot (өнім: 1)

- өнім: crates/fastpq_prover/src/backend.rs:535 — `env::var_os("SystemRoot").map(PathBuf::from)`

## TARGET (құрал: 2)

- құрал: xtask/src/poseidon_bench.rs:87 — `target: std::env::var("TARGET")`
- құрал: xtask/src/stage1_bench.rs:63 — `target: std::env::var("TARGET")`

## ТЕСТ_ЖУРНАЛЫ_СҮЗГІ (өнім: 1)

- өнім: crates/iroha_logger/src/lib.rs:89 — `filter: std::env::var("TEST_LOG_FILTER")`

## TEST_LOG_LEVEL (өнім: 1)

- өнім: crates/iroha_logger/src/lib.rs:85 — `level: std::env::var("TEST_LOG_LEVEL")`

## ТЕСТ_ЖЕЛІЛІК_ЖҮК (тест: 1)

- сынақ: crates/iroha_test_network/src/lib.rs:865 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TORII_DEBUG_SORT (өнім: 1)

- өнім: crates/iroha_torii/src/routing.rs:12530 — `std::env::var("TORII_DEBUG_SORT").ok().is_some(),`

## TORII_MOCK_HARNESS_METRICS_PATH (құрал: 1)

- құрал: xtask/src/bin/torii_mock_harness.rs:106 — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (құрал: 1)

- құрал: xtask/src/bin/torii_mock_harness.rs:109 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_RETRY_TOTAL (құрал: 1)

- құрал: xtask/src/bin/torii_mock_harness.rs:297 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (құрал: 1)

- құрал: xtask/src/bin/torii_mock_harness.rs:112 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (құрал: 1)

- құрал: xtask/src/bin/torii_mock_harness.rs:104 — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## TORII_OPENAPI_TOKEN (құрал: 2)

- құрал: xtask/src/main.rs:11020 — `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`
- құрал: xtask/src/main.rs:11073 — `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## ЖАҢАРТУ_ЖАҢАЛЫҚТАР (сынақ: 1)

- сынақ: crates/iroha_core/tests/snapshots.rs:42 — `let update = env::var("UPDATE_FIXTURES")`

## ПАЙДАЛАНУШЫ ПРОФИЛЬІ (өнім: 1)

- өнім: crates/iroha/src/config.rs:55 — `env::var_os("USERPROFILE").map(PathBuf::from)`

## VERGEN_CARGO_FETURES (өнім: 1)

- өнім: crates/irohad/src/main.rs:4850 — `const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {`

## VERGEN_CARGO_TARGET_TRIPLE (өнім: 1)

- өнім: crates/iroha_telemetry/src/ws.rs:238 — `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");`

## VERGEN_GIT_SHA (өнім: 4)

- өнім: crates/iroha_cli/src/main_shared.rs:57 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`
- өнім: crates/iroha_telemetry/src/ws.rs:237 — `let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");`
- өнім: crates/iroha_torii/src/routing.rs:32515 — `git_sha: option_env!("VERGEN_GIT_SHA")`
- өнім: crates/irohad/src/main.rs:4845 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`

## ТЕКСЕРУ_ПАРАТЫ (орындық: 1)

- орындық: жәшіктер/ivm/benches/bench_voting.rs:225 — `let verify_batch = std::env::var("VERIFY_BATCH")`

## ӘРБІРІН ТЕКСЕРІҢІЗ (орындық: 1)

- орындық: жәшіктер/ivm/benches/bench_voting.rs:213 — `let verify_every: u64 = std::env::var("VERIFY_EVERY")`

## САЙЛАУШЫЛАР (орын: 1)

- орындық: жәшіктер/ivm/орықтар/bench_voting.rs:207 — `let voters: u64 = std::env::var("VOTERS")`

## прокси жоқ (өнім: 1)

- өнім: crates/iroha_p2p/src/transport.rs:471 — `.or_else(|| env::var("no_proxy").ok());`