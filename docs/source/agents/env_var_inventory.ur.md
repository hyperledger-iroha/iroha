---
lang: ur
direction: rtl
source: docs/source/agents/env_var_inventory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0a23896374a10f0862aeec69c607ba3022d3b337ae7f6bb54a61b8f7424410
source_last_modified: "2026-01-21T10:22:44.993254+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ماحولیات ٹوگل انوینٹری

`python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`_ کے ذریعے تازہ ترین

کل حوالہ جات: ** 505 ** · منفرد متغیرات: ** 137 **

## ایکشن_ڈ_ٹوکین_رکیوسٹ_ٹوکن (پروڈ: 1)

- پروڈ: کریٹس/sorafs_orchestrator/src/bin/sorafs_cli.rs: 313 - `let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {`

## ایکشن_ڈ_ٹوکین_رکیوسٹ_ورل (پروڈ: 1)

- پروڈ: کریٹس/sorafs_orchestrator/src/bin/sorafs_cli.rs: 310 - `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`

## بلاک_ڈمپ_ہائٹس (مثال کے طور پر: 1)

- مثال کے طور پر: کریٹس/آئروہ_کور/مثالوں/بلاک_ڈمپ۔ آر ایس: 62 - `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`

## block_dump_sum_asset (مثال کے طور پر: 1)

- example: crates/iroha_core/examples/block_dump.rs:51 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`

## بلاک_ڈمپ_برز (مثال کے طور پر: 1)

- مثال کے طور پر: کریٹس/آئروہ_کور/مثالوں/بلاک_ڈمپ ڈاٹ آر ایس: 50 - `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`

## کارگو (پروڈ: 3 ، ٹیسٹ: 2)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/lib.rs: 1026 - `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- ٹیسٹ: کریٹس/sorafs_manifest/tests/provider_admission_fixtures.rs: 11 - `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 547- `let cargo = env::var_os("CARGO")`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 602- `let cargo = env::var_os("CARGO")`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 656- `let cargo = env::var_os("CARGO")`

## cargo_bin_exe_iroha (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/اروہ_ سی ایل آئی/ٹیسٹ/CLI_SMOKE.RS: 40 - `env!("CARGO_BIN_EXE_iroha")`
- ٹیسٹ: کریٹس/اروہ_ سی ایل آئی/ٹیسٹ/تائیکائی_پولیسی۔ آر ایس: 20 - `env!("CARGO_BIN_EXE_iroha")`

## cargo_bin_exe_iroha_monitor (ٹیسٹ: 4)

- ٹیسٹ: کریٹس/آئروہ_مونیٹر/ٹیسٹ/اٹیچ_رینڈر۔ آر ایس: 11 - `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- ٹیسٹ: کریٹس/آئروہ_مونیٹر/ٹیسٹ/http_limits.rs: 10 - `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- ٹیسٹ: کریٹس/آئروہ_مونیٹر/ٹیسٹ/infalid_credentials.rs: 9 - `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- ٹیسٹ: کریٹس/آئروہ_مونیٹر/ٹیسٹ/دھواں۔ آر ایس: 9 - `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## cargo_bin_exe_kagami (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/اروہ_کاگامی/ٹیسٹ/عام/Mod.rs: 22 - `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- ٹیسٹ: کریٹس/آئروہ_کاگامی/ٹیسٹ/پاپ_مبڈ۔ آر ایس: 34 - `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`

## cargo_bin_exe_kagami_mock (ٹیسٹ: 1)

- ٹیسٹ: موچی/موچی انٹیگریشن/ٹیسٹ/سپروائزر۔ آر ایس: 33- `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## cargo_bin_exe_koto_compile (ٹیسٹ: 3)

- ٹیسٹ: کریٹس/IVM/ٹیسٹ/CLI_SMOKE.RS: 8 - `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/CLI_SMOKE.RS: 56 - `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/CLI_SMOKE.RS: 88 - `let bin = env!("CARGO_BIN_EXE_koto_compile");`

## cargo_bin_exe_sorafs_chunk_dump (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/sorafs_chunker/tests/one_gib.rs: 103 - `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## cargo_bin_exe_sorafs_cli (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/sorafs_car/ٹیسٹ/sorafs_cli.rs: 42 - `let path = env::var("CARGO_BIN_EXE_sorafs_cli")`

## cargo_bin_exe_sorafs_fetch (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/sorafs_car/src/bin/sorafs_fetch.rs: 2831 - `if let Ok(path) = env::var("CARGO_BIN_EXE_sorafs_fetch") {`

## cargo_bin_exe_taikai_car (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/sorafs_car/tests/sorafs_cli.rs: 48 - `let path = env::var("CARGO_BIN_EXE_taikai_car")`

## cargo_bin_name (prod: 3)

- پروڈ: کریٹس/آئروہ_ سی ایل آئی/ایس آر سی/مین_شریڈ۔ آر ایس: 63 - `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`
- پروڈ: کریٹس/آئروہ_ سی ایل آئی/ایس آر سی/مین_شریڈ۔ آر ایس: 72 - `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- پروڈ: کریٹس/آئروہاد/ایس آر سی/مین۔ آر ایس: 3176 - `let build_line = BuildLine::from_bin_name(env!("CARGO_BIN_NAME"));`

## cargo_build_target (ٹول: 2)

- ٹول: XTASK/SRC/poseidon_bench.rs: 88 - `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- ٹول: XTASK/SRC/STEG1_BENCH.RS: 64 - `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## cargo_cfg_target_arch (prod: 2 ، ٹول: 2)- prod: crates/iroha_crypto/src/bin/sm_perf_check.rs:632 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- پروڈ: کریٹس/آئروہ_کرپٹو/ایس آر سی/بن/ایس ایم_پرف_چیک. آر ایس: 668 - `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- ٹول: XTASK/SRC/poseidon_bench.rs: 89 - `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- ٹول: XTASK/SRC/STEG1_BENCH.RS: 65 - `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## cargo_cfg_target_os (تعمیر: 1 ، پروڈ: 2 ، ٹول: 2)

- بلڈ: کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 27 - `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();`
- پروڈ: کریٹس/آئروہ_کرپٹو/ایس آر سی/بن/ایس ایم_پیرف_چیک۔ آر ایس: 633 - `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- پروڈ: کریٹس/آئروہ_کرپٹو/ایس آر سی/بن/ایس ایم_پیرف_چیک۔ آر ایس: 669 - `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- ٹول: XTASK/SRC/poseidon_bench.rs: 91 - `os: std::env::var("CARGO_CFG_TARGET_OS")`
- ٹول: ایکس ٹی ایس اے ایس سی/ایس آر سی/اسٹیج 1_بینچ۔ آر ایس: 67 - `os: std::env::var("CARGO_CFG_TARGET_OS")`

## cargo_feature_cuda (تعمیر: 2)

- بلڈ: کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 25 - `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
- بلڈ: کریٹس/ivm/build.rs: 12 - `if env::var_os("CARGO_FEATURE_CUDA").is_some()`

## cargo_feature_cuda_kernel (تعمیر: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:12 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## cargo_feature_fastpq_gpu (تعمیر: 1)

- بلڈ: کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 26 - `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## cargo_feature_ffi_export (prod: 1)

- پروڈ: کریٹس/بلڈ سپورٹ/ایس آر سی/لیب۔ آر ایس: 30- `let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();`

## cargo_feature_ffi_import (prod: 1)

- پروڈ: کریٹس/بلڈ سپورٹ/ایس آر سی/لیب۔ آر ایس: 29- `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`

