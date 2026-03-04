---
lang: ba
direction: ltr
source: docs/source/agents/env_var_inventory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0a23896374a10f0862aeec69c607ba3022d3b337ae7f6bb54a61b8f7424410
source_last_modified: "2026-01-21T19:17:13.235848+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Тирә-яҡ мөхитте үҙгәртеү инвентаризацияһы

_Һуңғы яңыртылған аша `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`.

Дөйөм һылтанмалар: **505** · Үҙенсәлекле үҙгәртеүселәр: **137**

## АКЦИЯҺЫ_ИД_ТОКЕН_СИМӘТ_ТОКЕН (прод: 1)

- prod: йәшниктәр/сорафтар_оркестратор/срк/бин/сорафтар_кли.р. 313 — `let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {`

## АКЦИЯЛАРЫ_ИД_ТОКЕН_СӘМӘТ_URL (прод: 1)

- prod: йәшниктәр/сорафтар_оркестратор/src/bin/sorafs_cli.s:310 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`

## BLOCK_DUMP_HEIGHTS (миҫал: 1)

- миҫал: йәшниктәр/ироха_ядро/миҫалдар/блок_думп.р. 62 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`

## BLOCK_DUMP_SSSET (миҫал: 1)

- миҫал: йәшниктәр/ироха_ядро/миҫалдар/блок_думп. rs:51 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`

## BLOCK_DUMP_VERBOSE (миҫал: 1)

- миҫал: йәшниктәр/ироха_ядро/миҫалдар/блок_думп. rs:50 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`

## КАРГО (прод: 3, тест: 2)

- тест: йәшниктәр/ироха_тест_сентовый/срк/либ.:1026 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- тест: йәшниктәр/сорафтар_манифест/тестар/провиратор_адмиссия_фикстуралар.11 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`
- prod: мочи/мочи-ядро/срк/суҡырыусы.р. 547 — `let cargo = env::var_os("CARGO")`.
- prod: мочи/мочи-ядро/срк/суҡырыусы.р.:602 — `let cargo = env::var_os("CARGO")` X
- prod: мочи/мочи-ядро/срк/суҡырыусы.р.:656 — `let cargo = env::var_os("CARGO")`

## CARGO_BIN_EXE_iroha (тест: 2)

- һынау: йәшниктәр/ироха_кли/тестар/cly_smoke.rs:40 — `env!("CARGO_BIN_EXE_iroha")`
- тест: йәшниктәр/ироха_кли/тестар/тайҡай_полиция.20 — `env!("CARGO_BIN_EXE_iroha")`

## CARGO_BIN_EXE_iroha_monitor (тест: 4)

- тест: йәшниктәр/ироха_монитор/тестар/беркетмәләр_рендер.р.:11 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- тест: йәшниктәр/ироха_монитор/тестар/http_limits.10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- тест: йәшниктәр/ироха_монитор/тестар/инвалид_кредицидтар.р:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- тест: йәшниктәр/ироха_монитор/тестар/төтөн.rs:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## CARGO_BIN_EXE_kagami (тест: 2)

- тест: йәшниктәр/ироха_кагами/тестар/дөйөм/мод.р.:22 — `let output = Command::new(env!("CARGO_BIN_EXE_kagami"))`
- һынау: йәшниктәр/ироха_кагами/тестар/pop_rembed.34 — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`

## CARGO_BIN_EXE_kagami_mock (тест: 1)

- тест: мочи/мочи-интеграция/тестар/башҡарыусы.р.:33 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");` X

## CARGO_BIN_EXE_koto_компиляция (тест: 3)

- тест: йәшниктәр/вм/тестар/cli_smoke.rs:8 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- тест: йәшниктәр/вм/тестар/cli_smoke.rs:56 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- тест: йәшниктәр/вм/тестар/cly_smoke.rs:88 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.

## CARGO_BIN_EXE_sorafs_chunk_dump (тест: 1)

- һынау: йәшниктәр/сорафтар_chunker/тестар/бер_гиб.:103 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## CARGO_BIN_EXE_sorafs_cli (тест: 1)

- тест: йәшниктәр/сорафтар_машина/тестар/сорафтар_cli.s:42 — `let path = env::var("CARGO_BIN_EXE_sorafs_cli")`

## CARGO_BIN_EXE_sorafs_fetch (тест: 1)

- тест: йәшниктәр/сорафтар_автор/src/bin/sorafs_fetch.s:2831 — `if let Ok(path) = env::var("CARGO_BIN_EXE_sorafs_fetch") {`

## КАРГО_БИН_ЕКСЕ_тайҡай_машина (тест: 1)

- тест: йәшниктәр/сорафтар_машина/тестар/сорафтар_cli.s:48 — `let path = env::var("CARGO_BIN_EXE_taikai_car")`

## CARGO_BIN_NAME (прод: 3)

- prod: йәшниктәр/ироха_кли/срк/маин_шар. rs:63 — `BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))`
- prod: йәшниктәр/ироха_кли/срк/маин_шар. rs:72 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`.
- prod: йәшниктәр/родад/срк/маин.3176 — `let build_line = BuildLine::from_bin_name(env!("CARGO_BIN_NAME"));` X.

## CARGO_BUILD_TARGET (ҡорал: 2)

- инструмент: xtask/src/poseidon_bench.rs:88 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`
- инструмент: xtask/src/stage1_bench.rs:64 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_TARGET_ARCH (прод: 2, ҡорал: 2)- prod: йәшниктәр/ироха_крипто/срк/бин/см_перф_чек.р. 632 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- prod: йәшниктәр/ироха_крипто/срк/бин/см_перф_чек.р. 668 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- инструмент: xtask/src/poseidon_bench.rs:89 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- инструмент: xtask/src/stage1_bench.rs:65 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")` .

## CARGO_CFG_TARGET_OS (төҙөү: 1, прод: 2, ҡорал: 2)

- төҙөү: йәшниктәр/Fastpq_prover/төҙөлөш.27 — `let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();` XX
- prod: йәшник/ироха_крипто/срк/бин/см_перф_чек.р. 633 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- prod: йәшниктәр/ироха_крипто/срк/бин/см_перф_чек.р. 669 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`.
- инструмент: xtask/src/poseidon_bech.rs:91 — `os: std::env::var("CARGO_CFG_TARGET_OS")` X.
- инструмент: xtask/src/stage1_bench.rs:67 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## CARGO_FEATURE_CUDA (төҙөү: 2)

- төҙөү: йәшниктәр/Fastpq_prover/build.rs:25 — `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
- төҙөү: йәшниктәр/вм/төҙөү:12 — `if env::var_os("CARGO_FEATURE_CUDA").is_some()`

## CARGO_FEATURE_CUDA_KERNEL (төҙөү: 1)

- төҙөү: йәшниктәр/норито/тиҙләткестәр/jsnastage1_cuda/build.s:12 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## CARGO_FEATURE_FASTPQ_GPU (төҙөү: 1)

- төҙөү: йәшниктәр/Fastpq_prover/build.rs:26 — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## CARGO_FEATURE_FFI_EXPORT (прод: 1)

- prod: йәшниктәр/төҙөү-ярҙам/src/lib.s:30 — `let ffi_export = std::env::var_os("CARGO_FEATURE_FFI_EXPORT").is_some();`

## CARGO_FEATURE_FFI_IMPORT (прод: 1)

