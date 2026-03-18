---
lang: am
direction: ltr
source: docs/source/agents/env_var_inventory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0a23896374a10f0862aeec69c607ba3022d3b337ae7f6bb54a61b8f7424410
source_last_modified: "2026-01-21T19:17:13.235848+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የአካባቢ ንብረት ቆጠራ

ለመጨረሻ ጊዜ የታደሰው በ`python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`_

ጠቅላላ ዋቢዎች፡ **505** · ልዩ ተለዋዋጮች፡ **137**

## የድርጊት_መታወቂያ_TOKEN_ጥያቄ_TOKEN (ምርት፡ 1)

- ፕሮድ፡ ሳጥኖች/ሶራፍስ_ኦርኬስትራ/src/bin/sorafs_cli.rs:313 — `let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {`

## ACTIONS_ID_TOKEN_REQUEST_URL (ምርት፡ 1)

- ፕሮድ፡ ሳጥኖች/ሶራፍስ_ኦርኬስትራ/src/bin/sorafs_cli.rs:310 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`

## BLOCK_DUMP_HEIGHTS (ምሳሌ፡ 1)

- ምሳሌ፡ crates/iroha_core/emples/block_dump.rs:62 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`

## BLOCK_DUMP_SUM_ASSET (ምሳሌ፡ 1)

- ምሳሌ፡ crates/iroha_core/emples/block_dump.rs:51 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`

## BLOCK_DUMP_VERBOSE (ምሳሌ፡ 1)

- ምሳሌ፡ crates/iroha_core/emples/block_dump.rs:50 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`

## CARGO (ምርት፡ 3፣ ሙከራ፡ 2)

ሙከራ፡ crates/iroha_test_network/src/lib.rs:1026 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- ሙከራ፡ ሳጥኖች/sorafs_manifest/tests/አቅራቢ_admission_fixtures.rs:11 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:547 — `let cargo = env::var_os("CARGO")`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:602 — `let cargo = env::var_os("CARGO")`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:656 — `let cargo = env::var_os("CARGO")`

## CARGO_BIN_EXE_iroha (ሙከራ፡ 2)

- ሙከራ፡ crates/iroha_cli/tests/cli_smoke.rs:40 — `env!("CARGO_BIN_EXE_iroha")`
- ሙከራ፡ crates/iroha_cli/tests/taikai_policy.rs:20 — `env!("CARGO_BIN_EXE_iroha")`

## CARGO_BIN_EXE_iroha_ሞኒተር (ሙከራ፡ 4)

- ሙከራ፡ ሳጥኖች/iroha_monitor/tests/attach_render.rs:11 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- ሙከራ፡ crates/iroha_monitor/tests/http_limits.rs:10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- ሙከራ፡ ሳጥኖች/iroha_monitor/tests/invalid_credentials.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- ሙከራ፡ ሳጥኖች/iroha_monitor/tests/smoke.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## CARGO_BIN_EXE_kagami (ሙከራ፡ 2)

- ሙከራ፡ ሳጥኖች/iroha_kagami/tests/common/mod.rs:22 — `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- ሙከራ፡ ሳጥኖች/iroha_kagami/tests/pop_embed.rs:34 — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`

## CARGO_BIN_EXE_ካጋሚ_ማሾፍ (ሙከራ፡ 1)

- ሙከራ: mochi/mochi-integration/tests/supervisor.rs:33 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## CARGO_BIN_EXE_ኮቶ_ማጠናቀር (ሙከራ፡ 3)