## cargo_manifest_dir (بینچ: 4 ، بلڈ: 5 ، مثال کے طور پر: 1 ، پروڈ: 27 ، ٹیسٹ: 164 ، ٹول: 4)- پروڈ: کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/پوسیڈون_مینیفیسٹ۔ آر ایس: 10 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/پیکنگ۔ آر ایس: 17 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/پوسیڈون_ مینفیسٹ_کنسٹنسی۔ آر ایس: 9 - `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/پوسیڈون_ مینفیسٹ_کنسٹنسی۔ آر ایس: 28 - `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/پروف_فکسچر۔ آر ایس: 9 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/ٹریس_کومیٹمنٹ۔ آر ایس: 14 - `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/ٹرانسکرپٹ_ری پلے۔ آر ایس: 56 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/اروہ/ایس آر سی/کلائنٹ۔ آر ایس: 8409 - `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/اروہ/ایس آر سی/ایس ایم آر ایس: 201 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/اروہ/ٹیسٹ/sm_signing.rs: 35 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ سی ایل آئی/ایس آر سی/کمپیوٹ۔ آر ایس: 518 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- پروڈ: کریٹس/آئروہ_ سی ایل آئی/ایس آر سی/مین_شریڈ
- ٹیسٹ: کریٹس/اروہ_ سی ایل آئی/ٹیسٹ/CLI_SMOKE.RS: 111 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/اروہ_ سی ایل آئی/ٹیسٹ/سی ایل آئی_سموک۔ آر ایس: 5236 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- پروڈ: کریٹس/آئروہ_کونفگ/ایس آر سی/پیرامیٹرز/صارف۔ آر ایس: 3490 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_کونفگ/ایس آر سی/پیرامیٹرز/صارف۔ آر ایس: 11561 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_کونفگ/ٹیسٹ/فاسٹ پی کیو_کیو_ اوور رائڈس۔ آر ایس: 15 - `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_کونفگ/ٹیسٹ/فکسچر۔ آر ایس: 36 - `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- بینچ: کریٹس/آئروہ_کور/بینچ/بلاکس/مشترکہ۔ آر ایس: 261 - `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- بینچ: کریٹس/آئروہ_کور/بینچز/بلاکس/عام/Mod.rs: 272 - `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- بینچ: کریٹس/آئروہ_کور/بینچ/توثیق۔ آر ایس: 96 - `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- بلڈ: کریٹس/آئروہ_کور/بلڈ آر آر ایس: 19 - `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- مثال کے طور پر: کریٹس/آئروہ_کور/مثالوں/جنریٹ_پیریٹی_فکسچرز۔ آر ایس: 17 - `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/ایگزیکیوٹر۔ آر ایس: 2385 - `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/ایگزیکیوٹر۔ آر ایس: 2521 - `let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/ایگزیکیوٹر۔ آر ایس: 2661 - `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- پروڈ: کریٹس/آئروہ_کور/ایس آر سی/اسمارٹ کنٹرکٹس/آئی ایس آئی/آف لائن۔ آر ایس: 2152 - `env!("CARGO_MANIFEST_DIR"),`
- پروڈ: کریٹس/اروہہ_کور/ایس آر سی/اسمارٹ کنٹریکٹ/آئی ایس آئی/آف لائن۔ آر ایس: 2156 - `env!("CARGO_MANIFEST_DIR"),`
- پروڈ: کریٹس/آئروہ_کور/ایس آر سی/اسمارٹ کنٹرکٹس/آئی ایس آئی/آف لائن۔ آر ایس: 2164 - `env!("CARGO_MANIFEST_DIR"),`
- پروڈ: کریٹس/آئروہ_کور/ایس آر سی/اسمارٹ کنٹریکٹرز/آئی ایس آئی/آف لائن۔ آر ایس: 2171 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسمارٹ کنٹریکٹ/آئی ایس آئی/آف لائن. آر ایس: 4852 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسمارٹ کنٹریکٹ/آئی ایس آئی/ریپو آر آر ایس: 1870 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسمارٹ کنٹریکٹ/آئی ایس آئی/ریپو آر آر ایس: 1874 - `env!("CARGO_MANIFEST_DIR"),`
- پروڈ: کریٹس/آئروہ_کور/ایس آر سی/اسٹیٹ۔ آر ایس: 9990 - `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسٹیٹ۔ آر ایس: 12012 - `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسٹریمنگ۔ آر ایس: 2980 - `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/ٹی ایکس آر آر ایس: 4424 - `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/اروہہ_کور/ٹیسٹ/ایگزیکیوٹر_مگریشن_ٹروسپیکٹ۔ آر ایس: 25 - `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/پن_ریگسٹری۔ آر ایس: 1158 - `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/اسنیپ شاٹ ڈاٹ آر ایس: 29 - `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/سمرگی_ڈوک_سینک.رس: 57 - `Path::new(env!("CARGO_MANIFEST_DIR"))`- ٹیسٹ: کریٹس/آئروہ_کرپٹو/ٹیسٹ/خفیہ_کیسیٹ_ویکٹرز۔ آر ایس: 57 - `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_کرپٹو/ٹیسٹ/sm2_fixure_vectors.rs: 50 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_کرپٹو/ٹیسٹ/sm_cli_matrix.rs: 19 - `env!("CARGO_MANIFEST_DIR"),`
- build: crates/iroha_data_model/build.rs:11 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");`
- پروڈ: کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/لیب۔ آر ایس: 186 - `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- پروڈ: کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/لیب۔ آر ایس: 190 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/آف لائن/پوسیڈن۔ آر ایس: 449 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/سورانیٹ/وی پی این آر ایس: 1217 - `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ٹیسٹ/اکاؤنٹ_اڈریس_ ویکٹرز۔ آر ایس: 147 - `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ٹیسٹ/ایڈریس_کوروی_ریگسٹری۔ آر ایس: 33 - `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/خفیہ_نکرپٹڈ_پائی لوڈ_ ویکٹرز۔ آر ایس: 48 - `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/خفیہ_ والیٹ_فکسچرز۔ آر ایس: 15 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اتفاق رائے_ راؤنڈ ٹریپ۔ آر ایس: 1308 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ٹیسٹ/آف لائن_فکسچرز۔ آر ایس: 130 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 25 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 29 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 33 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 37 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 41 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ریسنس_فکسچرز۔ آر ایس: 46 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 50 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 54 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 58 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 62 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/اوریکل_ ریفرنس_فکسچرز۔ آر ایس: 200 - `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/رن ٹائم_ڈوک_سین سی آر ایس: 8 - `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_جینیسیس/ایس آر سی/لیب۔ آر ایس: 1173 - `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- ٹیسٹ: کریٹس/آئروہ_جینیسیس/ایس آر سی/لیب۔ آر ایس: 3847 - `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- ٹیسٹ: کریٹس/آئروہ_جینیسیس/ایس آر سی/لیب۔ آر ایس: 4228 - `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- ٹیسٹ: کریٹس/آئروہ_جینیسیس/ایس آر سی/لیب۔ آر ایس: 4243 - `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- ٹیسٹ: کریٹس/آئروہ_آئ 18 این/ایس آر سی/لیب۔ آر ایس: 495 - `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- ٹیسٹ: کریٹس/آئروہ_ جے ایس_ہوسٹ/ایس آر سی/لیب۔ آر ایس: 6943 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- پروڈ: کریٹس/آئروہ_کاگامی/نمونے/کوڈیک/جنریٹ۔ آر ایس: 13 - `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- پروڈ: کریٹس/اروہہ_کاگامی/نمونے/کوڈیک/ایس آر سی/مین۔ آر ایس: 35 - `let dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/آئروہ_کاگامی/ایس آر سی/کوڈیک.رس: 393 - `env!("CARGO_MANIFEST_DIR"),`
- پروڈ: کریٹس/آئروہ_کاگامی/ایس آر سی/لوکل نیٹ۔ آر ایس: 604 - `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_کاگامی/ٹیسٹ/کوڈیک۔ آر ایس: 11 - `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- ٹیسٹ: کریٹس/اروہہ_ٹیلمیٹری/ٹیسٹ/ڈرل_لاگ۔ آر ایس: 10 - `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/آئروہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/کنفگ۔ آر ایس: 724 - `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- ٹیسٹ: کریٹس/آئروہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/ایف ایس لاک_پورٹس۔ آر ایس: 23 - `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- ٹیسٹ: کریٹس/آئروہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/ایف ایس لاک_پورٹس۔ آر ایس: 25 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 281 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_سمپلس/ایس آر سی/لیب۔ آر ایس: 204 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_سمپلس/ایس آر سی/لیب۔ آر ایس: 241 - `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ایس آر سی/ڈی اے/ٹیسٹ۔ آر ایس: 3490 - `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ایس آر سی/سرفس/api.rs: 4053 - `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/اکاؤنٹ_اڈریس_ ویکٹرز۔ آر ایس: 140 - `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/اکاؤنٹس_ پورٹفولیو۔ آر ایس: 91 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/sorafs_discovery.rs: 1056 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- بلڈ: کریٹس/ivm/build.rs: 21 - `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- پروڈ: کریٹس/ivm/src/bin/gen_abi_hash_doc.rs: 28 - `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- پروڈ: کریٹس/IVM/SRC/BIN/GEN_HEADER_DOC.RS: 48 - `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- پروڈ: کریٹس/ivm/src/bin/gen_pointer_types_doc.rs: 23 - `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- پروڈ: کریٹس/ivm/src/bin/gen_syscalls_doc.rs: 24 - `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- پروڈ: کریٹس/IVM/SRC/BIN/IVM_PREBUILD.RS: 16 - `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- پروڈ: کریٹس/ivm/src/bin/ivm_predecoder_export.rs: 23 - `let _crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- پروڈ: کریٹس/ivm/src/presecoder_fixtures.rs: 236 - `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/CLI_SMOKE.RS: 9 - `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/CLI_SMOKE.RS: 57 - `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/CLI_SMOKE.RS: 89 - `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/DOCS_CONSISTYCY.RS: 3 - `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/IVM_ABI_DOC_SYNC.RS: 8 - `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/IVM_HEADER_DOC_SYNC.RS: 8 - `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/نوریٹو_ پورٹل_سنیپیٹس_کومپائل۔ آر ایس: 19 - `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/پوائنٹر_ٹائپس_ڈوک_جینریٹیڈ۔ آر ایس: 7 - `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/پوائنٹر_ٹائپس_ڈوک_جینریٹڈ_ آئی وی ایم_ ایم ڈی آر آر ایس: 8 - `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/syscalls_doc_generated.rs: 7 - `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/syscalls_doc_sync.rs: 8 - `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/syscalls_gas_names.rs: 11 - `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- بینچ: کریٹس/نوریٹو/بینچ/پیریٹی_کومپیر۔ آر ایس: 79 - `let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- بلڈ: کریٹس/نوریٹو/بلڈ آر آر ایس: 17 - `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- پروڈ: کریٹس/نوریٹو/ایس آر سی/بن/نوریٹو_ریجین_گولڈینس۔ آر ایس: 9 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/نوریٹو/ٹیسٹ/AOS_NCB_MORE_GOLDEN.RS: 200 - `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- ٹیسٹ: کریٹس/نوریٹو/ٹیسٹ/JSON_GOLDEN_LOADER.RS: 14 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/نوریٹو/ٹیسٹ/NCB_ENUM_ITER_SAMPLES.RS: 353 - `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/نوریٹو/ٹیسٹ/NCB_ENUM_ITER_SAMPLES.RS: 387 - `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/نوریٹو/ٹیسٹ/NCB_ENUM_ITER_SAMPLES.RS: 554 - `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- ٹیسٹ: کریٹس/نوریٹو/ٹیسٹ/NCB_ENUM_ITER_SAMPLES.RS: 665 - `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- ٹیسٹ: کریٹس/نوریٹو/ٹیسٹ/NCB_ENUM_LARGE_FIXTURE.RS: 37 - `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- ٹیسٹ: کریٹس/sorafs_car/src/bin/da_reconstruct.rs: 434 - `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_car/src/bin/da_reconstruct.rs: 710 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- پروڈ: کریٹس/sorafs_car/src/bin/soranet_trustless_verifier.rs: 140 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- ٹیسٹ: کریٹس/sorafs_car/ٹیسٹ/صلاحیت_سیمولیشن_ٹولکیٹ.Rs: 9 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_car/tests/fetch_cli.rs: 50 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_car/tests/fetch_cli.rs: 1043 - `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_car/tests/fetch_cli.rs: 1159 - `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_car/tests/taikai_car_cli.rs: 227 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_car/tests/taikai_viewer_cli.rs: 22 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_car/tests/testless_verifier.rs: 8 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- پروڈ: کریٹس/sorafs_chunker/src/bin/برآمد_ ویکٹر. rs: 175 - `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/sorafs_chunker/tests/backpressure.rs: 8 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_chunker/tests/vectors.rs: 12 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_manifest/tests/por_fixtures.rs: 11 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: کریٹس/sorafs_manifest/tests/provider_admission_fixtures.rs: 12 - `cmd.current_dir(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_manifest/tests/replication_order_fixtures.rs: 8 - `env!("CARGO_MANIFEST_DIR"),`
- پروڈ: کریٹس/sorafs_node/src/bin/sorafs_gateway.rs: 55 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- پروڈ: کریٹس/sorafs_node/src/bin/sorafs_gateway.rs: 59 - `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0")`
- ٹیسٹ: کریٹس/sorafs_node/src/گیٹ وے۔ آر ایس: 2006 - `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");`
- ٹیسٹ: کریٹس/sorafs_node/tests/cli.rs: 122 - `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_node/ٹیسٹ/گیٹ وے۔ آر ایس: 14 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_node/ٹیسٹ/گیٹ وے۔ آر ایس: 30 - `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/sorafs_orchestrator/src/lib.rs: 6316 - `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/sorafs_orchestrator/src/lib.rs: 6449 - `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/sorafs_orchestrator/src/lib.rs: 8100 - `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: کریٹس/sorafs_orchestrator/tests/orchestrator_parity.rs: 180 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: کریٹس/سورانیٹ_پق/ٹیسٹ/KAT_VECTORS.RS: 9 - `env!("CARGO_MANIFEST_DIR"),`
- بلڈ: انضمام_ٹیسٹس/بلڈ آر آر ایس: 17 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹس/ایس آر سی/sorafs_gateway_capability_refusal.rs: 157 - `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- ٹیسٹ: انٹیگریشن_ٹیسٹس/ایس آر سی/sorafs_gateway_conformance.rs: 1032 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/ایڈریس_کینیکلائزیشن۔ آر ایس: 126 - `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اثاثہ۔ آر ایس: 49 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/فاسٹ_ڈس ایل_بولڈ. آر ایس: 7 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/جینیسس_جسن. آر ایس: 16 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/جینیسس_جسن. آر ایس: 22 - `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اروہہ_کلئ۔ آر ایس: 24 ​​- `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/IVM_HEADER_DECODE.RS: 49 - `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/IVM_HEADER_SMOKE.RS: 24 - `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 70 - `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 123 - `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 173 - `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/گٹھ جوڑ/CBDC_ORLOUT_BUNDLE.RS: 9 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/گٹھ جوڑ/سی بی ڈی سی_ وائٹ لسٹ۔ آر ایس: 26 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/گٹھ جوڑ/گلوبل_کممیٹ۔ آر ایس: 17 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/گٹھ جوڑ/لین_ریگسٹری۔ آر ایس: 12 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/نوریٹو_برن_فکسچر۔ آر ایس: 19 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/ریپو۔ آر ایس: 31 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اسٹریمنگ/Mod.rs: 339 - `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 220- `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 538- `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: موچی/موچی کور/src/torii.rs: 4740- `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: موچی/موچی کور/src/torii.rs: 4755- `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: موچی/موچی کور/src/torii.rs: 4770- `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: موچی/موچی کور/src/torii.rs: 4785- `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹیسٹ: موچی/موچی انٹیگریشن/ٹیسٹ/سپروائزر۔ آر ایس: 168- `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
-ٹیسٹ: ٹولز/سورانیٹ ہینڈ شیک ہارنس/ٹیسٹ/فکسچر_ورائیفائ۔ آر ایس: 6-`let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
-ٹیسٹ: ٹولز/سورانیٹ ہینڈ شیک ہارنس/ٹیسٹ/انٹرپ_پیریٹی۔ آر ایس: 77-`let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
-ٹیسٹ: ٹولز/سورانیٹ ہینڈ شیک ہارنس/ٹیسٹ/پرف_گیٹ۔ آر ایس: 173-`let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ٹول: XTASK/SRC/BIN/control_plane_mock.rs: 362 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹول: XTASK/SRC/MAIN.RS: 11383 - `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹول: XTASK/SRC/SORAFS/GATTWAY_FIXTURE.RS: 28 - `env!("CARGO_MANIFEST_DIR"),`
- ٹول: XTASK/SRC/SORAFS/GATTWAY_FIXTURE.RS: 32 - `env!("CARGO_MANIFEST_DIR"),`
- ٹیسٹ: XTASK/ٹیسٹ/ایڈریس_ ویکٹرز۔ آر ایس: 7 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/android_dashboard_parity_cli.rs: 7 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/CODEC_RANS_TABLES.RS: 18 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/DA_PROOF_BENCH.RS: 8 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/iso_bridge_lint.rs: 7 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/وزارت_جینڈا. آر ایس: 7 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/SNS_CATALOG_VERIFY.RS: 5 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soradns_cli.rs: 11 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/sorafs_fetch_fixure.rs: 9 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_bug_bounty.rs: 11 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_gateway_billing.rs: 12 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_gateway_billing_m0.rs: 30 - `let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_gateway_m1.rs: 11 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_gateway_m2.rs: 25 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_pop_template.rs: 10 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_pop_template.rs: 77 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_pop_template.rs: 131 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_pop_template.rs: 202 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_pop_template.rs: 306 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_pop_template.rs: 356 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/soranet_pop_template.rs: 489 - `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/اسٹریمنگ_بنڈل_چیک. آر ایس: 9 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ٹیسٹ: XTASK/ٹیسٹ/اسٹریمنگ_ینٹروپی_بینچ۔ آر ایس: 8 - `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## cargo_pkg_version (prod: 11 ، ٹیسٹ: 1 ، ٹول: 1)- پروڈ: کریٹس/اروہ/ایس آر سی/کلائنٹ۔ آر ایس: 478 - `map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));`
- پروڈ: کریٹس/آئروہ_ سی ایل آئی/ایس آر سی/کمانڈز/sorafs.rs: 1767 - `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- پروڈ: کریٹس/اروہ_ سی ایل آئی/ایس آر سی/مین_شریڈ۔ آر ایس: 72 - `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- پروڈ: کریٹس/آئروہ_ سی ایل آئی/ایس آر سی/مین_شریڈ
- ٹیسٹ: کریٹس/اروہ_ سی ایل آئی/ٹیسٹ/CLI_SMOKE.RS: 359 - `let expected_version = env!("CARGO_PKG_VERSION");`
- پروڈ: کریٹس/آئروہ_کور/ایس آر سی/سومرگی/آر بی سی_ اسٹور. آر ایس: 40 - `version: env!("CARGO_PKG_VERSION").to_owned(),`
- پروڈ: کریٹس/آئروہ_ جے ایس_ہوسٹ/ایس آر سی/لیب۔ آر ایس: 2998 - `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- پروڈ: کریٹس/آئروہ_ٹیلمیٹری/ایس آر سی/ڈبلیو ایس آر ایس: 243 - `env!("CARGO_PKG_VERSION")`
- پروڈ: کریٹس/آئروہاد/ایس آر سی/مین۔ آر ایس: 502 - `version = env!("CARGO_PKG_VERSION"),`
- پروڈ: کریٹس/آئروہاد/ایس آر سی/مین۔ آر ایس: 3986 - `version = env!("CARGO_PKG_VERSION"),`
- پروڈ: کریٹس/sorafs_car/src/bin/sorafs_fetch.rs: 1111 - `Value::from(env!("CARGO_PKG_VERSION")),`
- پروڈ: کریٹس/sorafs_orchestrator/src/bin/sorafs_cli.rs: 93 - `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
-ٹول: ٹولز/ٹیلی میٹری سکیما-ڈف/ایس آر سی/مین۔ آر ایس: 248-`tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## cargo_primary_package (تعمیر: 1)