- prod: йәшниктәр/төҙөү-ярҙам/src/lib.s:29 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`.

## CARGO_MANIFEST_DIR (эскал: 4, төҙөү: 5, миҫал: 1, прод: 27, тест: 164, ҡорал: 4)- prod: йәшниктәр/Fastpq_prover/src/poseidon_manifest.10 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/Fastpq_prover/тестар/пакетинг. rs:17 — `Path::new(env!("CARGO_MANIFEST_DIR"))`.
- тест: йәшниктәр/Fastpq_prover/тестар/посейдон_манифест_эҙмә-эҙлеклелек. 9 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");` X
- тест: йәшниктәр/Fastpq_prover/тестар/посейдон_манифест_эҙмә-эҙлеклелек.28 — `let cuda_path = concat!(env!("CARGO_MANIFEST_DIR"), "/cuda/fastpq_cuda.cu");`
- тест: йәшниктәр/Fastpq_prover/тестар/иҫбатлау_фикстура.р.:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/Fastpq_prover/тестар/эҙ_комитет.14 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- тест: йәшниктәр/Fastpq_prover/тестар/трансрипт_реплей.р.:56 — `Path::new(env!("CARGO_MANIFEST_DIR"))` .
- тест: йәшниктәр/ироха/срк/клиент. 8409 — `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха/срк/см.р.:201 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- һынау: йәшниктәр/ироха/тестар/sm_signing.35 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_кли/срк/компью. 518 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- prod: йәшниктәр/ироха_кли/срк/маин_шар. 766 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`.
- тест: йәшниктәр/ироха_кли/тестар/cly_smoke.rs:111 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`.
- тест: йәшниктәр/ироха_кли/тестар/cly_smoke.rs:5236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: йәшниктәр/ироха_конфиг/срк/параметрҙар/user.rs:3490 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшник/ироха_конфиг/срк/параметрҙар/параметр:11561 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: йәшниктәр/ироха_конфиг/тестар/Fastpq_queue_verride.15 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_конфиг/тестар/фикстуралар. rs:36 — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))` .
- эскәмйә: йәшниктәр/ироха_ядро/эскәмйә/блоктар/common.rs:261 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");` .
- эскәмйә: йәшниктәр/ироха_ядро/эскәмйә/блоктар/дөйөм/мод.р.:272 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`.
- эскәмйә: йәшниктәр/ироха_ядро/эскәмйә/валидация.р.:96 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- төҙөү: йәшниктәр/ироха_ядро/төҙөү.19 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- миҫал: йәшниктәр/ироха_ядро/миҫалдар/генерация_партита_фикстуралар.17 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`X.
- һынау: йәшниктәр/ироха_ядро/срк/башҡарыусы.р. 2385 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- тест: йәшниктәр/ироха_ядро/срк/башҡарыусы.р. 2521 — `let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- һынау: йәшниктәр/ироха_ядро/срк/башҡарыусы.р.:2661 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- prod: йәшниктәр/ироха_ядро/срк/смарт контракттар/is/offline.rs:2152 — `env!("CARGO_MANIFEST_DIR"),`.
- prod: йәшниктәр/ироха_ядро/срк/смарт контракттар/is/offline.rs:2156 — `env!("CARGO_MANIFEST_DIR"),`
- prod: йәшниктәр/ироха_ядро/срк/смарт контракттар/is/offline.rs:2164 — `env!("CARGO_MANIFEST_DIR"),`
- prod: йәшниктәр/ироха_ядро/срк/смарт контракттар/is/offline.rs:2171 — `env!("CARGO_MANIFEST_DIR"),`.
- тест: йәшниктәр/ироха_ядро/срк/аҡыллы контракттар/is/offline.s:4852 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.
- тест: йәшниктәр/ироха_ядро/срк/аҡыллы контракттар/is/repo.s:1870 — `env!("CARGO_MANIFEST_DIR"),`.
- тест: йәшниктәр/ироха_ядро/срк/аҡыллы контракттар/is/repo.s:1874 — `env!("CARGO_MANIFEST_DIR"),`.
- prod: йәшниктәр/ироха_ядро/срк/штат:9990 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_config/iroha_test_config.toml");`
- тест: йәшниктәр/ироха_ядро/срк/штат:12012 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/ироха_ядро/срк/астрелинг.р.:2980 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- һынау: йәшниктәр/ироха_ядро/срк/тх.р:4424 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_ядро/тестар/башҡарма_миграция_интрос.:25 — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/ироха_ядро/тестар/pin_регистр. 1158 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/ироха_ядро/тестар/snapshots.rs:29 — `let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`X
- тест: йәшниктәр/ироха_ядро/тестар/ҡасумераги_док_синх.:57 — `Path::new(env!("CARGO_MANIFEST_DIR"))`- тест: йәшниктәр/ироха_крипто/тестар/конфиденциаль_keyset_vectors. rs:57 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`.
- тест: йәшниктәр/ироха_крипто/тестар/sm2_fixture_vectors. rs:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`X
- тест: йәшниктәр/ироха_крипто/тестар/sm_clix. rs:19 — `env!("CARGO_MANIFEST_DIR"),`
- төҙөү: йәшниктәр/ироха_мәғлүмәттәр_мод.:11 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");`
- prod: йәшниктәр/ироха_мәғлүмәттәр_модель/срк/либ.:186 — `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- prod: йәшниктәр/ироха_мәғлүмәттәр_модель/срк/либ.:190 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/срк/офлайн/посейдон:449 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/срк/соранет/vpn.s:1217 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`
- тест: йәшниктәр/ироха_мәғлүмәттәр/модель/тест/һайлау_адресы_векторҙар.147 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/адресы_кыршы_регист.33 — `let registry_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/конфиденциаль_шифрланған_түләү_векторҙары. rs:48 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`.
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/конфиденциаль_монадерия_фикстуралар.15 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`X .
- тест: йәшниктәр/ироха_мәғлүмәттәр/моделдәр/тестар/консенсус_рауттрип.1308 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тест/офлайн_фикстуралар.р.:130 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/оракле_һылтанма_фикстуралар.25 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_мәғлүмәттәр/моделдәр/тестар/оракле_матик_фикстуралар.29 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/оракле_һылтанма_фикстуралар.33 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/оракле_һылтанма_фикстуралар.37 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_мәғлүмәттәр/модель/тестар/оракле_һылтанма_фикстуралар.41 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_мәғлүмәттәр/модель/тестар/оракле_һылтанма_фикстуралар.р.:46 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/оракле_һылтанма_фикстуралар. rs:50 — `env!("CARGO_MANIFEST_DIR"),`.
- тест: йәшниктәр/ироха_мәғлүмәттәр/модель/тестар/оракле_һылтанма_фикстуралар. rs:54 — `env!("CARGO_MANIFEST_DIR"),`.
- тест: йәшниктәр/ироха_мәғлүмәттәр/моделдәр/тестар/оракле_һылтанма_фикстуралар. rs:58 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/оракле_һылтанма_фикстуралар.р.:62 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/оракле_һылтанма_фикстуралар. 200 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_мәғлүмәттәр/модель/тестар/йүгереү ваҡыты_doc_sync. 8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_генез/src/lib.s:1173 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- тест: йәшниктәр/ироха_генез/src/lib.s:3847 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- тест: йәшниктәр/ироха_генез/src/lib.s:4228 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- тест: йәшниктәр/ироха_генез/src/lib.s:4243 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- тест: йәшниктәр/ироха_и18н/срк/либ.:495 — `let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);`
- һынау: йәшниктәр/ироха_js_host/src/lib.s:6943 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` X
- prod: йәшниктәр/ироха_кагами/өлгөләр/кодек/генерата.13 — `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- prod: йәшниктәр/ироха_кагами/өлгөләр/кодек/срк/маин.35 — `let dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/ироха_кагами/срк/код.:393 — `env!("CARGO_MANIFEST_DIR"),`
- prod: йәшниктәр/ироха_кагами/срк/локальнет. 604 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_кагами/тестар/код.:11 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- тест: йәшниктәр/ироха_телеметрия/тестар/бырауы_лог.р.10 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/ироха_тест селтәре/src/config. rs:724 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`X- тест: йәшниктәр/ироха_тест_ селтәр/src/fsclock_ports.rs:23 — `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- тест: йәшниктәр/ироха_тест селтәре/src/fsclock_ports.rs:25 — `env!("CARGO_MANIFEST_DIR"),`.
- тест: йәшниктәр/ироха_тест_ селтәр/срк/либ.:281 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` X
- тест: йәшниктәр/ироха_тест_проблема/срк/либ.:204 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_тест_проблема/срк/либ.:241 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_тории/срк/да/тесттар. 3490 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- һынау: йәшниктәр/ироха_тории/срк/сораф/апи.р.:4053 — `let matrix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_тории/тестар/иҫәп_адресы_векторҙар.140 — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/ироха_торий/тестар/иҫәптәр_портфель.р.с:91 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/ироха_тории/тестар/сорафтар_асыу. 1056 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- төҙөү: йәшниктәр/вм/төҙөү.21 — `let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);`
- prod: йәшниктәр/срк/бин/ген_аби_хаш_док. 28 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`.
- prod: йәшниктәр/вм/срк/бин/ген_башы_s:48 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`X
- prod: йәшниктәр/вм/срк/бин/gen_point_типтар_s:23 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: йәшниктәр/вм/срк/бин/ген_syscalls_doc.s:24 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- prod: йәшник/vm/src/bin/vm_prebuild.16 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: йәшниктәр/vm/src/bin/vm_predecoder_export.rs:23 — `let _crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: йәшниктәр/vm/src/predecoder_fixtures.rs:236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/predecoder/mixed")`
- тест: йәшниктәр/вм/тестар/cli_smoke.rs:9 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- тест: йәшниктәр/вм/тестар/cli_smoke.rs:57 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- тест: йәшниктәр/вм/тестар/cli_smoke.rs:89 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- тест: йәшниктәр/вм/тестар/доктар_эҙмә-эҙлеклелек.3 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");` XX
- тест: йәшниктәр/вм/тестар/вм_аби_док_синх.:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`X
- тест: йәшниктәр/вм/тест/вм_хатель_doc_sync. 8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/вм/тестар/норито_portal_snippet_compile.19 — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/вм/тестар/пункт_типтар_doc_gened.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- тест: йәшниктәр/вм/тестар/пункт_типтар_doc_gened_vm_md.rs:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/вм/тестар/syscalls_doc_gened.rs:7 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- тест: йәшниктәр/вм/тестар/syscalls_doc_sync.s:8 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`
- тест: йәшниктәр/вм/тестар/syscalls_gas_names.11 — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- эскәмйә: йәшниктәр/норито/эскәмйә/паритет_compare.rs:79 — `let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- төҙөү: йәшниктәр/норито/төҙөү.17 — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));` XX
- prod: йәшниктәр/норито/срк/бин/норито_реген_голденс.р:9 — `Path::new(env!("CARGO_MANIFEST_DIR"))` X.
- тест: йәшниктәр/норито/тестар/аос_ncb_more_golden.s:200 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- тест: йәшниктәр/норито/тестар/json_golden_lover.rs:14 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/норито/тестар/ncb_enum_ema_samples.rs:353 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/норито/тестар/ncb_enum_ema_samples.rs:387 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/норито/тестар/ncb_enum_em_iter_samples.rs:554 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- тест: йәшниктәр/норито/тестар/ncb_enum_emyter_samples.rs:665 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- тест: йәшниктәр/норито/тестар/ncb_num_blarge_fixture.37 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`X
- тест: йәшниктәр/сорафтар_красть/src/bin/da_reconstruction:434 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))` .
- тест: йәшниктәр/сорафтар_красть/src/bin/da_reconstrontrast.710 — `Path::new(env!("CARGO_MANIFEST_DIR"))` XX .
- prod: йәшниктәр/сорафтар_автомобиль/src/bin/soranet_portless_verfier.140 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` X.- тест: йәшниктәр/сорафтар_автомобиль/тестар/ҡорамалдар_simulation_toolkit.s:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/сорафтар_машина/тестар/fetch_cli.s:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/сорафтар_машина/тестар/fetch_cli.s:1043 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/сорафтар_машина/тестар/fetch_cli.s:1159 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/сорафтар_машина/тестар/тайҡай_car_cli.rs:227 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/сорафтар_автомобиль/тестар/тайҡай_күҙлек.22 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/сорафтар_автомобиль/тестар/ышанысһыҙ_верфикатор.8:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`.
- prod: йәшниктәр/сорафтар_chunker/src/bin/экспорт_векторҙары. 175 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/сорафтар_chunker/тестар/ҡайтарыу. rs:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`.
- тест: йәшниктәр/сорафтар_chunker/тестар/векторҙар:12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` X
- тест: йәшниктәр/сорафтар_манифест/тестар/por_fixtures. rs:11 — `env!("CARGO_MANIFEST_DIR"),`
- тест: йәшниктәр/сорафтар_манифест/тестар/провиратор_адмиссия_фикстуралар.12 — `cmd.current_dir(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/сорафтар_манифест/тест/репликация_ордаҡ_фикстуралар. 8 — `env!("CARGO_MANIFEST_DIR"),`
- prod: йәшниктәр/sorafs_node/src/bin/sorafs_gateway.rs:55 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- prod: йәшниктәр/сорафтар_төймә/src/bin/sorafs_gateway.59 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0")`
- тест: йәшниктәр/sorafs_node/src/gateaway.rs:2006 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0");`
- тест: йәшниктәр/сорафтар_төймә/тестар/кли.р.:122 — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`X .
- тест: йәшниктәр/сорафтар_төйөн/тестар/ҡапҡалар:14 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.
- тест: йәшниктәр/сорафтар_төйөн/тестар/ҡапҡалар.р.:30 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`X.
- тест: йәшниктәр/сорафтар_оркестратор/срк/либ.:6316 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`X
- тест: йәшниктәр/сорафтар_оркестратор/срк/либ.:6449 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/сорафтар_оркестратор/срк/либ.:8100 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: йәшниктәр/сорафтар_оркестратор/тестар/оркестры_паритет.р.:180 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: йәшниктәр/соранет_пҡ/тестар/кат_векторҙар.р:9 — `env!("CARGO_MANIFEST_DIR"),`
- төҙөү: интеграция_тестар/төҙөү.17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/src/sorafs_gateaway_cability_refusal.rs:157 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`
- тест: интеграция_тестар/src/sorafs_gateaway_conformancecance.1032 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`X .
- тест: интеграция_тестар/тестар/адресы_канонический. rs:126 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/активтар.rs:49 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`X
- тест: интеграция_тестар/тестар/тиҙ_dsl_build.rs:7 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`X
- тест: интеграция_тестар/тестар/генис_json.16 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/генис_json.rs:22 — `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
- тест: интеграция_тестар/тестар/ироха_кли.р. 24 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/vm_header_decode.rs:49 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/vm_header_smoke.rs:24 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/котодама_миҫәләр.рс:70 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/котодама_миҫәләр.рс:123 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`.
- тест: интеграция_тестар/тестар/котодама_миҫәләр.рс:173 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/нексус/cbdc_rollout_bundle.s:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`.
- тест: интеграция_тестар/тестар/нексус/cbdc_whitelist.rrs:26 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`.
- тест: интеграция_тестар/тестар/нексус/глобаль_коммит.17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- тест: интеграция_тестар/тестар/нексус/линия_регист.12 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/norito_burn_fixture.19 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/репо.р.:31 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: интеграция_тестар/тестар/стриминг/мод.р. 339 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- prod: мочи/мочи-ядро/срк/суҡырыусы. 220 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- prod: мочи/мочи-ядро/срк/суҡырыусы.р.:538 — `let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- тест: мочи/мочи-ядро/срк/тори.:4740 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- һынау: мочи/мочи-ядро/срк/тори.:4755 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- һынау: мочи/мочи-ядро/срк/тории.р. 4770 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));` X
- тест: мочи/мочи-ядро/срк/тори.:4785 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: мочи/мочи-интеграция/тестар/башҡарыусы.р. 168 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- тест: инструменттар/соранет-ҡул ҡыҫҡыс-йәғеле/тестар/фикстуралар_ тикшерелгән. 6 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: ҡоралдар/соранет-ҡул ҡыҫышыу-йәғеле/тестар/интерп-паритет.р.:77 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- тест: инструменттар/соранет-ҡул ҡыҫышыу-йәғни/тестар/perf_gate.rs:173 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`.
- инструмент: xtask/src/bin/control_plane_mock.362 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- инструмент: xtask/src/main.s:11383 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- инструмент: xtask/src/sorafs/gateaway_fixture.rs:28 — `env!("CARGO_MANIFEST_DIR"),`
- инструмент: xtask/src/sorafs/gateaway_fixture.32 — `env!("CARGO_MANIFEST_DIR"),`.
- тест: xtask/тестар/адрес_векторҙар.р:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` XX .
- тест: xtask/тестар/андроид_dashboard_parity.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/кодек_рандар_р.:18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/da_porof_bench.rs:8 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: xtask/тестар/iso_bridge_lint.rs:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/министрлыҡ_агенда.р:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/sns_каталог_разведка:5 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` X.
- тест: xtask/тестар/сорадндар_кли.р. 11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))` XX
- тест: xtask/тестар/сорафтар_fetch_fixture.rs:9 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/соранет_баг_bounty.11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))` XX
- тест: xtask/тестар/соранет_ливлинг.р.:12 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`X.
- тест: xtask/тестар/соранет_ливлинг_м0.30:30 — `let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/соранет_гейтвей_м1.р:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/соранет_гейтвей_м2.р:25 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/soranet_pop_pall.s:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/soranet_pop_plate.rs:77 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`.
- тест: xtask/тестар/soranet_pop_plate.s:131 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/soranet_pop_plate.rs:202 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`X .
- тест: xtask/тестар/soranet_pop_bhande.306 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))` .
- тест: xtask/тестар/soranet_pop_plate.rs:356 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`X
- тест: xtask/тестар/soranet_pop_plate.rs:489 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`X
- тест: xtask/тестар/аҙығын_баҫым_чек.р:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- тест: xtask/тестар/астрелинг_энтропия_бэх.:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_VERSION (прод: 11, тест: 1, ҡорал: 1)- prod: йәшниктәр/ироха/срк/клиент.рс:478 — `map.insert("version".into(), JsonValue::from(env!("CARGO_PKG_VERSION")));`
- prod: йәшниктәр/ироха_кли/срк/командалар/сорафс.р. 1767 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- prod: йәшниктәр/ироха_кли/срк/маин_шар. rs:72 — `#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]`
- prod: йәшниктәр/ироха_кли/срк/маин_шар. 530 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`.
- тест: йәшниктәр/ироха_кли/тестар/cly_smoke.359 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`.
- prod: йәшниктәр/ироха_ядро/срк/смераги/rbc_store.s:40 — `version: env!("CARGO_PKG_VERSION").to_owned(),`
- prod: йәшниктәр/ироха_js_host/src/lib.s:2998 — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));` XX
- prod: йәшниктәр/ироха_телеметрия/срк/ws.rs:243 — `env!("CARGO_PKG_VERSION")`X
- prod: йәшниктәр/родад/срк/маин. 502 — `version = env!("CARGO_PKG_VERSION"),`
- prod: йәшниктәр/rohad/src/main. 3986 — `version = env!("CARGO_PKG_VERSION"),`
- prod: йәшниктәр/сорафтар_автор/срк/бин/сорафтар_fetch. 1111 — `Value::from(env!("CARGO_PKG_VERSION")),`
- prod: йәшниктәр/сорафтар_оркестратор/срк/бин/сорафтар_cli.s:93 — `const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");`
- инструмент: инструменттар/телеметрия-схема-дифф/срк/маин. 248 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`.

## CARGO_PRIMARY_PACKAGE (төҙөү: 1)

- төҙөү: йәшниктәр/соранет_пҡ/төҙөлөш. 6 — `if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {`

## CARGO_TARGET_DIR (прод: 3, тест: 2, ҡорал: 1)

- тест: йәшниктәр/ироха_тест_ селтәр/срк/либ.:524 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- тест: йәшниктәр/ироха_тест_ селтәр/срк/либ.:759 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- prod: мочи/мочи-ядро/срк/суҡырыусы.р.:575 — `let cargo = env::var_os("CARGO")`.
- prod: мочи/мочи-ядро/срк/суҡырыусы. 629 — `let target_root = env::var_os("CARGO_TARGET_DIR")`X
- prod: мочи/мочи-ядро/срк/суҡырыусы.р.:683 — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- инструмент: xtask/src/mochi.383 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## CARGO_SPACE_DIR (тест: 1)

- тест: йәшниктәр/ироха_ядро/срк/штат:12015 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.

## CRYPTO_SM_INTRINSICS (эскәмйә: 1)

- эскәмйә: йәшниктәр/ироха_крипто/эскәмйә/см_перф.:183 — `let raw_policy = match std::env::var("CRYPTO_SM_INTRINSICS") {`.

## CUDA_HOME (төҙөү: 2)

- төҙөү: йәшниктәр/Fastpq_prover/төҙөлөш.198 — `env::var_os("CUDA_HOME")`
- төҙөү: йәшниктәр/норито/тиҙләткестәр/jsnastage1_cuda/build.rs:63 — `let root = env::var_os("CUDA_HOME")`

## CUDA_PATH (төҙөү: 2)

- төҙөү: йәшниктәр/Fastpq_prover/build.rs:199 — `.or_else(|| env::var_os("CUDA_PATH"))`
- төҙөү: йәшниктәр/норито/тиҙләткестәр/jsnastage1_cuda/build.rs:64 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.

## ДАТАСЕСЕСАВИРЕКСАЙЫШ_АРТИРИДИР (тест: 1)

- тест: интеграция_тестар/тестар/нексус/кросс_лин.р.р. 686 — `let cargo = env::var_os("CARGO")`.

## DOCS_RS (төҙөү: 1)

- төҙөү: йәшниктәр/норито/төҙөү. 8 — `if env::var_os("DOCS_RS").is_some() {` XX

## ЭНУМ_БЕНЧ_Н (элюк: 1)

- эскәмйә: йәшниктәр/норито/эскәмйә/энум_пакет_бэх.:75 — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_DEBUG_FUSED (прод: 1)

- prod: йәшниктәр/Fastpq_prover/src/эҙ.:98 — `Some(*DEBUG_FUSED_ENV.get_or_init(|| env::var_os("FASTPQ_DEBUG_FUSED").is_some()))`

## FASTPQ_EXPECTED_KIB (тест: 1)

- тест: йәшниктәр/Fastpq_prover/тестар/перф_производство.rs:88 — `env::var("FASTPQ_EXPECTED_KIB")`

## FASTPQ_EXPECTED_MS (тест: 1)

- тест: йәшниктәр/Fastpq_prover/тестар/перф_производство.80 — `env::var("FASTPQ_EXPECTED_MS")`

## FASTPQ_PROOF_ROWS (тест: 1)

- тест: йәшниктәр/Fastpq_prover/тестар/перф_производство.72 — `env::var("FASTPQ_PROOF_ROWS")`

## FASTPQ_SKIP_GPU_BUILD (төҙөү: 1)

- төҙөү: йәшниктәр/Fastpq_prover/төҙөлөш. 45 — `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## FASTPQ_UPDATE_FIXTURES (тест: 6)- тест: йәшниктәр/Fastpq_prover/тестар/бэкэнд_регрессия.р.:47 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`X .
- тест: йәшниктәр/Fastpq_prover/тестар/бэкэнд_регрессия.rs:69 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.
- тест: йәшниктәр/Fastpq_prover/тестар/иҫбатлау_фикстура. 43 — `let cargo = env::var_os("CARGO")`.
- тест: йәшниктәр/Fastpq_prover/тестар/эҙләү_комитеты. 23 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`X
- тест: йәшниктәр/Fastpq_prover/тестар/эҙләү_комитет.111 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- тест: йәшниктәр/Fastpq_prover/тестар/транскрипт_реплей.р.:67 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`

## GENESIS_DeBUG_MODE (тест: 1)

- тест: йәшниктәр/ироха_тест_селтәр/миҫалдар/генис_debug.s:15 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.

## GENESIS_DeBUG_PAYLOAD (тест: 1)

- тест: йәшниктәр/ироха_тест_ селтәр/миҫалдар/генис_debug.s:122 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## ГИТУБ_СТЕП_СУММАР (прод: 2)

- prod: йәшниктәр/ироха_крипто/src/bin/gost_perf_check.rs:22 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- prod: йәшниктәр/ироха_крипто/срк/бин/см_перф_чек.р. 205 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`.

## ГИТ_КОММИТ_ХӘШ (прод: 1)

- prod: йәшниктәр/ироха_ядро/срк/смераги/rbc_store.s:42 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`

## ӨЙҘӘ (прод: 1)

- prod: йәшниктәр/ироха/срк/config.57 — `env::var_os("HOME").map(PathBuf::from)`

## ИРОХА_АЛЛОУ_НЕТ (тест: 1)

- тест: йәшниктәр/занами/срк/хаос. 374 — `let cargo = env::var_os("CARGO")`.

## IROHa_CONF_GAS_EDED (тест: 1)

- тест: йәшниктәр/ироха_тест_проблема/срк/либ.:57 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`X

## IROHa_DA_SPOOL_DIR (тест: 1)

- тест: йәшниктәр/ироха_ядро/срк/штат:9096 — `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`

## IROHa_METRICS_PANIC_ON_DUPLICATE (тест: 2)

- тест: йәшниктәр/ироха_телеметрия/src/src/метрҙар:11397 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`
- тест: йәшниктәр/ироха_тории/тестар/метрика_регистр.33 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.

## IROHa_RUN_IGNORED (тест: 71)- тест: йәшниктәр/ироха_ядро/тестар/gov_auto_close_раҫы. rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_finalize_real_vk.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_min_duration.18 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`
- һынау: йәшниктәр/ироха_ядро/тестар/gov_mode_mismicatch.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_mode_mismicatch_zk.s:29 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`
- тест: йәшниктәр/ироха_ядро/тестар/gov_plain_bollot.s:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`.
- тест: йәшниктәр/ироха_ядро/тестар/gov_plain_burniction.20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {` X
- тест: йәшниктәр/ироха_ядро/тестар/gov_plain_disabled.s:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_plain_missing_s ref.15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- һынау: йәшниктәр/ироха_ядро/тестар/gov_plain_revote_mononic.18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- һынау: йәшниктәр/ироха_ядро/тестар/gov_proted_gate.rs:64 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_refreendum_epen_close.24 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_ports.rs:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_ports.rs:82 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_ports_posive.20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_out_sweep.16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`.
- тест: йәшниктәр/ироха_ядро/тестар/gov_zk_bollet_lock_tered.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_zk_ballot_real_vk.rs:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/gov_zk_referendum_window_guard.16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_ядро/тестар/зк_тамырҙары_get_cap.s:29 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: йәшниктәр/ироха_ядро/тестар/зк_vote_get_rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- һынау: йәшниктәр/ироха_крипто/срк/меркле.р. 1317 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`.
- тест: йәшниктәр/ироха_крипто/срк/меркле.р. 1332 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_крипто/тестар/меркл_норито_раундтрип.18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`.
- тест: йәшниктәр/ироха_крипто/тестар/меркл_норито_раундтрип.р.:42 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/срк/блок/баш:624 — `let cargo = env::var_os("CARGO")`.
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/срк/si/mod.rs:1888 — `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`.
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/src/si/register.rs:324 — `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`
- тест: йәшниктәр/ироха_мәғлүмәттәр моделдәре/срк/иҫбатлау.798 — `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- тест: йәшниктәр/ироха_мәғлүмәттәр моделдәре/срк/транзакция/ҡулланыу. rs:1048 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/инструкция_регистрация_ялҡаулыҡ_инит.р:8 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`.
- тест: йәшниктәр/ироха_мәғлүмәттәр/моделдәр/тестар/инструкция_регист_ресесс.7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/модель_десплавный.15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тест/модель_депро.р.р. 37 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`.
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тестар/модель_репро.р. 58 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тест/модель_десплавный_репро.р. 84 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`.
- тест: йәшниктәр/ироха_мәғлүмәттәр/моделдәр/тестар/регистрация_decode_rountrip. rs:9 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`.
- тест: йәшниктәр/ироха_мәғлүмәттәр/моделдәр/тестар/тракт_объекттар.р:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_мәғлүмәттәр_модель/тест/һыҙыҡтар/тикшерәләр. 27 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`- тест: йәшниктәр/ироха_мәғлүмәттәр/моделдәр/тестар/зк_энвертрип.р.с:6 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: йәшниктәр/ироха_торий/тестар/контракттар_активлау.р.р. 23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_торий/тестар/контракттар_активлау.р. 174 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`.
- тест: йәшниктәр/ироха_торий/тестар/контракттар_шылтыратыу_интеграция.21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_торий/тестар/контракттар_баҫмаһы_интеграция. 23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`X .
- тест: йәшниктәр/ироха_торий/тестар/контракттар_инстант_активация_интеграция.19 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {` .
- тест: йәшниктәр/ироха_тории/тестар/контракттар_күҙәктәр_исправник_марш.:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`.
- тест: йәшниктәр/ироха_торий/тестар/gov_concil_persist_integration.22 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {` X.
- һынау: йәшниктәр/ироха_торий/тестар/gov_concil_vrf.:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_тории/тестар/gov_enact_ter.rs:12 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_тории/тестар/gov_instances_list. rs:14 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: йәшниктәр/ироха_торий/тестар/gov_mode_mismisatch_and_aatoclose.42 — `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- тест: йәшниктәр/ироха_тории/тестар/gov_proted_entpoints.rs:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/ироха_торий/тестар/gov_proted_ents_marter.rs:20 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`.
- тест: йәшниктәр/ироха_тории/тестар/gov_redations_router.rs:21 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/вм/тест/белем_тест.rs:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: йәшниктәр/вм/тестар/котодама_структура_яландары.11 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {` XX
- тест: йәшниктәр/вм/тестар/зк_тамырҙары_һәм_vote_syscalls.16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {` X
- тест: йәшниктәр/вм/тестар/зк_тамырҙары_һәм_vote_syscalls.rs:50 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: интеграция_тестар/тестар/ваҡиғалар/хәбәр итеү. rs:81 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: интеграция_тестар/тестар/өҫтәмә_функциональ/стабиль_network.rs:753 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: интеграция_тестар/тестар/өҫтәмә_функциональ/стабиль_network.rs:769 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {` .
- тест: интеграция_тестар/тестар/өҫтәмә_функциональ/стабиль_network.rs:785 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: интеграция_тестар/тестар/өҫтәмә_функциональ/стабиль_network.rs:801 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: интеграция_тестар/тестар/өҫтөндә_функциональ/стабиль_network.rs:817 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`X .
- тест: интеграция_тестар/тестар/рөхсәттәр.р:202 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: интеграция_тестар/тестар/рөхсәттәр.р.:265 — `let cargo = env::var_os("CARGO")`.
- тест: интеграция_тестар/тестар/рөхсәттәр.р.:328 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`X
- тест: интеграция_тестар/тестар/тормошонда_блок_кире өҙөлгән.17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: интеграция_тестар/тестар/сортлау.р.:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- тест: интеграция_тестар/тестар/триггер/ by_croll_trigger.139 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: интеграция_тестар/тестар/триггер/ваҡыт_триггер. 149 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {` .

## ИРОХА_РУН_ZK_WRAPPERS (тест: 1)

- тест: йәшниктәр/вм/тестар/котодама_врапперс.р:2 — `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`

## IROHa_SKIP_BIND_CHECKS (тест: 1)

- тест: йәшниктәр/ироха_тест_сентовый/срк/либ.:3098 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (тест: 1)

- тест: йәшниктәр/ироха_крипто/тестар/sm_clix.rs:47 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`X

## ИРОХА_ТЕСТ_ДУМП_ГЕНЕЗ (тест: 1)

- тест: йәшниктәр/ироха_тест_ селтәр/срк/либ.:6661 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.## IROHa_ETEST_PREBUILD_DEFAULT_EXCUTOR (төҙөү: 1, тест: 1)

- тест: йәшниктәр/ироха_тест/серц/срк/config. 155 — `let cargo = env::var_os("CARGO")`.
- төҙөү: интеграция_тестар/төҙөлөш. 205 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`.

## ИРОХА_ТЕСТ_SKIP_BUILD (тест: 1)

- тест: йәшниктәр/ироха_тест_ селтәр/срк/либ.:1244 — `std::env::var("IROHA_TEST_SKIP_BUILD")`

## ИРОХА_ТЕСТ_ТАРЖЕТ_DIR (тест: 2)

- тест: йәшниктәр/ироха_тест/срк/либ.р.:521 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`
- тест: йәшниктәр/ироха_тест_ селтәр/срк/либ.:765 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`

## IROHa_ETEST_USE_DEFAULT_EXCUTOR (тест: 3)

- һынау: йәшниктәр/ироха_ядро/срк/башҡарыусы.р.:2383 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- тест: йәшниктәр/ироха_ядро/срк/башҡарыусы.р. 2520 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- тест: йәшниктәр/ироха_ядро/срк/штат:12008 — `if std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR").is_some() {`

## ИРОХА_ТОРИИ_ОПЕНАПИ_АКТУАЛ (тест: 1)

- тест: йәшниктәр/ироха_тории/тестар/маршрут_функциональ_матрица. rs:94 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`X .

## IROHa_TORIII_OPENAPI_EXPECTED (тест: 2)

- тест: йәшниктәр/ироха_тории/тестар/маршрут_функциональ_матрица. 88 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.
- тест: йәшниктәр/ироха_торий/тестар/маршрутизаторҙар_мөхәббәт_матрица.104 — `let cargo = env::var_os("CARGO")`.

## ИРОХА_ТОРИИ_ОПЕНАПИ_ТОКЕНС (ҡорал: 2)

- инструмент: xtask/src/main.s:11025 — `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`X
- инструмент: xtask/src/main.s:11077 — `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IVM_BIN (тест: 2)

- тест: интеграция_тестар/тестар/котодама_миҫәләр.рс:60 — `let ivm_bin = env::var("IVM_BIN")`
- тест: интеграция_тестар/тестар/котодама_миҫәләр.рс:163 — `let ivm_bin = env::var("IVM_BIN")`

## IVM_COMPILER_DeBUG (прод: 1)

- prod: йәшниктәр/котодама_ланг/срк/компиляр. 4094 — `let height_filter = env::var("BLOCK_DUMP_HEIGHTS").ok().map(|raw| {`.

## IVM_CUDA_GENCODE (төҙөү: 1)

- төҙөү: йәшниктәр/вм/төҙөү.35 — `env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| "arch=compute_61,code=sm_61".to_string());`

## IVM_CUDA_NVCC (төҙөү: 1)

- төҙөү: йәшниктәр/вм/төҙөү.30 — `let nvcc = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVVCC_EXTRA (төҙөү: 1)

- төҙөү: йәшниктәр/вм/төҙөү.36 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`.

## IVM_DEBUG_IR (тест: 1)

- тест: йәшниктәр/вм/тестар/разбатка_р.:15 — `if std::env::var_os("IVM_DEBUG_IR").is_some() {` .

## IVM_DeBUG_METAL_ENUM (разработка: 1)

- отладка: йәшниктәр/vm/src/vector.rs:474 — `let cargo = env::var_os("CARGO")`.

## IVM_DEBUG_METAL_SELFTEST (отладка: 1)

- отладка: йәшниктәр/вм/срк/вектор.р. 1205 — `let cargo = env::var_os("CARGO")`

## IVM_DISABLE_CUDA (отладка: 1)

- отладка: йәшниктәр/src/cruda.rs:315 — `&& std::env::var("IVM_DISABLE_CUDA")`

## IVM_DISABLE_METAL (отладка: 1)

- отладка: йәшниктәр/вм/срк/вектор.р. 299 — `let disabled = std::env::var("IVM_DISABLE_METAL")`

## IVM_FORCE_CUDA_SELFTEST_FAIL (оставка: 1)

- отладка: йәшниктәр/срк/cuda.rs:326 — `&& std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL")`

## IVM_FORCE_METAL_ENUM (оставка: 1)

- отладка: йәшниктәр/срк/вектор.р. 444 — `std::env::var("IVM_FORCE_METAL_ENUM")`

## IVM_FORCE_METAL_SELFTEST_FAIL (разработка: 1)

- отладка: йәшниктәр/вм/срк/вектор.р. 1192 — `std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")`

## IVM_TOOL_BIN (тест: 1)

- тест: интеграция_тестар/тестар/котодама_миҫәләр.рс:113 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## ИҘӘМИ_АЛЛОВ_NET (тест: 1)

- тест: йәшниктәр/занами/срк/хаос. 373 — `std::env::var("IZANAMI_ALLOW_NET")`

## ИЗАНМИ_ТУИ_АЛЛОВ_ЗЕРОСЕД (прод: 1)

- prod: йәшниктәр/занами/срк/туй.р. 134 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (төҙөү: 1)

- төҙөү: йәшниктәр/норито/тиҙләткестәр/jsnastage1_cuda/build.rs:40 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_SKIP_BUILD (төҙөү: 1)

- төҙөү: йәшниктәр/норито/тиҙләткестәр/jsnastage1_cuda/build.s:18 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## КОТО_БИН (тест: 2)- тест: интеграция_тестар/тестар/котодама_миҫәләр.рс:50 — `let koto_bin = env::var("KOTO_BIN")`
- тест: интеграция_тестар/тестар/котодама_миҫәләр.рс:154 — `let koto_bin = env::var("KOTO_BIN")`

## LANG (тест: 3)

- тест: йәшник/вм/срк/бин/кото_линт.rs:702 — `let previous = env::var("LANG").ok();`
- тест: йәшниктәр/вм/тестар/i18н.11 — `let old_lang = env::var("LANG").ok();`
- тест: йәшниктәр/вм/тестар/i18n.rs:69 — `let old_lang = env::var("LANG").ok();`

## LC_ALL (тест: 2)

- тест: йәшниктәр/вм/тестар/i18n.р:12 — `let old_lc_all = env::var("LC_ALL").ok();`
- тест: йәшниктәр/вм/тестар/i18n.rs:70 — `let old_lc_all = env::var("LC_ALL").ok();`

## LC_MESSAGES (тест: 2)

- тест: йәшниктәр/вм/тестар/i18n.s:13 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`
- тест: йәшниктәр/вм/тестар/i18n.rs:71 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## MAX_DEGRE (прод: 1)

- prod: йәшниктәр/ироха_ядро/срк/зк.р:106 — `let current = std::env::var("MAX_DEGREE")` X

## МОХИ_КОНФИГ (прод: 1)

- prod: мочи/мочи-уи-эгуи/срк/config.325 — `if let Some(value) = env::var_os("MOCHI_CONFIG").filter(|value| !value.is_empty()) {`

## МОХИ_ДАТА_РООТ (прод: 1)

- prod: мочи/мочи-ядро/срк/суҡырыусы.р.:2068 — `std::env::var_os("MOCHI_DATA_ROOT")`

## МОХИ_ТЕСТ_USE_INTERNAL_GENESIS (прод: 1)

- prod: мочи/мочи-ядро/срк/суҡырыусы.р.:1994 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.

## НОРИТО_БЕНЧ_СУММАРИ (эскәмйә: 1)

- эскәмйә: йәшниктәр/норито/эскәмйә/паритет_compare.s:70 — `if std::env::var("NORITO_BENCH_SUMMARY").ok().as_deref() == Some("1") {`

## NORITO_CPU_INFO (ҡорал: 1)

- инструмент: xtask/src/stage1_bench.rs:69 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## NORITO_CRC64_GPU_LIB (прод: 1)

- prod: йәшниктәр/норито/срк/ядро/simd_ccc64.р:215 — `std::env::var("NORITO_CRC64_GPU_LIB").ok(),`

## NORITO_DISABLE_PACKED_STRUCT (прод: 1, тест: 1)

- прод: йәшниктәр/ироха_js_host/src/lib.s:183 — `let env_set = std::env::var_os("NORITO_DISABLE_PACKED_STRUCT").is_some();`
- тест: йәшниктәр/норито/срк/либ. 322 — `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## NORITO_GPU_CRC64_MIN_BYTES (прод: 1)

- prod: йәшниктәр/норито/срк/ядро/simd_ccc64.р:83 — `std::env::var("NORITO_GPU_CRC64_MIN_BYTES").ok(),`.

## NORITO_PAR_STAGE1_MIN (тест: 1)

- тест: йәшниктәр/норито/срк/lib.s:4847 — `std::env::var("NORITO_PAR_STAGE1_MIN")` X

## NORITO_SKIP_BINDINGS_SYNC (төҙөү: 1)

- төҙөү: йәшниктәр/норито/төҙөү.12 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## NORITO_STAGE1_GPU_MIN_BYTES (тест: 1)

- тест: йәшник/норито/срк/либ.:4881 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## НОРИТО_ТРАЦИ (тест: 2)

- тест: йәшниктәр/норито/срк/либ.:123 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- тест: йәшниктәр/норито/срк/либ.:138 — `let env_enabled = env::var_os("NORITO_TRACE").is_some();`

## NO_PROXY (прод: 1)

- prod: йәшниктәр/ироха_п2п/срк/транспорт.р. 469 — `let no_proxy = env::var("NO_PROXY")`

## НВКК (төҙөү: 1)

- төҙөү: йәшниктәр/вм/төҙөү.31 — `.or_else(|_| env::var("NVCC"))`

## OUT_DIR (төҙөү: 4, прод: 12, тест: 2)- төҙөү: йәшниктәр/Fastpq_prover/төҙөлөш. 99 — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`X
- төҙөү: йәшниктәр/ироха_мәғлүмәттәр_мод.:12 — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));`
- prod: йәшниктәр/ироха_мәғлүмәттәр_модель/срк/либ.:179 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
- төҙөү: йәшниктәр/вм/төҙөү. 27 — `let out_dir = PathBuf::from(env::var("OUT_DIR")?);` XX
- төҙөү: йәшниктәр/вм/төҙөү.120 — `if let Some(out_dir) = env::var_os("OUT_DIR") {`
- prod: йәшниктәр/vm/src/cuda.rs:12 — `static PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));`
- prod: йәшниктәр/vm/src/cuda.rs:13 — `static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));`
- prod: йәшниктәр/vm/src/cuda.rs:14 — `static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));`
- prod: йәшниктәр/vm/src/cuda.rs:15 — `static SHA_LEAVES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx"));`
- prod: йәшниктәр/vm/src/cuda.rs:16 — `static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));`
- prod: йәшниктәр/vm/src/cuda.rs:17 — `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`X
- prod: йәшниктәр/vm/src/cuda.rs:18 — `static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));`
- prod: йәшниктәр/vm/src/cuda.rs:19 — `static BN254_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bn254.ptx"));`.
- prod: йәшниктәр/vm/src/cuda.rs:20 — `static SIG_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/signature.ptx"));`X
- prod: йәшниктәр/vm/src/cuda.rs:21 — `static SHA_PAIRS_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx"));`
- prod: йәшниктәр/vm/src/cuda.rs:22 — `static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));`
- тест: йәшниктәр/вм/срк/ptx_tests.rs:7 — `let out_dir = match std::env::var("OUT_DIR") {`
- тест: йәшниктәр/вм/тестар/ptx_kernels.rs:5 — `let out_dir = env!("OUT_DIR");`

## P2P_TURN (прод: 1)

- prod: йәшниктәр/ироха_п2п/срк/транспорт.р. 296 — `let endpoint = std::env::var("P2P_TURN")`

## ПАТТ (прод: 2, тест: 1)

- prod: йәшниктәр/ироха_кли/срк/командалар/сорафс.р. 7276 — `if let Some(path_var) = env::var_os("PATH") {`
- тест: интеграция_тестар/тестар/котодама_миҫалдар.р. 17 — `let path = env::var_os("PATH")?;` XX .
- prod: мочи/мочи-ядро/срк/суҡырыусы. 511 — `let path_var = env::var_os("PATH")?;`

## ПРИНТ_СОРАКЛЕС_ФИКСТУРС (тест: 1)

- тест: йәшниктәр/ироха_мәғлүмәттәр моделдәре/срк/оракле/мод.р. 3249 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`.

## ПРИНТ_ТОРИИ_СПЕК (тест: 1)

- тест: йәшниктәр/ироха_тории/срк/опенапи.р. 4380 — `if std::env::var("PRINT_TORII_SPEC").is_ok() {`X

## ПРОФИЛ (төҙөү: 1, прод: 1, тест: 1)

- prod: йәшниктәр/ироха_ядро/срк/смераги/rbc_store.s:41 — `profile: option_env!("PROFILE").unwrap_or("unknown").to_owned(),`
- тест: йәшниктәр/ироха_тест_сентовый/src/lib.s:995 — `let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());`
- төҙөү: интеграция_тестар/төҙөү.180 — `let profile = match env::var("PROFILE").unwrap_or_default().as_str() {`

## ПИТОН3 (тест: 2)

- тест: йәшниктәр/сорафтар_машина/тестар/тайҡай_car_cli.rs:235 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`
- тест: йәшниктәр/сорафтар_автомобиль/тестар/тайҡай_күҙлек.30:30 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`.

## ПИТОНПАТ (тест: 1)

- тест: йәшниктәр/ироха_кли/тестар/cly_smoke.rs:5245 — `match env::var("PYTHONPATH") {`

## РБК_СЕССИЯ_ПАТА (тест: 1)

- тест: йәшниктәр/ироха_ядро/срк/смераги/rbc_store.rs:823 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`.

## REPO_PROOF_DEST_OUT (тест: 1)

- тест: йәшниктәр/ироха_ядро/срк/аҡыллы контракттар/is/repo.s:2017 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (тест: 1)

- тест: йәшниктәр/ироха_ядро/срк/аҡыллы контракттар/is/repo.s:2005 — `let cargo = env::var_os("CARGO")`.

## РУСТ_LOG (прод: 1, тест: 2)

- тест: йәшниктәр/ироха_тест_ селтәр/срк/либ.:5414 — `let cargo = env::var_os("CARGO")`
- тест: йәшниктәр/ироха_тест_сентовый/срк/либ.:5431 — `let original = env::var("RUST_LOG").ok();`
- prod: йәшниктәр/занами/src/config.rs:269 — `let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string());`

## SM_PERF_CPU_LABEL (прод: 2)

- prod: йәшниктәр/ироха_крипто/срк/бин/см_перф_чек.р. 637 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.
- prod: йәшниктәр/ироха_крипто/срк/бин/см_перф_чек.р. 679 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## ЯУАП_НОДСКИПСКИП_ИНГЕСТ_ТЕСТТАР (тест: 1)

- тест: йәшниктәр/сорафтар_төймә/тестар/кли.р.:17 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`.

## ЯУАП_ТОРИИ_СКИП_ИНГЕСТ_ТЕСТТАР (тест: 1)

- тест: йәшниктәр/ироха_тории/тестар/сорафтар_асыу. 95 — `std::env::var("SORAFS_TORII_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`## СУМЕРАГИ_ЯҠТЫҠ_АРТИФИР (тест: 1)

- тест: интеграция_тестар/тестар/сумераги_адверсариаль.р.:1222 — `let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {`X

## СУМЕРАГИ_БАЗИН_АРТИФИР (прод: 1, тест: 1)

- prod: йәшниктәр/төҙөү-ярҙам/срк/бин/смераги_базалин_реше. 40 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.
- тест: интеграция_тестар/тестар/сумераги_нпос_перформанс.1396 — `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`.

## СУМЕРАГИ_ДА_АРТИФИР (прод: 1, тест: 1)

- prod: йәшниктәр/төҙөү-ярҙам/срк/бин/смераги_да_репорт. 41 — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`.
- тест: интеграция_тестар/тестар/сумераги_да.р.:1599 — `let Ok(dir) = std::env::var("SUMERAGI_DA_ARTIFACT_DIR") else {`

## Systemroot (прод: 1)

- prod: йәшниктәр/Fastpq_prover/src/backend.rs:535 — `env::var_os("SystemRoot").map(PathBuf::from)`

## ТАРГЕТ (ҡорал: 2)

- инструмент: xtask/src/poseidon_bench.rs:87 — `target: std::env::var("TARGET")`
- инструмент: xtask/src/stage1_bench.rs:63 — `target: std::env::var("TARGET")`

## ТЕСТ_LOG_FILTER (прод: 1)

- prod: йәшниктәр/ироха_логер/срк/либ.р. 89 — `filter: std::env::var("TEST_LOG_FILTER")`

## ТЕСТ_LOG_LEVEL (прод: 1)

- prod: йәшниктәр/ироха_логгер/срк/либ.р. 85 — `level: std::env::var("TEST_LOG_LEVEL")`

## ТЕСТ_НЕТВОРК_КАРГО (тест: 1)

- тест: йәшниктәр/ироха_тест/срк/src/lib.s:865 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`X .

## ТОРИ_ДЕБУГ_СОРТ (прод: 1)

- prod: йәшниктәр/ироха_тории/срк/маршрутлау.р. 12530 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`.

## ТОРИ_МОК_ХАРНЕССМИКСАТКАҺЫСАТИКСАТ (инструмент: 1)

- инструмент: xtask/src/bin/torii_mock_harness.106 — `let cargo = env::var_os("CARGO")`.

## ТОРИЯ_МОК_ХАРНЕССЕС_РЕСПОООТ (ҡорал: 1)

- инструмент: xtask/src/bin/torii_mock_harness.109 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`X.

## ТОРИ_МОК_ХАРНЕССА_РЕТРИ_ТОТАЛЬ (ҡорал: 1)

- инструмент: xtask/src/bin/torii_mock_harness.297 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## ТОРИЯ_МОК_ХАРНЕССАСССИРУНЕР (ҡорал: 1)

- ҡорал: xtask/src/bin/torii_mock_harness.112 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## ТОРИ_МОК_ХАРНЕССДК (инструмент: 1)

- инструмент: xtask/src/bin/torii_mock_arness.104 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`.

## ТОРИ_ОПЕНАПИ_ТОКЕН (ҡорал: 2)

- инструмент: xtask/src/main.s:11020 — `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`.
- инструмент: xtask/src/main.s:11073 — `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## ЯҢЫРТЫУСЫЛЫҠСА (тест: 1)

- тест: йәшниктәр/ироха_ядро/тестар/snapshots.rs:42 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`

## USERPROFILE (прод: 1)

- prod: йәшниктәр/ироха/срк/config. 55 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`.

## ВЕРГЕН_КАРГО_ФЕАТУРС (прод: 1)

- prod: йәшниктәр/родад/срк/маин.р. 4850 — `const VERGEN_CARGO_FEATURES: &str = match option_env!("VERGEN_CARGO_FEATURES") {`

## ВЕРГЕН_КАРГО_ТАРГЕТ_ТРИПЛЫ (прод: 1)

- прод: йәшниктәр/ироха_телеметрия/срк/ws.rs:238 — `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");` XX

## ВЕРГЕН_GIT_SHA (прод: 4)

- prod: йәшниктәр/ироха_кли/срк/маин_шар. rs:57 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`X
- prod: йәшниктәр/ироха_телеметрия/срк/ws.rs:237 — `let vergen_git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");`
- prod: йәшниктәр/ироха_тории/срк/маршрутлау.32515 — `git_sha: option_env!("VERGEN_GIT_SHA")`
- prod: йәшниктәр/родад/срк/маин.р.р.:4845 — `const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {`

## ВЕРИФИ_БАТЧ (эскәмйә: 1)

- эскәмйә: йәшниктәр/вм/эскәмйә/эскәмйә_вос.:225 — `let verify_batch = std::env::var("VERIFY_BATCH")`

## ВЕРИФИ_СЕВЕ (эск: 1)

- эскәмйә: йәшниктәр/вм/эскәмйә/эскәмйе_вос.:213 — `let verify_every: u64 = std::env::var("VERIFY_EVERY")`

## ҠАТЫР (эскәмйә: 1)

- эскәмйә: йәшниктәр/вм/эскәмйә/эскәмйә_вос.:207 — `let voters: u64 = std::env::var("VOTERS")` .

## no_proxy (прод: 1)

- prod: йәшниктәр/ироха_п2п/срк/транспорт.р. 471 — `.or_else(|| env::var("no_proxy").ok());`.