- ሙከራ፡ ሳጥኖች/ivm/tests/cli_smoke.rs:8 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- ሙከራ፡ crates/ivm/tests/cli_smoke.rs:56 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- ሙከራ፡ crates/ivm/tests/cli_smoke.rs:88 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`

## CARGO_BIN_EXE_sorafs_chunk_mump (ሙከራ፡ 1)

- ሙከራ፡ ሳጥኖች/sorafs_chunker/tests/one_gib.rs:103 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## CARGO_BIN_EXE_sorafs_cli (ሙከራ፡ 1)

- ሙከራ፡ ሳጥኖች/ሶራፍስ_car/tests/sorafs_cli.rs:42 — `let path = env::var("CARGO_BIN_EXE_sorafs_cli")`

## CARGO_BIN_EXE_sorafs_fetch (ሙከራ፡ 1)

- ሙከራ: ሳጥኖች/sorafs_car/src/bin/sorafs_fetch.rs:2831 — `if let Ok(path) = env::var("CARGO_BIN_EXE_sorafs_fetch") {`

## CARGO_BIN_EXE_ታይካይ_መኪና (ሙከራ፡ 1)

- ሙከራ፡ ሳጥኖች/ሶራፍስ_car/tests/sorafs_cli.rs:48 — `let path = env::var("CARGO_BIN_EXE_taikai_car")`

## CARGO_BIN_NAME (ምርት፡ 3)

- ፕሮድ፡ crates/iroha_cli/src/main_shared.rs:63 — `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`
- ፕሮድ፡ crates/iroha_cli/src/main_shared.rs:72 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- ፕሮድ፡ crates/irohad/src/main.rs:3176 — `let build_line = BuildLine::from_bin_name(env!("CARGO_BIN_NAME"));`

## CARGO_BUILD_TARGET (መሳሪያ፡ 2)

መሣሪያ፡ xtask/src/poseidon_bench.rs:88 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
መሣሪያ፡ xtask/src/stage1_bench.rs:64 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_TARGET_ARCH (ምርት፡ 2፣ መሳሪያ፡ 2)- ፕሮድ፡ crates/iroha_crypto/src/bin/sm_perf_check.rs:632 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- ፕሮድ፡ crates/iroha_crypto/src/bin/sm_perf_check.rs:668 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
መሣሪያ፡ xtask/src/poseidon_bench.rs:89 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
መሣሪያ፡ xtask/src/stage1_bench.rs:65 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## CARGO_CFG_TARGET_OS (ግንባታ፡ 1፣ ፕሮድ፡ 2፣ መሳሪያ፡ 2)

ግንባታ፡ crates/ fastpq_prover/build.rs:27 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- ፕሮድ፡ crates/iroha_crypto/src/bin/sm_perf_check.rs:633 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- ፕሮድ፡ crates/iroha_crypto/src/bin/sm_perf_check.rs:669 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
መሣሪያ፡ xtask/src/poseidon_bench.rs:91 — `os: std::env::var("CARGO_CFG_TARGET_OS")`
መሣሪያ፡ xtask/src/stage1_bench.rs:67 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## CARGO_FEATURE_CUDA (ግንባታ፡ 2)

ግንባታ፡ crates/ fastpq_prover/build.rs:25 — `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
ግንባታ፡ crates/ivm/build.rs:12 — `if env::var_os("CARGO_FEATURE_CUDA").is_some()`

## CARGO_FEATURE_CUDA_KERNEL (ግንባታ፡ 1)

ግንባታ፡ crates/norito/accelerators/jsonstage1_cuda/build.rs:12 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (ግንባታ፡ 1)

ግንባታ፡ crates/ fastpq_prover/build.rs:26 — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## CARGO_FEATURE_FFI_EXPORT (ምርት፡ 1)

- ፕሮድ፡ ሳጥኖች/የግንባታ ድጋፍ/src/lib.rs:30 — `let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();`

## የካርጎ_ባህሪ_ፊፊ_አይምፖርት (ምርት፡ 1)

- ፕሮድ፡ ሳጥኖች/ግንባታ-ድጋፍ/src/lib.rs:29 — `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`

## CARGO_MANIFEST_DIR (ቤንች፡ 4፣ ግንባታ፡ 5፣ ምሳሌ፡ 1፣ ፕሮድ፡ 27፣ ሙከራ፡ 164፣ መሳሪያ፡ 4)- ፕሮድ፡ crates/fastpq_prover/src/poseidon_manifest.rs:10 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ crates/ fastpq_prover/tests/packing.rs:17 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/ fastpq_prover/tests/poseidon_manifest_consistency.rs:9 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- ሙከራ፡ crates/ fastpq_prover/tests/poseidon_manifest_consistency.rs:28 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- ሙከራ፡ crates/ fastpq_prover/tests/proof_fixture.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ: crates/fastpq_prover/tests/trace_commitment.rs:14 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- ሙከራ፡ crates/ fastpq_prover/tests/transcript_replay.rs:56 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ ሳጥኖች/iroha/src/client.rs:8409 — `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ: crates/iroha/src/sm.rs:201 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/iroha/tess/sm_signing.rs:35 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ: crates/iroha_cli/src/compute.rs:518 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ፕሮድ፡ crates/iroha_cli/src/main_shared.rs:766 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ፡ crates/iroha_cli/tests/cli_smoke.rs:111 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_cli/tests/cli_smoke.rs:5236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ፕሮድ፡ crates/iroha_config/src/parameters/user.rs:3490 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ: ሳጥኖች/iroha_config/src/parameters/user.rs:11561 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_config/tests/fastpq_queue_overrides.rs:15 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
ሙከራ: ሳጥኖች/iroha_config/tests/fixtures.rs:36 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- አግዳሚ ወንበር፡ crates/iroha_core/benches/blocks/common.rs:261 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- አግዳሚ ወንበር፡ ሳጥኖች/iroha_core/benches/blocks/common/mod.rs:272 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- አግዳሚ ወንበር፡ crates/iroha_core/benches/validation.rs:96 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
ግንባታ፡ crates/iroha_core/build.rs:19 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- ምሳሌ፡ crates/iroha_core/emples/generate_parity_fixtures.rs:17 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
ሙከራ: crates/iroha_core/src/executor.rs:2385 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
ሙከራ: crates/iroha_core/src/executor.rs:2521 — `let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ: crates/iroha_core/src/executor.rs:2661 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- ፕሮድ፡ crates/iroha_core/src/smartcontracts/isi/offline.rs:2152 — `env!("CARGO_MANIFEST_DIR"),`
- ፕሮድ፡ crates/iroha_core/src/smartcontracts/isi/offline.rs:2156 — `env!("CARGO_MANIFEST_DIR"),`
- ፕሮድ፡ crates/iroha_core/src/smartcontracts/isi/offline.rs:2164 — `env!("CARGO_MANIFEST_DIR"),`
- ፕሮድ፡ crates/iroha_core/src/smartcontracts/isi/offline.rs:2171 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ crates/iroha_core/src/smartcontracts/isi/offline.rs:4852 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ: ሳጥኖች/iroha_core/src/smartcontracts/isi/repo.rs:1870 — `env!("CARGO_MANIFEST_DIR"),`
ሙከራ: ሳጥኖች/iroha_core/src/smartcontracts/isi/repo.rs:1874 — `env!("CARGO_MANIFEST_DIR"),`
- ፕሮድ፡ crates/iroha_core/src/state.rs:9990 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
ሙከራ: crates/iroha_core/src/state.rs:12012 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ: crates/iroha_core/src/streaming.rs:2980 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ: crates/iroha_core/src/tx.rs:4424 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_core/tests/executor_migration_introspect.rs:25 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
ሙከራ፡ ሳጥኖች/iroha_core/tests/pin_registry.rs:1158 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ፡ crates/iroha_core/tests/snapshots.rs:29 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ፡ crates/iroha_core/tests/sumeragi_doc_sync.rs:57 — `Path::new(env!("CARGO_MANIFEST_DIR"))`ሙከራ፡ ሳጥኖች/iroha_crypto/tests/confidential_keyset_vectors.rs:57 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ ሳጥኖች/iroha_crypto/tests/sm2_fixture_vectors.rs:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ: ሳጥኖች/iroha_crypto/tests/sm_cli_matrix.rs:19 — `env!("CARGO_MANIFEST_DIR"),`
ግንባታ፡ crates/iroha_data_model/build.rs:11 - `let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");`
- ፕሮድ፡ crates/iroha_data_model/src/lib.rs:186 — `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- ፕሮድ፡ crates/iroha_data_model/src/lib.rs:190 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/src/offline/poseidon.rs:449 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ: ሳጥኖች/iroha_data_model/src/soranet/vpn.rs:1217 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/account_address_vectors.rs:147 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_data_model/tests/address_curve_registry.rs:33 — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ ሳጥኖች/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:48 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_data_model/tests/confidential_wallet_fixtures.rs:15 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/consensus_roundtrip.rs:1308 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_data_model/tests/offline_fixtures.rs:130 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:29 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:33 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:37 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:41 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:46 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:50 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:54 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:58 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/oracle_reference_fixtures.rs:62 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ crates/iroha_data_model/tests/oracle_reference_fixtures.rs:200 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/runtime_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_genesis/src/lib.rs:1173 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- ሙከራ፡ crates/iroha_genesis/src/lib.rs:3847 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- ሙከራ: crates/iroha_genesis/src/lib.rs:4228 - `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- ሙከራ፡ crates/iroha_genesis/src/lib.rs:4243 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- ሙከራ: crates/iroha_i18n/src/lib.rs:495 — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
ሙከራ፡- crates/iroha_js_host/src/lib.rs:6943 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ፕሮድ፡ crates/iroha_kagami/samples/codec/generate.rs:13 — `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- ፕሮድ፡ crates/iroha_kagami/samples/codec/src/main.rs:35 — `let dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ: crates/iroha_kagami/src/codec.rs:393 — `env!("CARGO_MANIFEST_DIR"),`
- ፕሮድ፡ crates/iroha_kagami/src/localnet.rs:604 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/iroha_kagami/tests/codec.rs:11 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- ሙከራ፡ ሳጥኖች/iroha_telemetry/tests/drill_log.rs:10 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ: crates/iroha_test_network/src/config.rs:724 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- ሙከራ፡ crates/iroha_test_network/src/fslock_ports.rs:23 — `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- ሙከራ፡ crates/iroha_test_network/src/fslock_ports.rs:25 — `env!("CARGO_MANIFEST_DIR"),`
ሙከራ፡ crates/iroha_test_network/src/lib.rs:281 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_test_samples/src/lib.rs:204 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ crates/iroha_test_samples/src/lib.rs:241 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/iroha_torii/src/da/tess.rs:3490 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- ሙከራ: crates/iroha_torii/src/sorafs/api.rs:4053 — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/iroha_torii/tests/account_address_vectors.rs:140 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/iroha_torii/tests/accounts_portfolio.rs:91 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ crates/iroha_torii/tests/sorafs_discovery.rs:1056 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ግንባታ፡ crates/ivm/build.rs:21 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- ፕሮድ፡ crates/ivm/src/bin/gen_abi_hash_doc.rs:28 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ፕሮድ፡ crates/ivm/src/bin/gen_header_doc.rs:48 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ምርት፡ crates/ivm/src/bin/gen_pointer_types_doc.rs:23 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ፕሮድ፡ crates/ivm/src/bin/gen_syscalls_doc.rs:24 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ፕሮድ፡ crates/ivm/src/bin/ivm_prebuild.rs:16 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ፕሮድ፡ crates/ivm/src/bin/ivm_predecoder_export.rs:23 — `let _crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ፕሮድ፡ crates/ivm/src/predecoder_fixtures.rs:236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- ሙከራ፡ crates/ivm/tests/cli_smoke.rs:9 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ሙከራ፡ crates/ivm/tests/cli_smoke.rs:57 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ሙከራ፡ crates/ivm/tests/cli_smoke.rs:89 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ሙከራ፡ crates/ivm/tests/docs_consistency.rs:3 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
ሙከራ፡ ሳጥኖች/ivm/tess/ivm_abi_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ ሳጥኖች/ivm/tests/ivm_header_doc_sync.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/ivm/tests/norito_portal_snippets_compile.rs:19 — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ፡ ሳጥኖች/ivm/tests/pointer_types_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- ሙከራ፡ ሳጥኖች/ivm/tests/pointer_types_doc_generated_ivm_md.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/ivm/tests/syscalls_doc_generated.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
ሙከራ፡ ሳጥኖች/ivm/tests/syscalls_doc_sync.rs:8 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- ሙከራ፡ crates/ivm/tests/syscalls_gas_names.rs:11 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- አግዳሚ ወንበር፡ crates/norito/benches/parity_compare.rs:79 — `let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
ግንባታ፡ crates/norito/build.rs:17 — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- ፕሮድ፡ crates/norito/src/bin/norito_regen_goldens.rs:9 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/norito/tests/aos_ncb_more_golden.rs:200 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- ሙከራ፡ crates/norito/tests/json_golden_loader.rs:14 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/norito/tests/ncb_enum_iter_samples.rs:353 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/norito/tests/ncb_enum_iter_samples.rs:387 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/norito/tests/ncb_enum_iter_samples.rs:554 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- ሙከራ፡ crates/norito/tests/ncb_enum_iter_samples.rs:665 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- ሙከራ፡ crates/norito/tests/ncb_enum_large_fixture.rs:37 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- ሙከራ: ሳጥኖች/sorafs_car/src/bin/da_reconstruct.rs:434 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ: ሳጥኖች/sorafs_car/src/bin/da_reconstruct.rs:710 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ፕሮድ፡ ሳጥኖች/ሶራፍስ_car/src/bin/soranet_trustless_verifier.rs:140 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- ሙከራ፡ ሳጥኖች/ሶራፍስ_car/tests/capacity_simulation_toolkit.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/ሶራፍስ_car/tests/fetch_cli.rs:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ ሳጥኖች/ሶራፍስ_car/tests/fetch_cli.rs:1043 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ: ሳጥኖች/ሶራፍስ_car/tests/fetch_cli.rs:1159 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/ሶራፍስ_car/ፈተናዎች/taikai_car_cli.rs:227 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/ሶራፍስ_car/tests/taikai_viewer_cli.rs:22 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/ሶራፍስ_car/tests/trustless_verifier.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ፕሮድ፡ crates/sorafs_chunker/src/bin/export_vectors.rs:175 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ፡ ሳጥኖች/sorafs_chunker/tests/backpressure.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/sorafs_chunker/tests/vectors.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/sorafs_manifest/tests/por_fixtures.rs:11 — `env!("CARGO_MANIFEST_DIR"),`
- ሙከራ፡ ሳጥኖች/sorafs_manifest/tests/አቅራቢ_admission_fixtures.rs:12 — `cmd.current_dir(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ሳጥኖች/sorafs_manifest/tests/replication_order_fixtures.rs:8 — `env!("CARGO_MANIFEST_DIR"),`
- ፕሮድ፡ crates/sorafs_node/src/bin/sorafs_gateway.rs:55 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ፕሮድ፡ crates/sorafs_node/src/bin/sorafs_gateway.rs:59 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0")`
ሙከራ: ሳጥኖች/sorafs_node/src/gateway.rs:2006 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");`
- ሙከራ: ሳጥኖች/sorafs_node/tests/cli.rs:122 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/sorafs_node/tests/gateway.rs:14 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/sorafs_node/tests/gateway.rs:30 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ ሳጥኖች/ሶራፍስ_ኦርኬስትራ/src/lib.rs:6316 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
ሙከራ፡ ሳጥኖች/ሶራፍስ_ኦርኬስትራ/src/lib.rs:6449 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
ሙከራ፡ ሳጥኖች/ሶራፍስ_ኦርኬስትራ/src/lib.rs:8100 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ፡ ሳጥኖች/sorafs_orchestrator/tests/orchestrator_parity.rs:180 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ crates/soranet_pq/tests/kat_vectors.rs:9 — `env!("CARGO_MANIFEST_DIR"),`
ግንባታ፡ integration_tests/build.rs:17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_tests/src/sorafs_gateway_capability_refusal.rs:157 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- ሙከራ፡ ውህደት_tests/src/sorafs_gateway_conformance.rs:1032 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/አድራሻ_canonicalisation.rs:126 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_tests/tess/asset.rs:49 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_tests/tests/fast_dsl_build.rs:7 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ ውህደት_ፈተና/ፈተና/genesis_json.rs:16 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/genesis_json.rs:22 - `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
ሙከራ፡ ውህደት_ሙከራዎች/ፈተናዎች/iroha_cli.rs:24 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_tests/tests/ivm_header_decode.rs:49 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ ውህደት_ፈተና/ፈተና/ivm_header_smoke.rs:24 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:70 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:123 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:173 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ሙከራዎች/ፈተናዎች/nexus/cbdc_rollout_bundle.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/nexus/cbdc_whitelist.rs:26 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_tests/tests/nexus/global_commit.rs:17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- ሙከራ፡ ውህደት_tests/tests/nexus/lane_registry.rs:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_tests/tests/norito_burn_fixture.rs:19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ፈተና/ፈተናዎች/repo.rs:31 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ሙከራ፡ ውህደት_ሙከራዎች/ፈተናዎች/ዥረት/mod.rs:339 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:220 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:538 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
ሙከራ: mochi/mochi-core/src/torii.rs:4740 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
ሙከራ: mochi/mochi-core/src/torii.rs:4755 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
ሙከራ: mochi/mochi-core/src/torii.rs:4770 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
ሙከራ: mochi/mochi-core/src/torii.rs:4785 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ: mochi/mochi-integration/tests/supervisor.rs:168 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- ሙከራ፡ መሳሪያዎች/ሶራኔት-የእጅ መጨባበጥ-harness/tests/fixtures_verify.rs:6 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ፡ መሳሪያዎች/ሶራኔት-የእጅ መጨባበጥ-harness/tests/interop_parity.rs:77 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ሙከራ፡ መሳሪያዎች/ሶራኔት-የእጅ መጨባበጥ-harness/tests/perf_gate.rs:173 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
መሣሪያ፡ xtask/src/bin/control_plane_mock.rs:362 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
መሣሪያ፡ xtask/src/main.rs:11383 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
መሣሪያ፡ xtask/src/sorafs/gateway_fixture.rs:28 — `env!("CARGO_MANIFEST_DIR"),`
መሣሪያ፡ xtask/src/sorafs/gateway_fixture.rs:32 — `env!("CARGO_MANIFEST_DIR"),`
ሙከራ፡ xtask/tess/address_vectors.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/android_dashboard_parity_cli.rs፡7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tess/codec_rans_tables.rs:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/da_proof_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tess/iso_bridge_lint.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/ministry_agenda.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/sns_catalog_verify.rs:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ: xtask/tests/soradns_cli.rs:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tess/sorafs_fetch_fixture.rs:9 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/soranet_bug_bounty.rs:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/soranet_gateway_billing.rs:12 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/soranet_gateway_billing_m0.rs:30 — `let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/soranet_gateway_m1.rs:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/soranet_gateway_m2.rs:25 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tess/soranet_pop_template.rs:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/soranet_pop_template.rs:77 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tess/soranet_pop_template.rs:131 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tess/soranet_pop_template.rs:202 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/soranet_pop_template.rs:306 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/soranet_pop_template.rs:356 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tess/soranet_pop_template.rs:489 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/streaming_bundle_check.rs:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
ሙከራ፡ xtask/tests/streaming_entropy_bench.rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_VERSION (ምርት፡ 11፣ ሙከራ፡ 1፣ መሳሪያ፡ 1)- ፕሮድ፡ crates/iroha/src/client.rs:478 — `map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));`
- ፕሮድ፡ crates/iroha_cli/src/commands/sorafs.rs:1767 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- ፕሮድ፡ crates/iroha_cli/src/main_shared.rs:72 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- ፕሮድ፡ crates/iroha_cli/src/main_shared.rs:530 — `let client_version = env!("CARGO_PKG_VERSION");`
- ሙከራ: crates/iroha_cli/tests/cli_smoke.rs:359 — `let expected_version = env!("CARGO_PKG_VERSION");`
- ፕሮድ፡ crates/iroha_core/src/sumeragi/rbc_store.rs:40 — `version: env!("CARGO_PKG_VERSION").to_owned(),`
- ፕሮድ፡ crates/iroha_js_host/src/lib.rs:2998 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- ፕሮድ፡ crates/iroha_telemetry/src/ws.rs:243 — `env!("CARGO_PKG_VERSION")`
- ፕሮድ፡ crates/irohad/src/main.rs:502 — `version = env!("CARGO_PKG_VERSION"),`
- ፕሮድ፡ crates/irohad/src/main.rs:3986 — `version = env!("CARGO_PKG_VERSION"),`
- ፕሮድ፡ ሳጥኖች/ሶራፍስ_car/src/bin/sorafs_fetch.rs:1111 — `Value::from(env!("CARGO_PKG_VERSION")),`
- ፕሮድ፡ ሳጥኖች/ሶራፍስ_ኦርኬስትራ/src/bin/sorafs_cli.rs:93 — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
መሳሪያ፡ መሳሪያዎች/ቴሌሜትሪ-ሼማ-ዲፍ/src/main.rs፡248 — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## CARGO_PRIMARY_PACKAGE (ግንባታ፡ 1)

ግንባታ፡ ሳጥኖች/soranet_pq/build.rs:6 — `if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {`

## CARGO_TARGET_DIR (ምርት፡ 3፣ ሙከራ፡ 2፣ መሳሪያ፡ 1)

- ሙከራ፡ crates/iroha_test_network/src/lib.rs:524 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- ሙከራ፡ crates/iroha_test_network/src/lib.rs:759 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:575 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:629 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:683 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
መሣሪያ፡ xtask/src/mochi.rs:383 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## CARGO_WORKSPACE_DIR (ሙከራ፡ 1)

ሙከራ: crates/iroha_core/src/state.rs:12015 — `if let Some(workspace_dir) = option_env!("CARGO_WORKSPACE_DIR") {`

## CRYPTO_SM_INTRINSICS (ቤንች፡ 1)

- አግዳሚ ወንበር፡ crates/iroha_crypto/benches/sm_perf.rs:183 — `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`

## CUDA_HOME (ግንባታ፡ 2)

ግንባታ፡ crates/ fastpq_prover/build.rs:198 — `env::var_os("CUDA_HOME")`
ግንባታ፡ crates/norito/accelerators/jsonstage1_cuda/build.rs:63 — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (ግንባታ፡ 2)

ግንባታ፡ crates/ fastpq_prover/build.rs:199 — `.or_else(|| env::var_os("CUDA_PATH"))`
ግንባታ፡ crates/norito/accelerators/jsonstage1_cuda/build.rs:64 — `.or_else(|| env::var_os("CUDA_PATH"))`

## DATASPACE_ADVERSARIAL_ARTIFACT_DIR (ሙከራ፡ 1)

- ሙከራ፡ ውህደት_tests/tests/nexus/cross_lane.rs:686 — `if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {`

## DOCS_RS (ግንባታ፡ 1)

ግንባታ፡ crates/norito/build.rs:8 — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (ቤንች፡ 1)

- አግዳሚ ወንበር፡ ሳጥኖች/ኖሪቶ/ቤንች/enum_packed_bench.rs:75 — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_DEBUG_FUSED (ምርት፡ 1)

- ፕሮድ፡ crates/ fastpq_prover/src/trace.rs:98 — `Some(*DEBUG_FUSED_ENV.get_or_init(|| env::var_os("FASTPQ_DEBUG_FUSED").is_some()))`

## FASTPQ_EXPECTED_KIB (ሙከራ፡ 1)

- ሙከራ: crates/fastpq_prover/tests/perf_production.rs:88 — `env::var("FASTPQ_EXPECTED_KIB")`

## FASTPQ_EXPECTED_MS (ሙከራ፡ 1)

- ሙከራ: crates/fastpq_prover/tests/perf_production.rs:80 — `env::var("FASTPQ_EXPECTED_MS")`

## FASTPQ_PROOF_ROWS (ሙከራ፡ 1)

- ሙከራ: crates/fastpq_prover/tests/perf_production.rs:72 — `env::var("FASTPQ_PROOF_ROWS")`

## FASTPQ_SKIP_GPU_BUILD (ግንባታ፡ 1)

ግንባታ፡ crates/ fastpq_prover/build.rs:45 — `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## FASTPQ_UPDATE_FIXTURES (ሙከራ፡ 6)- ሙከራ: crates/fastpq_prover/tests/backend_regression.rs:47 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- ሙከራ: crates/fastpq_prover/tests/backend_regression.rs:69 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- ሙከራ፡ crates/ fastpq_prover/tests/proof_fixture.rs:43 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- ሙከራ: crates/fastpq_prover/tests/trace_commitment.rs:23 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- ሙከራ: crates/fastpq_prover/tests/trace_commitment.rs:111 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- ሙከራ፡ crates/ fastpq_prover/tests/transcript_replay.rs:67 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`

## GENESIS_DEBUG_MODE (ሙከራ፡ 1)

- ሙከራ፡ crates/iroha_test_network/emples/genesis_debug.rs:15 — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## ጀነሲስ_ዲቡግ_ጭነት (ሙከራ፡ 1)

- ሙከራ፡ crates/iroha_test_network/emples/genesis_debug.rs:122 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## GITHUB_ደረጃ_ማጠቃለያ (ምርት፡ 2)

- ፕሮድ፡ crates/iroha_crypto/src/bin/gost_perf_check.rs:22 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- ፕሮድ፡ crates/iroha_crypto/src/bin/sm_perf_check.rs:205 — `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## GIT_COMMIT_HASH (ምርት፡ 1)

- ፕሮድ፡ crates/iroha_core/src/sumeragi/rbc_store.rs:42 — `git_commit: option_env!("GIT_COMMIT_HASH").map(str::to_owned),`

## ቤት (ምርት፡ 1)

- ፕሮድ፡ crates/iroha/src/config.rs:57 — `env::var_os("HOME").map(PathBuf::from)`

## IROHA_ALLOW_NET (ሙከራ፡ 1)

ሙከራ: crates/izanami/src/chaos.rs:374 — `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## IROHA_CONF_GAS_SEED (ሙከራ፡ 1)

- ሙከራ፡ crates/iroha_test_samples/src/lib.rs:57 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_DA_SPOOL_DIR (ፈተና፡ 1)

ሙከራ፡ crates/iroha_core/src/state.rs:9096 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_METRICS_PANIC_ON_DUPLICATE (ሙከራ፡ 2)

- ሙከራ፡ crates/iroha_telemetry/src/metrics.rs:11397 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- ሙከራ፡ crates/iroha_torii/tests/metrics_registry.rs:33 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## IROHA_run_ ችላ (ፈተና፡ 71)- ሙከራ፡ crates/iroha_core/tests/gov_auto_close_approv.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_finalize_real_vk.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ፡ crates/iroha_core/tests/gov_min_duration.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_mode_mismatch.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_mode_mismatch_zk.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_plain_ballot.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_plain_conviction.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_plain_disabled.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_plain_missing_ref.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_plain_revote_monotonic.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_protected_gate.rs:64 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_referendum_open_close.rs:24 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_thresholds.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_thresholds.rs:82 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_thresholds_positive.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_unlock_sweep.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_zk_ballot_lock_verified.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_zk_ballot_real_vk.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/gov_zk_referendum_window_guard.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/zk_roots_get_cap.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_core/tests/zk_vote_get_tally.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ: crates/iroha_crypto/src/merkle.rs:1317 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ: crates/iroha_crypto/src/merkle.rs:1332 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ፡ ሳጥኖች/iroha_crypto/tests/merkle_norito_roundtrip.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ሳጥኖች/iroha_crypto/tests/merkle_norito_roundtrip.rs:42 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ: ሳጥኖች/iroha_data_model/src/block/header.rs:624 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ: ሳጥኖች/iroha_data_model/src/isi/mod.rs:1888 — `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- ሙከራ፡ crates/iroha_data_model/src/isi/register.rs:324 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ: ሳጥኖች/iroha_data_model/src/proof.rs:798 - `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- ሙከራ: ሳጥኖች/iroha_data_model/src/transaction/signed.rs:1048 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ሳጥኖች/ኢሮሃ_ዳታ_ሞዴል/ፈተናዎች/መመሪያ_ሬጅስትሪ_lazy_init.rs፡8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/instruction_registry_reset.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ: ሳጥኖች/iroha_data_model/tests/model_derive_repro.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ: ሳጥኖች/iroha_data_model/tests/model_derive_repro.rs:37 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ: ሳጥኖች/iroha_data_model/tests/model_derive_repro.rs:58 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ: ሳጥኖች/iroha_data_model/tests/model_derive_repro.rs:84 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ፡ ሳጥኖች/iroha_data_model/tests/registry_decode_roundtrip.rs:9 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/trait_objects.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ሳጥኖች/iroha_data_model/tests/trait_objects.rs:27 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`- ሙከራ፡ crates/iroha_data_model/tests/zk_envelope_roundtrip.rs:6 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/contracts_activate_integration.rs:23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/contracts_activate_integration.rs:174 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/contracts_call_integration.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ሳጥኖች/iroha_torii/tests/contracts_deploy_integration.rs:23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ሳጥኖች/iroha_torii/tests/contracts_intance_activate_integration.rs:19 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/contracts_instances_list_router.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/gov_council_persist_integration.rs:22 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/gov_council_vrf.rs:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/gov_enact_handler.rs:12 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/gov_intances_list.rs:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/gov_mode_mismatch_and_autoclose.rs:42 — `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/gov_protected_endpoints.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/gov_protected_endpoints_router.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/iroha_torii/tests/gov_read_endpoints_router.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/ivm/tests/beep_test.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ፡ ሳጥኖች/ivm/tests/kotodama_struct_fields.rs:11 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/ivm/tests/zk_roots_and_vote_syscalls.rs:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ crates/ivm/tests/zk_roots_and_vote_syscalls.rs:50 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_ፈተና/ፈተናዎች/ክስተቶች/notification.rs:81 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ፡ ውህደት_ሙከራዎች/ፈተናዎች/extra_functional/unstable_network.rs፡753 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ፡ ውህደት_ሙከራዎች/ፈተናዎች/extra_functional/unstable_network.rs፡769 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/extra_functional/unstable_network.rs:785 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ፡ ውህደት_ሙከራዎች/ፈተናዎች/extra_functional/unstable_network.rs፡801 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
ሙከራ፡ ውህደት_ሙከራዎች/ፈተናዎች/extra_functional/unstable_network.rs:817 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_tests/tess/permissions.rs፡202 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_tests/tess/permissions.rs:265 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_tests/tess/permissions.rs:328 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/ቧንቧ_ብሎክ_rejected.rs:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_tests/tess/sorting.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_ፈተና/ፈተናዎች/ቀስቃሾች/በካll_trigger.rs:139 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ሙከራ፡ ውህደት_ፈተና/ፈተናዎች/ቀስቃሾች/time_trigger.rs:149 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_RUN_ZK_WRAPPERS (ሙከራ፡ 1)

ሙከራ፡ crates/ivm/tests/kotodama_wrappers.rs:2 — `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`

## IROHA_ቼኮች_ዝለል (ሙከራ፡ 1)

ሙከራ፡ crates/iroha_test_network/src/lib.rs:3098 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (ሙከራ፡ 1)

ሙከራ: ሳጥኖች/iroha_crypto/tests/sm_cli_matrix.rs:47 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_TEST_DUMP_GENESIS (ፈተና፡ 1)

ሙከራ፡ crates/iroha_test_network/src/lib.rs:6661 — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (ግንባታ፡ 1፣ ሙከራ፡ 1)

- ሙከራ: crates/iroha_test_network/src/config.rs:155 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
ግንባታ፡ integration_tests/build.rs:205 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_ዝለል_ግንባ(ሙከራ፡ 1)

ሙከራ፡ crates/iroha_test_network/src/lib.rs:1244 — `std::env::var("IROHA_TEST_SKIP_BUILD")`

## IROHA_TEST_TARGET_DIR (ሙከራ፡ 2)

- ሙከራ፡ crates/iroha_test_network/src/lib.rs:521 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`
- ሙከራ፡ crates/iroha_test_network/src/lib.rs:765 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`

## IROHA_TEST_አጠቃቀም_ነባሪ_EXECUTOR (ሙከራ፡ 3)

ሙከራ: crates/iroha_core/src/executor.rs:2383 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- ሙከራ: crates/iroha_core/src/executor.rs:2520 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
ሙከራ: crates/iroha_core/src/state.rs:12008 — `if std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR").is_some() {`

## IROHA_TORII_OPENAPI_ACTUAL (ሙከራ፡ 1)

ሙከራ፡ ሳጥኖች/iroha_torii/tests/router_feature_matrix.rs:94 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_EXPECTED (ፈተና፡ 2)

ሙከራ፡ ሳጥኖች/iroha_torii/tests/router_feature_matrix.rs:88 — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
ሙከራ፡ ሳጥኖች/iroha_torii/tests/router_feature_matrix.rs:104 — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_TORII_OPENAPI_TOKENS (መሳሪያ፡ 2)

መሣሪያ፡ xtask/src/main.rs:11025 — `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`
መሣሪያ፡ xtask/src/main.rs:11077 — `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IVM_BIN (ሙከራ፡ 2)

- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:60 — `let ivm_bin = env::var("IVM_BIN")`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:163 — `let ivm_bin = env::var("IVM_BIN")`

## IVM_COMPILER_DEBUG (ምርት፡ 1)

- ፕሮድ፡ crates/kotodama_lang/src/compiler.rs:4094 — `let compiler_debug = if std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_GENCODE (ግንባታ፡ 1)

ግንባታ፡ crates/ivm/build.rs:35 — `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| "arch=compute_61,code=sm_61".to_string());`

## IVM_CUDA_NVCC (ግንባታ፡ 1)

ግንባታ፡ crates/ivm/build.rs:30 — `let nvcc = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVCC_EXTRA (ግንባታ፡ 1)

ግንባታ፡ crates/ivm/build.rs:36 — `let extra_flags: Vec<String> = env::var("IVM_CUDA_NVCC_EXTRA")`

## IVM_DEBUG_IR (ሙከራ፡ 1)

- ሙከራ፡ crates/ivm/tests/debug_contains.rs:15 — `if std::env::var_os("IVM_DEBUG_IR").is_some() {`

## IVM_DEBUG_METAL_ENUM (ስህተት: 1)

- ማረም፡ crates/ivm/src/vector.rs:474 — `std::env::var("IVM_DEBUG_METAL_ENUM")`

## IVM_DEBUG_METAL_SELFTEST (ማረም፡ 1)

- ማረም፡ crates/ivm/src/vector.rs:1205 — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DISABLE_CUDA (ማረም፡ 1)

- ማረም፡ crates/ivm/src/cuda.rs:315 — `&& std::env::var("IVM_DISABLE_CUDA")`

## IVM_DISABLE_METAL (ማረም፡ 1)

- ማረም፡ crates/ivm/src/vector.rs:299 — `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELFTEST_FAIL (ማረም፡ 1)

- ማረም፡ crates/ivm/src/cuda.rs:326 — `&& std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL")`

## IVM_FORCE_METAL_ENUM (ስህተት: 1)

- ማረም፡ crates/ivm/src/vector.rs:444 — `std::env::var("IVM_FORCE_METAL_ENUM")`

## IVM_FORCE_METAL_SELFTEST_FAIL (ማረም፡ 1)

- ማረም፡ crates/ivm/src/vector.rs:1192 — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IVM_TOOL_BIN (ሙከራ፡ 1)

- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:113 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## IZANAMI_ALLOW_NET (ሙከራ፡ 1)

ሙከራ: crates/izanami/src/chaos.rs:373 — `std::env::var("IZANAMI_ALLOW_NET")`

## IZANAMI_TUI_ALLOW_ZERO_SEED (prod: 1)

- ፕሮድ፡ crates/izanami/src/tui.rs:134 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (ግንባታ፡ 1)

ግንባታ፡ crates/norito/accelerators/jsonstage1_cuda/build.rs:40 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_SKIP_BUILD (ግንባታ፡ 1)

ግንባታ፡ crates/norito/accelerators/jsonstage1_cuda/build.rs:18 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KOTO_BIN (ሙከራ፡ 2)- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:50 — `let koto_bin = env::var("KOTO_BIN")`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:154 — `let koto_bin = env::var("KOTO_BIN")`

## LANG (ሙከራ፡ 3)

- ሙከራ: crates/ivm/src/bin/koto_lint.rs:702 — `let previous = env::var("LANG").ok();`
ሙከራ፡ ሳጥኖች/ivm/tess/i18n.rs:11 — `let old_lang = env::var("LANG").ok();`
ሙከራ፡ ሳጥኖች/ivm/tess/i18n.rs:69 — `let old_lang = env::var("LANG").ok();`

## LC_ALL (ሙከራ፡ 2)

ሙከራ፡ ሳጥኖች/ivm/tess/i18n.rs:12 — `let old_lc_all = env::var("LC_ALL").ok();`
ሙከራ፡ ሳጥኖች/ivm/tess/i18n.rs:70 — `let old_lc_all = env::var("LC_ALL").ok();`

## LC_MESSAGES (ሙከራ፡ 2)

ሙከራ፡ ሳጥኖች/ivm/tess/i18n.rs:13 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`
ሙከራ፡ ሳጥኖች/ivm/tess/i18n.rs:71 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## ማክስ_ዲግሪ (ምርት፡ 1)

- ፕሮድ፡ crates/iroha_core/src/zk.rs:106 — `let current = std::env::var("MAX_DEGREE")`

## MOCHI_CONFIG (ምርት፡ 1)

ፕሮድ፡ mochi/mochi-ui-egui/src/config.rs:325 — `if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## MOCHI_DATA_ROOT (ምርት፡ 1)

ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:2068 — `std::env::var_os("MOCHI_DATA_ROOT")`

## MOCHI_TEST_የውስጥ_ጀነሲስን_ተጠቀም (ምርት፡ 1)

ፕሮድ፡ mochi/mochi-core/src/supervisor.rs፡1994 — `if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {`

## NORITO_BENCH_SUMMARY (ቤንች፡ 1)

- አግዳሚ ወንበር፡ crates/norito/benches/parity_compare.rs:70 — `if std::env::var("NORITO_BENCH_SUMMARY").ok().as_deref() == Some("1") {`

## NORITO_CPU_INFO (መሳሪያ፡ 1)

መሣሪያ፡ xtask/src/stage1_bench.rs:69 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_GPU_LIB (ምርት፡ 1)

- ፕሮድ፡ crates/norito/src/core/simd_crc64.rs:215 — `std::env::var("NORITO_CRC64_GPU_LIB").ok(),`

## ኖሪቶ_ማሰናከል_የታሸገ_ግንባታ (ምርት፡ 1፣ ሙከራ፡ 1)

- ፕሮድ፡ crates/iroha_js_host/src/lib.rs:183 — `let env_set = std::env::var_os("NORITO_DISABLE_PACKED_STRUCT").is_some();`
ሙከራ፡ ሳጥኖች/norito/src/lib.rs:322 — `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## NORITO_GPU_CRC64_MIN_BYTES (ምርት፡ 1)

- ፕሮድ፡ crates/norito/src/core/simd_crc64.rs:83 — `std::env::var("NORITO_GPU_CRC64_MIN_BYTES").ok(),`

## NORITO_PAR_STAGE1_MIN (ሙከራ፡ 1)

ሙከራ፡ ሳጥኖች/norito/src/lib.rs:4847 — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## NORITO_SKIP_BINDINGS_SYNC (ግንባታ፡ 1)

ግንባታ፡ crates/norito/build.rs:12 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_ጂፒዩ_MIN_BYTES (ሙከራ፡ 1)

ሙከራ፡ ሳጥኖች/norito/src/lib.rs:4881 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## NORITO_TRACE (ሙከራ፡ 2)

ሙከራ፡ ሳጥኖች/norito/src/lib.rs:123 — `std::env::var_os("NORITO_TRACE").is_some()`
ሙከራ፡ ሳጥኖች/norito/src/lib.rs:138 — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## ምንም_ፕሮክሲ (ምርት፡ 1)

- ፕሮድ፡ crates/iroha_p2p/src/transport.rs:469 — `let no_proxy = env::var("NO_PROXY")`

## NVCC (ግንባታ፡ 1)

ግንባታ፡ crates/ivm/build.rs:31 — `.or_else(|_| env::var("NVCC"))`

## OUT_DIR (ግንባታ፡ 4፣ ፕሮድ፡ 12፣ ሙከራ፡ 2)ግንባታ፡ crates/ fastpq_prover/build.rs:99 — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
ግንባታ፡ crates/iroha_data_model/build.rs:12 — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));`
- ፕሮድ፡ crates/iroha_data_model/src/lib.rs:179 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
ግንባታ፡ crates/ivm/build.rs:27 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
ግንባታ፡ crates/ivm/build.rs:120 — `if let Some(out_dir) = env::var_os("OUT_DIR") {`
- ፕሮድ፡ crates/ivm/src/cuda.rs:12 — `static PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:13 — `static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:14 — `static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:15 — `static SHA_LEAVES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:16 — `static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:17 — `static SHA3_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:18 — `static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:19 — `static BN254_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:20 — `static SIG_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/signature.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:21 — `static SHA_PAIRS_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx"));`
- ፕሮድ፡ crates/ivm/src/cuda.rs:22 — `static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));`
ሙከራ፡ crates/ivm/src/ptx_tests.rs:7 — `let out_dir = match std::env::var("OUT_DIR") {`
ሙከራ፡ crates/ivm/tests/ptx_kernels.rs:5 — `let out_dir = env!("OUT_DIR");`

## P2P_TURN (ምርት: 1)

- ፕሮድ፡ crates/iroha_p2p/src/transport.rs:296 — `let endpoint = std::env::var("P2P_TURN")`

## PATH (ምርት፡ 2፣ ሙከራ፡ 1)

- ፕሮድ፡ crates/iroha_cli/src/commands/sorafs.rs:7276 — `if let Some(path_var) = env::var_os("PATH") {`
- ሙከራ፡ ውህደት_ፈተና/ፈተና/kotodama_examples.rs:17 — `let path = env::var_os("PATH")?;`
ፕሮድ፡ mochi/mochi-core/src/supervisor.rs:511 — `let path_var = env::var_os("PATH")?;`

## PRINT_SORACLES_FIXTURES (ሙከራ፡ 1)

ሙከራ: ሳጥኖች/iroha_data_model/src/oracle/mod.rs:3249 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORII_SPEC (ሙከራ፡ 1)

- ሙከራ: crates/iroha_torii/src/openapi.rs:4380 — `if std::env::var("PRINT_TORII_SPEC").is_ok() {`

## መገለጫ (ግንባታ፡ 1፣ ፕሮድ፡ 1፣ ሙከራ፡ 1)

- ፕሮድ፡ crates/iroha_core/src/sumeragi/rbc_store.rs:41 — `profile: option_env!("PROFILE").unwrap_or("unknown").to_owned(),`
- ሙከራ፡ crates/iroha_test_network/src/lib.rs:995 — `let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());`
ግንባታ፡ integration_tests/build.rs:180 — `let profile = match env::var("PROFILE").unwrap_or_default().as_str() {`

## PYTHON3 (ሙከራ፡ 2)

- ሙከራ: ሳጥኖች/ሶራፍስ_car/ፈተናዎች/taikai_car_cli.rs:235 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`
- ሙከራ፡ ሳጥኖች/ሶራፍስ_car/tests/taikai_viewer_cli.rs:30 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONPATH (ሙከራ፡ 1)

- ሙከራ፡ crates/iroha_cli/tests/cli_smoke.rs:5245 — `match env::var("PYTHONPATH") {`

## RBC_SESSION_PATH (ሙከራ፡ 1)

ሙከራ፡ crates/iroha_core/src/sumeragi/rbc_store.rs:823 — `let path = std::env::var("RBC_SESSION_PATH").expect("set RBC_SESSION_PATH");`

## REPO_PROOF_DIGEST_ውጭ (ሙከራ፡ 1)

- ሙከራ: crates/iroha_core/src/smartcontracts/isi/repo.rs:2017 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_መውጣቱ (ሙከራ፡ 1)

ሙከራ፡ ሳጥኖች/iroha_core/src/smartcontracts/isi/repo.rs፡2005 — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUST_LOG (ምርት፡ 1፣ ሙከራ፡ 2)

ሙከራ፡ crates/iroha_test_network/src/lib.rs:5414 — `let original = env::var("RUST_LOG").ok();`
ሙከራ፡ crates/iroha_test_network/src/lib.rs:5431 — `let original = env::var("RUST_LOG").ok();`
- ፕሮድ፡ crates/izanami/src/config.rs:269 — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`

## SM_PERF_CPU_LABEL (ምርት፡ 2)

- ፕሮድ፡ crates/iroha_crypto/src/bin/sm_perf_check.rs:637 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- ፕሮድ፡ crates/iroha_crypto/src/bin/sm_perf_check.rs:679 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## SORAFS_NODE_ዝለል_ፈተናዎች (ፈተና፡ 1)

ሙከራ: ሳጥኖች/sorafs_node/tests/cli.rs:17 — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## SORAFS_TORII_ዝለል_እጅግ_ፈተናዎች (ሙከራ፡ 1)

- ሙከራ፡ crates/iroha_torii/tests/sorafs_discovery.rs:95 — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`## SUMERAGI_ADVERSARIAL_ARTIFACT_DIR (ሙከራ፡ 1)

- ሙከራ፡ ውህደት_tests/tests/sumeragi_adversarial.rs:1222 — `let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {`

## SUMERAGI_BASELINE_ARTIFACT_DIR (ምርት፡ 1፣ ሙከራ፡ 1)

- ምርት፡ ሳጥኖች/build-support/src/bin/sumeragi_baseline_report.rs:40 — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`
- ሙከራ፡ ውህደት_tests/tests/sumeragi_npos_performance.rs:1396 — `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`

## SUMERAGI_DA_ARTIFACT_DIR (ምርት፡ 1፣ ፈተና፡ 1)

- ምርት፡ ሳጥኖች/build-support/src/bin/sumeragi_da_report.rs:41 — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`
ሙከራ፡ ውህደት_ፈተና/ፈተና/sumeragi_da.rs፡1599 — `let Ok(dir) = std::env::var("SUMERAGI_DA_ARTIFACT_DIR") else {`

## SystemRoot (ምርት፡ 1)

- ፕሮድ፡ crates/ fastpq_prover/src/backend.rs:535 — `env::var_os("SystemRoot").map(PathBuf::from)`

## ዒላማ (መሳሪያ፡ 2)

መሣሪያ፡ xtask/src/poseidon_bench.rs:87 — `target: std::env::var("TARGET")`
መሣሪያ፡ xtask/src/stage1_bench.rs:63 — `target: std::env::var("TARGET")`

## የሙከራ_ምዝግብ ማስታወሻ (ምርት፡ 1)

- ፕሮድ፡ crates/iroha_logger/src/lib.rs:89 — `filter: std::env::var("TEST_LOG_FILTER")`

## የሙከራ_ምዝግብ ማስታወሻ (ምርት፡ 1)

- ፕሮድ፡ crates/iroha_logger/src/lib.rs:85 — `level: std::env::var("TEST_LOG_LEVEL")`

## መረብ_መረብ_ካርጎ (ሙከራ፡ 1)

ሙከራ፡ crates/iroha_test_network/src/lib.rs:865 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TORII_DEBUG_SORT (ምርት፡ 1)

- ፕሮድ፡ crates/iroha_torii/src/routing.rs:12530 — `std::env::var("TORII_DEBUG_SORT").ok().is_some(),`

## TORII_MOCK_HARNESS_METRICS_PATH (መሳሪያ፡ 1)

መሣሪያ፡ xtask/src/bin/torii_mock_harness.rs:106 — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (መሳሪያ፡ 1)

መሣሪያ፡ xtask/src/bin/torii_mock_harness.rs:109 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_ድጋሚ_ጠቅላላ (መሳሪያ፡ 1)

መሣሪያ፡ xtask/src/bin/torii_mock_harness.rs:297 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (መሳሪያ፡ 1)

መሣሪያ፡ xtask/src/bin/torii_mock_harness.rs:112 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (መሳሪያ፡ 1)

መሣሪያ፡ xtask/src/bin/torii_mock_harness.rs:104 — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## TORII_OPENAPI_TOKEN (መሳሪያ፡ 2)

መሣሪያ፡ xtask/src/main.rs:11020 — `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`
መሣሪያ፡ xtask/src/main.rs:11073 — `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## ማሻሻያዎች (ሙከራ፡ 1)

- ሙከራ፡ crates/iroha_core/tests/snapshots.rs:42 — `let update = env::var("UPDATE_FIXTURES")`

## USERPROFILE (ምርት፡ 1)

- ፕሮድ፡ crates/iroha/src/config.rs:55 — `env::var_os("USERPROFILE").map(PathBuf::from)`

## VERGEN_CARGO_FEATURES (ምርት፡ 1)

- ፕሮድ፡ crates/irohad/src/main.rs:4850 — `const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {`

## VERGEN_CARGO_TARGET_triPLE (ምርት፡ 1)

- ፕሮድ፡ crates/iroha_telemetry/src/ws.rs:238 — `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");`

## VERGEN_GIT_SHA (ምርት፡ 4)

- ፕሮድ፡ crates/iroha_cli/src/main_shared.rs:57 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`
- ፕሮድ፡ crates/iroha_telemetry/src/ws.rs:237 — `let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");`
- ፕሮድ፡ crates/iroha_torii/src/routing.rs:32515 — `git_sha: option_env!("VERGEN_GIT_SHA")`
- ፕሮድ፡ crates/irohad/src/main.rs:4845 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`

## አረጋግጥ (ቤንች፡ 1)

- አግዳሚ ወንበር፡ crates/ivm/benches/bench_voting.rs:225 — `let verify_batch = std::env::var("VERIFY_BATCH")`

## ሁሉንም_አረጋግጥ (ቤንች፡ 1)

- አግዳሚ ወንበር፡ crates/ivm/benches/bench_voting.rs:213 — `let verify_every: u64 = std::env::var("VERIFY_EVERY")`

## ድምጽ ሰጪዎች (ቤንች፡ 1)

- አግዳሚ ወንበር፡ crates/ivm/benches/bench_voting.rs:207 — `let voters: u64 = std::env::var("VOTERS")`

## ምንም_ፕሮክሲ (ፕሮድ፡ 1)

- ፕሮድ፡ crates/iroha_p2p/src/transport.rs:471 — `.or_else(|| env::var("no_proxy").ok());`