- بلڈ: کریٹس/سورانیٹ_پی کیو/بلڈ آر آر ایس: 6 - `if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {`

## cargo_target_dir (prod: 3 ، ٹیسٹ: 2 ، ٹول: 1)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 524 - `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 759 - `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 575- `let target_root = env::var_os("CARGO_TARGET_DIR")`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 629- `let target_root = env::var_os("CARGO_TARGET_DIR")`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 683- `let target_root = env::var_os("CARGO_TARGET_DIR")`
- ٹول: XTASK/SRC/MOCHI.RS: 383 - `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## cargo_workspace_dir (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسٹیٹ۔ آر ایس: 12015 - `if let Some(workspace_dir) = option_env!("CARGO_WORKSPACE_DIR") {`

## crypto_sm_intrinsics (بینچ: 1)

- بینچ: کریٹس/آئروہ_کرپٹو/بینچز/ایس ایم_پیرف۔ آر ایس: 183 - `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`

## cuda_home (تعمیر: 2)

- بلڈ: کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 198 - `env::var_os("CUDA_HOME")`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:63 — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (تعمیر: 2)

- بلڈ: کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 199 - `.or_else(|| env::var_os("CUDA_PATH"))`
- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:64 — `.or_else(|| env::var_os("CUDA_PATH"))`

## ڈیٹاسپیس_اڈورسیریل_آرٹیکٹ_ڈیر (ٹیسٹ: 1)

- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/گٹھ جوڑ/کراس_لین۔ آر ایس: 686 - `if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {`

## Docs_rs (تعمیر: 1)

- بلڈ: کریٹ/نوریٹو/بلڈ آر آر ایس: 8 - `if env::var_os("DOCS_RS").is_some() {`

## enum_bench_n (بینچ: 1)

- بینچ: کریٹس/نوریٹو/بینچ/enum_packed_bench.rs: 75 - `let n: usize = std::env::var("ENUM_BENCH_N")`

## فاسٹ پی کیو_ڈی بوگ_فوزڈ (پروڈ: 1)

- پروڈ: کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/ٹریس۔ آر ایس: 98 - `Some(*DEBUG_FUSED_ENV.get_or_init(|| env::var_os("FASTPQ_DEBUG_FUSED").is_some()))`

## fastpq_expected_kib (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/پرف_ پروڈکشن۔ آر ایس: 88 - `env::var("FASTPQ_EXPECTED_KIB")`

## fastpq_expected_ms (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/پرف_ پروڈکشن۔ آر ایس: 80 - `env::var("FASTPQ_EXPECTED_MS")`

## فاسٹ پی کیو_پروف_رو (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/پرف_ پروڈکشن۔ آر ایس: 72 - `env::var("FASTPQ_PROOF_ROWS")`

## fastpq_skip_gpu_build (تعمیر: 1)

- بلڈ: کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 45 - `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## fastpq_update_fixtures (ٹیسٹ: 6)- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/بیک اینڈ_ریگریشن۔ آر ایس: 47 - `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/بیک اینڈ_ریگریشن۔ آر ایس: 69 - `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/پروف_فکسچر۔ آر ایس: 43 - `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/ٹریس_کومیٹمنٹ۔ آر ایس: 23 - `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/ٹریس_کومیٹمنٹ۔ آر ایس: 111 - `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- ٹیسٹ: کریٹس/فاسٹ پی کیو_پروور/ٹیسٹ/ٹرانسکرپٹ_ری پلے۔ آر ایس: 67 - `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`

## جنیسیس_ڈیبگ_موڈ (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_ٹیسٹ_ نیٹ ورک/مثالوں/جینیسس_ڈی بوگ۔ آر ایس: 15 - `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## جنیسیس_ڈیبگ_پے لوڈ (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_ٹیسٹ_ نیٹ ورک/مثالوں/جینیسس_ڈی بوگ۔ آر ایس: 122 - `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## github_step_summary (prod: 2)

- پروڈ: کریٹس/آئروہ_کرپٹو/ایس آر سی/بن/گوسٹ_پیرف_چیک۔ آر ایس: 22 - `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- پروڈ: کریٹس/آئروہ_کرپٹو/ایس آر سی/بن/ایس ایم_پیرف_چیک۔ آر ایس: 205 - `summary_target: env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from),`

## git_commit_hash (prod: 1)

- پروڈ: کریٹس/آئروہ_کور/ایس آر سی/سومرگی/آر بی سی_ اسٹور. آر ایس: 42 - `git_commit: option_env!("GIT_COMMIT_HASH").map(str::to_owned),`

## ہوم (پروڈ: 1)

- پروڈ: کریٹس/اروہ/ایس آر سی/کنفیگ۔ آر ایس: 57 - `env::var_os("HOME").map(PathBuf::from)`

## Iroha_allow_net (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/ایزانامی/ایس آر سی/افراتفری۔ آر ایس: 374 - `.or_else(|_| std::env::var("IROHA_ALLOW_NET"))`

## iroha_conf_gas_seed (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_سمپلس/ایس آر سی/لیب۔ آر ایس: 57 - `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## Iroha_da_spool_dir (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسٹیٹ۔ آر ایس: 9096 - `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## iroha_metrics_panic_on_duplicate (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/آئروہ_ٹیلمیٹری/ایس آر سی/میٹرکس۔ آر ایس: 11397 - `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/میٹرکس_ریگسٹری۔ آر ایس: 33 - `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE").unwrap_or_else(|_| "0".to_string());`

## iroha_run_ignored (ٹیسٹ: 71)- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_AUTO_CLOSE_APPROVE.RS: 20 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_FINALIZE_REAL_VK.RS: 7 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_MIN_DURATION.RS: 18 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_MODE_MISMATCH.RS: 20 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_MODE_MISMATCH_ZK.RS: 29 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_PLAIN_BALLOT.RS: 20 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_PLAIN_CONVICTION.RS: 20 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/گورنمنٹ_پلین_ڈیسبلڈ. آر ایس: 17 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_PLAIN_MISSING_REF.RS: 15 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_PLAIN_REVOTE_MONOTONIC.RS: 18 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_PROTECTECT_GATE.RS: 64 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_REFERENDUM_OPEN_CLOSE.RS: 24 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/گورنمنٹ_تھلسولڈس آر ایس: 20 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/گورنمنٹ_تھلسولڈس آر ایس: 82 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/گورنمنٹ_تھرہولڈس_پوسٹیٹو. آر ایس: 20 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_UNLOCK_SWEEP.RS: 16 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_ZK_Ballot_lock_verified.rs: 8 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_ZK_BALLOT_REAL_VK.RS: 8 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/GOV_ZK_REFERENDUM_WINDOW_GUARD.RS: 16 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/زیڈ کے_روٹس_جیٹ_کیپ۔ آر ایس: 29 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/ZK_VOTE_GET_TALY.RS: 29 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کرپٹو/ایس آر سی/مرکل.رس: 1317 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کرپٹو/ایس آر سی/مرکل.رس: 1332 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کرپٹو/ٹیسٹ/مرکل_نوریٹو_ راؤنڈ ٹریپ۔ آر ایس: 18 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_کرپٹو/ٹیسٹ/مرکل_نوریٹو_ راؤنڈ ٹریپ۔ آر ایس: 42 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ایس آر سی/بلاک/ہیڈر۔ آر ایس: 624 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ایس آر سی/آئی ایس آئی/موڈ۔ آر ایس: 1888 - `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/آئی ایس آئی/رجسٹر۔ آر ایس: 324 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/پروف۔ آر ایس: 798 - `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/ٹرانزیکشن/سائن انڈ۔ آر ایس: 1048 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/انسٹرکشن_ریگسٹری_لازی_نیٹ۔ آر ایس: 8 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/انسٹرکشن_ریگسٹری_ریسیٹ۔ آر ایس: 7 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/ماڈل_ڈرائیو_ری پرو. آر ایس: 15 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/ماڈل_ڈرائیو_ری پرو. آر ایس: 37 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/ماڈل_ڈرائیو_ری پرو. آر ایس: 58 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/ماڈل_ڈرائیو_ری پرو. آر ایس: 84 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/رجسٹری_ڈیکوڈ_ راؤنڈ ٹریپ۔ آر ایس: 9 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ٹیسٹ/ٹریٹ_بیکجیکٹس۔ آر ایس: 7 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈل/ٹیسٹ/ٹریٹ_بیکجیکٹس۔ آر ایس: 27 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ٹیسٹ/زیڈ کے_نوولوپ_ راؤنڈ ٹریپ۔ آر ایس: 6 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/معاہدے_ایکٹیویٹ_ٹیگریشن۔ آر ایس: 23 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/معاہدے_ایکٹیویٹ_ٹریگریشن۔ آر ایس: 174 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/معاہدے_کال_ٹینگریشن۔ آر ایس: 21 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/معاہدے_ڈی تعیناتی_ٹیگریشن۔ آر ایس: 23 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/معاہدے_آنسٹینس_ایکٹیویٹ_ٹریگریشن۔ آر ایس: 19 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/معاہدے_آنسٹینس_لسٹ_روٹر۔ آر ایس: 17 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/گورنمنٹ_کونسل_پرسٹ_ٹیگریشن۔ آر ایس: 22 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/گورنمنٹ_کونسل_VRF.RS: 18 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/GOV_ENACT_HANDLER.RS: 12 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/گورنمنٹ_آنسٹینس_لسٹ۔ آر ایس: 14 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/اروہہ_ٹوری/ٹیسٹ/گورنمنٹ_موڈ_مزمچچ_ند_اوٹوکلوز۔ آر ایس: 42 - `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/GOV_PROTECTECT_ENDPOINTS.RS: 15 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/گورنمنٹ_پریٹیکٹڈ_ینڈپوائنٹس_روٹر۔ آر ایس: 20 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/اروہہ_ٹوری/ٹیسٹ/گورنمنٹ_ریڈ_ینڈ پوائنٹس_روٹر۔ آر ایس: 21 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/BEEP_TEST.RS: 7 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/کوٹوڈاما_اسٹرکٹ_فیلڈس آر ایس: 11 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/ZK_ROOTS_AND_VOTE_SYSCALL.RS: 16 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/ZK_ROOTS_AND_VOTE_SYSCALL.RS: 50 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/واقعات/نوٹیفکیشن۔ آر ایس: 81 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اضافی_فونکشنل/غیر مستحکم_نیٹ ورک۔ آر ایس: 753 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اضافی_فونکشنل/غیر مستحکم_نیٹ ورک۔ آر ایس: 769 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اضافی_فونکشنل/غیر مستحکم_نیٹ ورک۔ آر ایس: 785 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اضافی_فونکشنل/غیر مستحکم_نیٹ ورک۔ آر ایس: 801 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اضافی_فونکشنل/غیر مستحکم_نیٹ ورک۔ آر ایس: 817 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اجازتیں۔ آر ایس: 202 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اجازتیں۔ آر ایس: 265 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/اجازتیں۔ آر ایس: 328 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/پائپ لائن_ بلاک_ریجیکٹڈ۔ آر ایس: 17 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/چھانٹ رہا ہے۔ آر ایس: 29 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/ٹرگرز/بذریعہ_کال_ٹریگر۔ آر ایس: 139 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/ٹرگرز/ٹائم_ٹریگر۔ آر ایس: 149 - `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## Iroha_run_zk_wrappers (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/IVM/ٹیسٹ/کوٹوڈاما_روپرس۔ آر ایس: 2 - `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`

## iroha_skip_bind_checks (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 3098 - `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## Iroha_sm_cli (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_کرپٹو/ٹیسٹ/sm_cli_matrix.rs: 47 - `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## iroha_test_dump_genesis (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 6661 - `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`## iroha_test_prebuild_default_executor (تعمیر: 1 ، ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/کنفیگ. آر ایس: 155 - `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- بلڈ: انٹیگریشن_ٹیسٹ/بلڈ۔ آر ایس: 205 - `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## iroha_test_skip_build (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 1244 - `std::env::var("IROHA_TEST_SKIP_BUILD")`

## iroha_test_target_dir (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 521 - `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`
- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 765 - `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`

## iroha_test_use_default_executor (ٹیسٹ: 3)

- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/ایگزیکیوٹر۔ آر ایس: 2383 - `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/ایگزیکیوٹر۔ آر ایس: 2520 - `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسٹیٹ۔ آر ایس: 12008 - `if std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR").is_some() {`

## iroha_torii_openapi_actual (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/روٹر_فیٹور_میٹرکس.رس: 94 - `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## iroha_torii_openapi_expected (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/روٹر_فیٹور_میٹرکس۔ آر ایس: 88 - `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/روٹر_فیٹور_میٹرکس.رس: 104 - `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## iroha_torii_openapi_tokens (ٹول: 2)

- ٹول: XTASK/SRC/MAIN.RS: 11025 - `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`
- ٹول: XTASK/SRC/MAIN.RS: 11077 - `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IVM_BIN (ٹیسٹ: 2)

- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 60 - `let ivm_bin = env::var("IVM_BIN")`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 163 - `let ivm_bin = env::var("IVM_BIN")`

## ivm_compiler_debug (prod: 1)

- پروڈ: کریٹس/کوٹوڈاما_لانگ/ایس آر سی/مرتب۔ آر ایس: 4094 - `let compiler_debug = if std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## ivm_cuda_gencode (تعمیر: 1)

- بلڈ: کریٹس/ivm/build.rs: 35 - `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| "arch=compute_61,code=sm_61".to_string());`

## ivm_cuda_nvcc (تعمیر: 1)

- بلڈ: کریٹس/ivm/build.rs: 30 - `let nvcc = env::var("IVM_CUDA_NVCC")`

## ivm_cuda_nvcc_extra (تعمیر: 1)

- بلڈ: کریٹس/ivm/build.rs: 36 - `let extra_flags: Vec<String> = env::var("IVM_CUDA_NVCC_EXTRA")`

## ivm_debug_ir (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/IVM/ٹیسٹ/ڈیبگ_کونٹین ڈاٹ آر ایس: 15 - `if std::env::var_os("IVM_DEBUG_IR").is_some() {`

## ivm_debug_metal_enum (ڈیبگ: 1)

- ڈیبگ: کریٹس/ivm/src/vector.rs: 474 - `std::env::var("IVM_DEBUG_METAL_ENUM")`

## ivm_debug_metal_selltest (ڈیبگ: 1)

- ڈیبگ: کریٹس/ivm/src/vector.rs: 1205 - `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## ivm_disable_cuda (ڈیبگ: 1)

- ڈیبگ: کریٹس/IVM/SRC/CUDA.RS: 315 - `&& std::env::var("IVM_DISABLE_CUDA")`

## ivm_disable_metal (ڈیبگ: 1)

- ڈیبگ: کریٹس/IVM/SRC/Vector.rs: 299 - `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELLTEST_FAIL (ڈیبگ: 1)

- ڈیبگ: کریٹس/IVM/SRC/CUDA.RS: 326 - `&& std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL")`

## IVM_FORCE_METAL_ENUM (ڈیبگ: 1)

- ڈیبگ: کریٹس/ivm/src/vector.rs: 444 - `std::env::var("IVM_FORCE_METAL_ENUM")`

## IVM_FORCE_METAL_SELLTEST_FAIL (ڈیبگ: 1)

- ڈیبگ: کریٹس/ivm/src/vector.rs: 1192 - `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## ivm_tool_bin (ٹیسٹ: 1)

- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 113 - `let ivm_tool = env::var("IVM_TOOL_BIN")`

## izanami_allow_net (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/ایزانامی/ایس آر سی/افراتفری۔ آر ایس: 373 - `std::env::var("IZANAMI_ALLOW_NET")`

## izanami_tui_allow_zero_seed (prod: 1)

- پروڈ: کریٹس/ایزانامی/ایس آر سی/ٹی یو آئی آر آر ایس: 134 - `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## jsonstage1_cuda_arch (تعمیر: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:40 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## jsonstage1_cuda_skip_build (تعمیر: 1)

- build: crates/norito/accelerators/jsonstage1_cuda/build.rs:18 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## koto_bin (ٹیسٹ: 2)- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 50 - `let koto_bin = env::var("KOTO_BIN")`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 154 - `let koto_bin = env::var("KOTO_BIN")`

## لینگ (ٹیسٹ: 3)

- ٹیسٹ: کریٹس/IVM/SRC/BIN/KOTO_LINT.RS: 702 - `let previous = env::var("LANG").ok();`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/i18n.rs: 11 - `let old_lang = env::var("LANG").ok();`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/i18n.rs: 69 - `let old_lang = env::var("LANG").ok();`

## lc_all (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/IVM/ٹیسٹ/i18n.rs: 12 - `let old_lc_all = env::var("LC_ALL").ok();`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/i18n.rs: 70 - `let old_lc_all = env::var("LC_ALL").ok();`

## lc_messages (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/IVM/ٹیسٹ/i18n.rs: 13 - `let old_lc_messages = env::var("LC_MESSAGES").ok();`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/i18n.rs: 71 - `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## میکس_ ڈگری (پروڈ: 1)

- پروڈ: کریٹس/آئروہ_کور/ایس آر سی/زیڈ کے آر ایس: 106 - `let current = std::env::var("MAX_DEGREE")`

## موچی_کونفگ (پروڈ: 1)

-پروڈ: موچی/موچی-یو-ای-ای جی یو آئی/ایس آر سی/کنفیگ۔ آر ایس: 325-`if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## موچی_ڈیٹا_روٹ (پروڈ: 1)

- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 2068- `std::env::var_os("MOCHI_DATA_ROOT")`

## mochi_test_use_internal_genesis (prod: 1)

- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 1994- `if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {`

## نوریٹو_بنچ_سمری (بینچ: 1)

- بینچ: کریٹس/نوریٹو/بینچ/پیریٹی_کومپیر۔ آر ایس: 70 - `if std::env::var("NORITO_BENCH_SUMMARY").ok().as_deref() == Some("1") {`

## نوریٹو_ سی پی یو_ انفو (ٹول: 1)

- ٹول: XTASK/SRC/STEG1_BENCH.RS: 69 - `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## نوریٹو_ سی آر سی 64_ جی پی یو_لیب (پروڈ: 1)

- پروڈ: کریٹس/نوریٹو/ایس آر سی/کور/سم ڈی_ سی آر سی 64. آر ایس: 215 - `std::env::var("NORITO_CRC64_GPU_LIB").ok(),`

## نوریٹو_ڈیس ایبل_پیکڈ_سٹرکٹ (پروڈ: 1 ، ٹیسٹ: 1)

- پروڈ: کریٹس/آئروہ_ جے ایس_ہوسٹ/ایس آر سی/لیب۔ آر ایس: 183 - `let env_set = std::env::var_os("NORITO_DISABLE_PACKED_STRUCT").is_some();`
- ٹیسٹ: کریٹس/نوریٹو/ایس آر سی/لیب۔ آر ایس: 322 - `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## نوریٹو_ جی پی یو_ سی آر سی 64_مین_بائٹس (پروڈ: 1)

- پروڈ: کریٹس/نوریٹو/ایس آر سی/کور/سم ڈی_ سی آر سی 64. آر ایس: 83 - `std::env::var("NORITO_GPU_CRC64_MIN_BYTES").ok(),`

## نوریٹو_پار_سٹیج 1_مین (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/نوریٹو/ایس آر سی/لیب۔ آر ایس: 4847 - `std::env::var("NORITO_PAR_STAGE1_MIN")`

## نوریٹو_سکیپ_بائنڈنگز_سنک (تعمیر: 1)

- بلڈ: کریٹ/نوریٹو/بلڈ آر آر ایس: 12 - `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## نوریٹو_سٹیج 1_ جی پی یو_مین_بائٹس (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/نوریٹو/ایس آر سی/لیب۔ آر ایس: 4881 - `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## نوریٹو_ ٹریس (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/نوریٹو/ایس آر سی/لیب۔ آر ایس: 123 - `std::env::var_os("NORITO_TRACE").is_some()`
- ٹیسٹ: کریٹس/نوریٹو/ایس آر سی/لیب۔ آر ایس: 138 - `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## no_proxy (پروڈ: 1)

- پروڈ: کریٹس/آئروہ_پی 2 پی/ایس آر سی/ٹرانسپورٹ۔ آر ایس: 469 - `let no_proxy = env::var("NO_PROXY")`

## NVCC (تعمیر: 1)

- بلڈ: کریٹس/ivm/build.rs: 31 - `.or_else(|_| env::var("NVCC"))`

## آؤٹ_ڈیر (بلڈ: 4 ، پروڈ: 12 ، ٹیسٹ: 2)- بلڈ: کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 99 - `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
- build: crates/iroha_data_model/build.rs:12 — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));`
- prod: crates/iroha_data_model/src/lib.rs:179 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
- بلڈ: کریٹس/ivm/build.rs: 27 - `let out_dir = PathBuf::from(env::var("OUT_DIR")?);`
- بلڈ: کریٹس/ivm/build.rs: 120 - `if let Some(out_dir) = env::var_os("OUT_DIR") {`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 12 - `static PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 13 - `static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 14 - `static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 15 - `static SHA_LEAVES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 16 - `static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 17 - `static SHA3_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 18 - `static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 19 - `static BN254_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 20 - `static SIG_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/signature.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 21 - `static SHA_PAIRS_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx"));`
- پروڈ: کریٹس/IVM/SRC/CUDA.RS: 22 - `static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));`
- ٹیسٹ: کریٹس/IVM/SRC/PTX_TESTS.RS: 7 - `let out_dir = match std::env::var("OUT_DIR") {`
- ٹیسٹ: کریٹس/IVM/ٹیسٹ/ptx_kernels.rs: 5 - `let out_dir = env!("OUT_DIR");`

## p2p_turn (prod: 1)

- پروڈ: کریٹس/آئروہ_پی 2 پی/ایس آر سی/ٹرانسپورٹ۔ آر ایس: 296 - `let endpoint = std::env::var("P2P_TURN")`

## راستہ (پروڈ: 2 ، ٹیسٹ: 1)

- پروڈ: کریٹس/آئروہ_ سی ایل آئی/ایس آر سی/کمانڈز/sorafs.rs: 7276 - `if let Some(path_var) = env::var_os("PATH") {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/کوٹوڈاما_ایکسامپلس۔ آر ایس: 17 - `let path = env::var_os("PATH")?;`
- پروڈ: موچی/موچی کور/ایس آر سی/سپروائزر۔ آر ایس: 511- `let path_var = env::var_os("PATH")?;`

## پرنٹ_سورکلز_فکسچر (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_ڈیٹا_موڈیل/ایس آر سی/اوریکل/موڈ. آر ایس: 3249 - `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## پرنٹ_ٹوری_سپیک (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_ٹوری/ایس آر سی/اوپن پی آئی آر ایس: 4380 - `if std::env::var("PRINT_TORII_SPEC").is_ok() {`

## پروفائل (بلڈ: 1 ، پروڈ: 1 ، ٹیسٹ: 1)

- پروڈ: کریٹس/آئروہ_کور/ایس آر سی/سومرگی/آر بی سی_ اسٹور. آر ایس: 41 - `profile: option_env!("PROFILE").unwrap_or("unknown").to_owned(),`
- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 995 - `let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());`
- بلڈ: انٹیگریشن_ٹیسٹ/بلڈ۔ آر ایس: 180 - `let profile = match env::var("PROFILE").unwrap_or_default().as_str() {`

## ازگر 3 (ٹیسٹ: 2)

- ٹیسٹ: کریٹس/sorafs_car/tests/taikai_car_cli.rs: 235 - `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`
- ٹیسٹ: کریٹس/sorafs_car/tests/taikai_viewer_cli.rs: 30 - `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## پائیتھنپاتھ (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/اروہ_ سی ایل آئی/ٹیسٹ/CLI_SMOKE.RS: 5245 - `match env::var("PYTHONPATH") {`

## rbc_session_path (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/سومرگی/آر بی سی_ اسٹور. آر ایس: 823 - `let path = std::env::var("RBC_SESSION_PATH").expect("set RBC_SESSION_PATH");`

## repo_proof_digest_out (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسمارٹ کنٹریکٹ/آئی ایس آئی/ریپو آر آر ایس: 2017 - `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## repo_proof_snapshot_out (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_کور/ایس آر سی/اسمارٹ کنٹرکٹس/آئی ایس آئی/ریپو آر آر ایس: 2005 - `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## رسٹ_لاگ (پروڈ: 1 ، ٹیسٹ: 2)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 5414 - `let original = env::var("RUST_LOG").ok();`
- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 5431 - `let original = env::var("RUST_LOG").ok();`
- پروڈ: کریٹس/ایزانامی/ایس آر سی/کنفیگ۔ آر ایس: 269 - `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`

## sm_perf_cpu_label (prod: 2)

- پروڈ: کریٹس/آئروہ_کرپٹو/ایس آر سی/بن/ایس ایم_پیرف_چیک۔ آر ایس: 637 - `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`
- پروڈ: کریٹس/اروہہ_کرپٹو/ایس آر سی/بن/ایس ایم_پرف_چیک۔ آر ایس: 679 - `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## sorafs_node_skip_ingest_tests (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/sorafs_node/ٹیسٹ/cli.rs: 17 - `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## sorafs_torii_skip_ingest_tests (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_ٹوری/ٹیسٹ/sorafs_discovery.rs: 95 - `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`## sumeragi_adversarial_artifact_dir (ٹیسٹ: 1)

- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/سومرگی_اڈورسیریل۔ آر ایس: 1222 - `let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {`

## sumeragi_baseline_artifact_dir (prod: 1 ، ٹیسٹ: 1)

- پروڈ: کریٹس/بلڈ سپورٹ/ایس آر سی/ایس آر سی/بن/سمرگی_بیسلین_ریپورٹ۔ آر ایس: 40- `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/سومرگی_نپوس_پرفارمنس۔ آر ایس: 1396 - `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`

## sumeragi_da_artifact_dir (prod: 1 ، ٹیسٹ: 1)

- پروڈ: کریٹس/بلڈ سپورٹ/ایس آر سی/ایس آر سی/بن/سمرگی_ڈا_رپورٹ۔ آر ایس: 41- `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`
- ٹیسٹ: انٹیگریشن_ٹیسٹ/ٹیسٹ/سومرگی_ڈا. آر ایس: 1599 - `let Ok(dir) = std::env::var("SUMERAGI_DA_ARTIFACT_DIR") else {`

## سسٹمروٹ (پروڈ: 1)

- پروڈ: کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/بیک اینڈ آر آر ایس: 535 - `env::var_os("SystemRoot").map(PathBuf::from)`

## ہدف (ٹول: 2)

- ٹول: XTASK/SRC/poseidon_bench.rs: 87 - `target: std::env::var("TARGET")`
- ٹول: ایکس ٹی ایس اے ایس سی/ایس آر سی/اسٹیج 1_بینچ۔ آر ایس: 63 - `target: std::env::var("TARGET")`

## test_log_filter (prod: 1)

- پروڈ: کریٹس/اروہہ_لگر/ایس آر سی/لیب۔ آر ایس: 89 - `filter: std::env::var("TEST_LOG_FILTER")`

## test_log_level (prod: 1)

- پروڈ: کریٹس/اروہہ_لگر/ایس آر سی/لیب۔ آر ایس: 85 - `level: std::env::var("TEST_LOG_LEVEL")`

## test_network_cargo (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/اروہہ_ٹیسٹ_ نیٹ ورک/ایس آر سی/لیب۔ آر ایس: 865 - `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## torii_debug_sort (prod: 1)

- پروڈ: کریٹس/آئروہ_ٹوری/ایس آر سی/روٹنگ۔ آر ایس: 12530 - `std::env::var("TORII_DEBUG_SORT").ok().is_some(),`

## torii_mock_harness_metrics_path (ٹول: 1)

- ٹول: XTASK/SRC/BIN/TORII_MOCK_HARNESS.RS: 106 - `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## torii_mock_harness_repo_root (ٹول: 1)

- ٹول: XTASK/SRC/BIN/TORII_MOCK_HARNESS.RS: 109 - `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## torii_mock_harness_retry_total (ٹول: 1)

- ٹول: XTASK/SRC/BIN/TORII_MOCK_HARNESS.RS: 297 - `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## torii_mock_harness_runner (ٹول: 1)

- ٹول: XTASK/SRC/BIN/TORII_MOCK_HARNESS.RS: 112 - `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## torii_mock_harness_sdk (ٹول: 1)

- ٹول: XTASK/SRC/BIN/TORII_MOCK_HARNESS.RS: 104 - `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## torii_openapi_token (ٹول: 2)

- ٹول: XTASK/SRC/MAIN.RS: 11020 - `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`
- ٹول: XTASK/SRC/MAIN.RS: 11073 - `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## اپ ڈیٹ_فکسچر (ٹیسٹ: 1)

- ٹیسٹ: کریٹس/آئروہ_کور/ٹیسٹ/اسنیپ شاٹ ڈاٹ آر ایس: 42 - `let update = env::var("UPDATE_FIXTURES")`

## یوزر پروفائل (پروڈ: 1)

- پروڈ: کریٹس/اروہ/ایس آر سی/کنفیگ۔ آر ایس: 55 - `env::var_os("USERPROFILE").map(PathBuf::from)`

## vergen_cargo_features (prod: 1)

- پروڈ: کریٹس/آئروہاد/ایس آر سی/مین۔ آر ایس: 4850 - `const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {`

## vergen_cargo_target_triple (prod: 1)

- پروڈ: کریٹس/آئروہ_ٹیلمیٹری/ایس آر سی/ڈبلیو ایس آر ایس: 238 - `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");`

## vergen_git_sha (پروڈ: 4)

- پروڈ: کریٹس/آئروہ_ سی ایل آئی/ایس آر سی/مین_شریڈ۔ آر ایس: 57 - `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`
- پروڈ: کریٹس/آئروہ_ٹیلمیٹری/ایس آر سی/ڈبلیو ایس آر ایس: 237 - `let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");`
- پروڈ: کریٹس/آئروہ_ٹوری/ایس آر سی/روٹنگ۔ آر ایس: 32515 - `git_sha: option_env!("VERGEN_GIT_SHA")`
- پروڈ: کریٹس/آئروہاد/ایس آر سی/مین۔ آر ایس: 4845 - `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`

## تصدیق_ بیچ (بینچ: 1)

- بینچ: کریٹس/IVM/بینچ/بینچ_ووٹنگ۔ آر ایس: 225 - `let verify_batch = std::env::var("VERIFY_BATCH")`

## تصدیق_یور (بینچ: 1)

- بینچ: کریٹس/IVM/بینچ/بینچ_ووٹنگ۔ آر ایس: 213 - `let verify_every: u64 = std::env::var("VERIFY_EVERY")`

## ووٹرز (بینچ: 1)

- بینچ: کریٹس/IVM/بینچ/بینچ_ووٹنگ۔ آر ایس: 207 - `let voters: u64 = std::env::var("VOTERS")`

## no_proxy (پروڈ: 1)

- پروڈ: کریٹس/آئروہ_پی 2 پی/ایس آر سی/ٹرانسپورٹ