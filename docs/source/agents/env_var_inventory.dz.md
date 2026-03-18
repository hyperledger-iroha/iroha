---
lang: dz
direction: ltr
source: docs/source/agents/env_var_inventory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0a23896374a10f0862aeec69c607ba3022d3b337ae7f6bb54a61b8f7424410
source_last_modified: "2026-01-21T19:17:13.235848+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ཁོར་ཡུག་བཤིག་པའི་ཐོ་ཡིག།

_མཐའ་མཇུག་ `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`_ བརྒྱུད་དེ་ གསར་བསྐྲུན་འབད་ཡོདཔ།

ཡོངས་བསྡོམས་གཞི་བསྟུན་ཚུ་: **505** · ཁྱད་པར་ཅན་གྱི་འགྱུར་ཅན་: **137**

## ACTIONS_ID_TOKEN_REQUEST_TOKEN (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/སོ་རཕ་ས_ཨོར་ཀེཊ་ཊར་/ཨེསི་ཨར་སི/བིན་/སོ་རཕ་ས_ཀླི་.313 — `let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {`:313 དང་།

## ACTIONS_ID_TOKEN_REQUEST_URL (prod: 1)

- པྲོཌ་: ཀེརེ་ཊི/སོ་རཕ་ས_ཨོར་ཀེཊ་ཊར་/ཨེསི་ཨར་སི/བིན་/སོ་རཕ་ས_ཀླི་.s:310 — `let raw_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL").map_err(|_| {`:

## BLOCK_DUMP_HEIGHTS (དཔེར་ན་: ༡)

- དཔེར་ན: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/པེལཀ་_ཌམཔ་.༦༢ — I༡༨NI00000003X

## BLOCK_DUMP_SUM_ASSET (དཔེར་ན་: ༡)

- དཔེར་ན: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/པེལཀ་_ཌམཔ་.༥༡ — I༡༨NI00000004X

## BLOCK_DUMP_VERBOSE (དཔེར་ན་: ༡)

- དཔེར་ན: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/དཔེ་/བཀག་ཆ་_ཌམཔ་.༥༠ — I༡༨NI00000005X

## ཀར་གོ་ (prod: 3, བརྟག་དཔྱད: ༢)

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:1026 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་སི་_མན་ཆད་/བརྟག་དཔྱད་/མཁོ་སྤྲོད་_འཛུལ་ཞུགས་_ཕིག་ཅུ། 11 — `let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));`
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པར་ཝི་ཨོར་སི་:༥༤༧ — I༡༨NI00000008X
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པེར་ཝི་ཨོར་:༦༠༢ — I༡༨NI00000009X
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པེར་ཝི་ཨོར་སི་:༦༥༦ — `let cargo = env::var_os("CARGO")`

## ཀར་གོ_བིན་_ཨི་ཨེགསི་_རི་རོ་ཧ་ (བརྟག་དཔྱད: ༢)

- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀིལི/བརྟག་དཔྱད་/ཀླི་_ས་མོ།:40 — `env!("CARGO_BIN_EXE_iroha")`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀིལི/བརྟག་དཔྱད་/ཏའི་ཀའི་_polic.s:20 — `env!("CARGO_BIN_EXE_iroha")`

## ཀར་གོ_བིན_ཨི་ཨེགསི་_རི་རོ་ཧ་_མོ་ཊར་ (བརྟག་དཔྱད་: ༤)

- བརྟག་དཔྱད།: ཀེརེས་/ཨི་རོ་ཧ་_མོ་ནི་ཊོར་/བརྟག་དཔྱད་/ཨེ་ཊཆ་_རེནཌར་.11 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_མོ་ནི་ཊོར་/བརྟག་དཔྱད་/ཨེཆ་ཊི་ཊི་པི་_ལི་མིཊ།:10 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_མོ་ནི་ཊོར་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ནུས་མེད་_ཀླད་ཀོར།:9 — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_མོ་ནི་ཊོར་/བརྟག་དཔྱད/ས་སྨུག་.༩ — `std::env::var_os("CARGO_BIN_EXE_iroha_monitor").map(PathBuf::from)`

## ཅར་གོ_བིན་_ཨི་ཨེགསི་_ཀ་ག་མི་ (བརྟག་དཔྱད: ༢)

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀ་ག་མི/བརྟག་དཔྱད་/སྤྱིར་བཏང་/མོ་ཌི་:༢༢ — I༡༨NI0000017X
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀ་ག་མི/བརྟག་དཔྱད་/པོཔ་_ཨེནབེཌ།:34 — `let status = Command::new(env!("CARGO_BIN_EXE_kagami"))`

## ཀར་གོ_བིན_ཨི་ཨེགསི་_ཀ་ག་མི་_མོཀ་ (བརྟག་དཔྱད: ༡)

- བརྟག་དཔྱད།: མོ་ཅི་/མོ་ཅི་-མཉམ་བསྡོམས་/བརྟག་དཔྱད་/བརྟག་དཔྱད་མཁན།:33 — `let kagami = env!("CARGO_BIN_EXE_kagami_mock");`

## ཅར་གོ_བིན_ཨི་ཨེགསི་_ཀོ་ཊོ་_བསྡུ་སྒྲིག་འབད་ (བརྟག་དཔྱད་: ༣)

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད/ཀླི་_སི་མོཀ.s:8 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད/ཀླི་_ས་མོ།
- བརྟག་དཔྱད།: ཀེརེཊསི/ཨཝ་ཨེམ་/བརྟག་དཔྱད/ཀླི་_སི་མོཀ.s:88 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`

## ཅར་གོ_བི་ཨའི་ཨེན་_ཨི་ཨེགསི་ཨི་_སོ་རཕ་_ཆུངཀ་_ཌམཔ་ (བརྟག་དཔྱད: ༡)

- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་སི་_ཆུན་ཀར་/བརྟག་དཔྱད་/གཅིག་_གིབ་.s:103 — `let chunk_dump_path = std::env::var("CARGO_BIN_EXE_sorafs_chunk_dump")`

## ཀར་གོ_བིན་_ཨི་ཨེགསི་ཨི་_སོ་རཕ་ས_ཀླི་ (བརྟག་དཔྱད: ༡)

- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད་/སོ་རཕ་_ཀི་ལི.s:42 — `let path = env::var("CARGO_BIN_EXE_sorafs_cli")`

## ཀར་གོ_བིན་_ཨི་ཨེགསི་_སོ་རཕ་_ཕེཆ་ (བརྟག་དཔྱད་: ༡)

- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་_ཀར་/སེརསི/སོ་རཕ་_ ཕེཆ་.2831 — `if let Ok(path) = env::var("CARGO_BIN_EXE_sorafs_fetch") {`

## ཀར་གོ_བིན་_EXE_taikai_car (test: 1)

- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད་/སོ་རཕ་_ཀི་ལི.s:48 — `let path = env::var("CARGO_BIN_EXE_taikai_car")`

## ཀར་གོ_བིན་_NAME (prod: 3).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིལི/སི་ཨར་སི/མའེན་_ཤེར་ཌི་.༦༣ — I༡༨NI00000027X དང་།
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིལི/སི་ཨར་སི/མའེན་_ཤེར་ཌི་.༧༢ — I༡༨NI0000028X དང་།
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧད་/སི་ཨར་སི་/མའེན་:༣༡༧༦ — I༡༨NI00000029X

## CARGO_BUILD_TARGET (ལག་ཆས་: ༢)

- ལག་ཆས་: xtask/src/poseidon_bench.s:88 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`
- ལག་ཆས་: xtask/src/stage1_bench.s:64 — `.unwrap_or_else(|_| std::env::var("CARGO_BUILD_TARGET").unwrap_or_default()),`

## CARGO_CFG_TARGET_ARCH (prod: 2, ལག་ཆས་: ༢)- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/ཨེསི་ཨར་སི/བིན་/ཨེསི་ཨེམ་_པར་ཕི་_ཅེག་.s:632 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/ཨེསི་ཨར་སི/བིན་/ཨེསི་ཨེམ་_པར་ཕི་_ཅེག་.s:668 — `let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| env::consts::ARCH.to_owned());`
- ལག་ཆས་: xtask/src/poseidon_bench.s:89 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`
- ལག་ཆས་: xtask/src/stage1_bench.s:65 — `arch: std::env::var("CARGO_CFG_TARGET_ARCH")`

## CARGO_CFG_TARGET_OS (བཟོ་བསྐྲུན་: ༡, prod: ༢, ལག་ཆས་: ༢)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཕཱསིཊི་པི་ཀིའུ་_པོརོ་ཝར་/བའིལཌི་.༢༧ — I༡༨NI00000036X
- པྲོཌ་: ཀེརེཊསི/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/ཨེསི་ཨར་སི/བིན་/ཨེསི་ཨེམ་_པར་ཕི་_ཅེག་.s:633 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/ཨེསི་ཨར་སི/བིན་/ཨེསི་ཨེམ་_པར་ཕི་_ཅེག་.s:669 — `let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.to_owned());`
- ལག་ཆས་: xtask/src/poseidon_bench.s:91 — `os: std::env::var("CARGO_CFG_TARGET_OS")`
- ལག་ཆས་: xtask/src/stage1_bench.s:67 — `os: std::env::var("CARGO_CFG_TARGET_OS")`

## ཅར་གོ_ཨེཕ་ཨེ་ཊི་ཡུ་ཨར་_CUDA (བསྐྲུན: ༢)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཕཱསིཊི་པི་q_prover/build.s:25 — `let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();`
- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཝི་ཨེམ་/བའིལཌ་.༡༢ — I༡༨NI00000042X .

## ཅར་གོ_ཨེཕ་ཊི་ཡུ་ཨར་_སི་ཡུ་ཌ་_ཀར་ནེལ་ (བཟོ་བསྐྲུན་: ༡)

- བཟོ་བསྐྲུན་: ཀེརེ་ཊི/ནོ་རི་ཊོ/མགྱོགས་ཚད་༡_ཀུ་ཌ་/བའིལཌི་.ཨར་ཨེསི་:༡༢ — I༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༤༣ཨེགསི་

## ཀཱར་གོ_ཨེཕ་ཊི་ཡུ་ཨར་_ཨེཕ་ཨེ་ཨེསི་པི་ཀིའུ་_ཇི་པི་ཡུ་ (བཟོ་བསྐྲུན་: ༡)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཕཱསཊི་པི་ཀིའུ་_པོརོ་ཝར་/བཱུལ་ཌི་ཨེསི་:༢༦ — `let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();`

## ཀཱར་གོ_ཨེཕ་ཊི་ཡུ་ཨར་_FFI_EXPORT (prod: 1)

- པྲོཌ་: ཀེརེཊསི/བཟོ་བསྐྲུན་-རྒྱབ་སྐྱོར་/ཨེསི་ཨར་སི/ལིབ་.རེ་:༣༠ — I༡༨NI00000045X

## ཀཱར་གོ_ཨེཕ་ཊི་ཡུ་ཨར་_ཨེཕ་ཨེཕ་ཨའི་_IMPORT (prod: 1).

- པྲོཌ་: ཀེརེཊསི/བཟོ་བསྐྲུན་-རྒྱབ་སྐྱོར་/ཨེསི་ཨར་སི/ལིབ་.29 — `let ffi_import = std::env::var_os("CARGO_FEATURE_FFI_IMPORT").is_some();`

## CARGO_MANIFEST_DIR (bench: 4, བཟོ་བསྐྲུན་: 5, དཔེན: 1, prod: 27, བརྟག་དཔྱད།: 164, ལག་ཆས་: 4).- པྲོཌ་: ཀེརེཊསི་/ཕཱསཊི་པི་q_པོརོ་ཝར་/ཨེསི་ཨར་སི/པོ་སི་ཌོན་_མན་ཕེསཊ།:༡༠ — I༡༨NI000000047X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་གཏམ།/བརྟག་དཔྱད་/ཐུམ་སྒྲིལ་.༡༧ — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་བཤུས་/བརྟག་དཔྱད་/པོ་སི་ཌོན་_མན་ཆད་_མཐུན་སྒྲིག.9 — `let metal_path = concat!(env!("CARGO_MANIFEST_DIR"), "/metal/kernels/poseidon2.metal");`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་བཤུས་/བརྟག་དཔྱད་/པོ་སི་ཌོན་_མན་ཆད་_མཐུན་སྒྲིག།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་རིས་/བརྟག་དཔྱད་/བདེན་དཔང་_ཕིག་ཅུར།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་གཏམ།/བརྟག་དཔྱད/ཊེསི་_ཀམ་ཊིམ་.14 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`
- བརྟག་དཔྱད: ཀེརཊི/ཕཱསཊི་པི་q_དཔེ་བཤུས་/བརྟག་དཔྱད/ཊེན་ཀིརིཔཀྲི་_རི་པལ.s:56 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་/སི་ཨར་སི/ཀླུང་།:8409 — `let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད: ཀྲེ་ཊི།
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་/བརྟག་དཔྱད་/sm_signing.s:35 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀིལི/ཀོཔ་ཨུ་ཊརས:༥༡༨ — I༡༨NI000000057X
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིལི/སི་ཨར་སི/མའེན་_ཤེར་ཌི་.༧༦༦ — I༡༨NI000000058X དང་།
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀིལི/བརྟག་དཔྱད་/ཀླི་_ས་མོ།:111 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀིལི/བརྟག་དཔྱད་/ཀླི་_ས་མོ།:5236 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- པྲོཌ་: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོན་སི་/པེ་ར་མི་ཊར་/ཡུ་ཨེསི་ཨར་.3490 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོན་སི་/ཚད་མ/ཚད་མདངས།:11561 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོན་ཊི་/བརྟག་དཔྱད་/ཕཱསིཊི་པི་q_ཀིའུ་_ཨོ་ཝར་རིཌསི་.༡༥ — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོན་ཊི་/བརྟག་དཔྱད་/ཕིགསི་ཅར་:༣༦ — `std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))`
- བེནཆ་: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/བེན་ཆི།
- བེནཆ་: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/བེན་ཆི།/བཀག་ཆ།/སྤྱིར་བཏང་/མོ་ཌི་:༢༧༢ — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- བེནཆ་: ཀྲེ་ཊི།
- བཟོ་བསྐྲུན་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/བའིལཌ་.19 — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;`
- དཔེར་ན་: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/དཔེ་/སྤྱིར་བཏང་_མཉམ་ཆ་_ཕིག་ཅར་ཚུ་།:༡༧ — I༡༨NI000000069X
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/Executer.s:2385 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/Executor.s:2521 — `let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/Executor.s:2661 — `std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/ཨེསི་ཨར་སི་/ཨོཕ་ལའིན་.2152 — `env!("CARGO_MANIFEST_DIR"),`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/ཨེསི་ཨར་སི་/ཨེསི་མཱརཊ་གན་ཡིག་/ཨའི་/ཨོཕ་ལའིན་.
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/ཨེསི་ཨར་སི/རིག་རྩལ་/ཨོཕ་ལའིན་.
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/ཨེསི་ཨར་སི་/ཨོཕ་ལའིན་.
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/smart གན་རྒྱ།/isi/offline.s:4852 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/smart གན་རྒྱ།/isi/repo.s:1870 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/smart གན་རྒྱ།/isi/repo.s:1874 — `env!("CARGO_MANIFEST_DIR"),`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/སི་ཨར་སི་/སི་ཊེཊ་:༩༩༩༠ — I༡༨NI00000080X
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/state.s:12012 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/streaming.2980 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/tx.s:4424 — `let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/ལག་ལེན་པ་_གནས་སྤོ་མི་_ཨིན་ཊོ་ཨིསི་པེག་ཊི་.ཨར་སི་:༢༥ — `let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/པིན་_ཐོ་བཀོད་.༡༡༥༨ — I༡༨NI00000085X
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/པར་ཆས་.༢༩ — I༡༨NI0000086X
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད/སུ་མར་གྷི་_ཌོག་_སིན་ཀ་.༥༧ — I༡༨NI000000087X- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/བརྟག་དཔྱད་/བདེན་ཁུངས་_ཀི་སེཊི་_ཝེག་ཊར་.༥༧ — `let cargo = env::var_os("CARGO")`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ/བརྟག་དཔྱད་/sm2_fixture_vectors:50 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ/བརྟག་དཔྱད་/sm_cli_matrix.s:19 — `env!("CARGO_MANIFEST_DIR"),`
- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/བཱུལ་ཌི་ཨེསི་:༡༡ — `let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/src/lib.s:186 — `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/src/lib.s:190 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/src/ཨོཕ་ལའིན་/པོ་སེ་ཌོན་.༤༤༩ — I༡༨NI000000094X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/སོ་རེ་ནེཊ་/སོ་རཱན་ཊི་/vpn.s:1217 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/རྩིས་ཁྲ་_ཁ་བྱང་_ཝེག་ཊར་.༡༤༧ — I༡༨NI00000000096X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/བརྟག་དཔྱད་/བདེན་ཁུངས་_གྱང་_ ཕིག་ཅར་ ༡༥ — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/བརྟག་དཔྱད་/ཀོན་སེན་ཌི་ཊིཔ་.1308 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/བརྟག་དཔྱད་/ཨོཕ་ལའིན་_ཕིགསི་ཅར།:130 — `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ ཕིག་ཅར་ ༢༥ — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ཕིག་ཅར་ཚུ་:༢༩ — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེས་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ཕིག་ཅུརསི།:33 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ཕིག་ཅུརསི།:37 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ ཕིག་ཅར་ཚུ་:༤༡ — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ཕིག་ཅར་ཚུ་:༤༦ — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེ་ཀལ་_གཞི་བསྟུན་_ཕིག་ཅུརསི།:50 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེ་ཀལ་_གཞི་བསྟུན་_ཕིག་ཅུརསི།:54 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ཕིག་ཅུརསི།:58 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེས་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ཕིག་ཅུརསི།:62 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/ཨོ་རེཀ་_གཞི་བསྟུན་_ཕིག་ཅུརསི།:༢༠༠ — `let base = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད/རན་ཊའིམ་_ཌོག་_སིན་ཀ་.s:8 — `let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི/ཨི་རོ་ཧ་རིགས་མཚན་/src/lib.s:1173 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་-རིགས་མཚན་/src/lib.s:3847 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་-རིགས་མཚན་/src/lib.s:4228 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- བརྟག་དཔྱད།: ཀེརེཊསི/ཨི་རོ་ཧ་རིགས་མཚན་/src/lib.4243 — `std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཨའི་༡༨ན/སེརསི/ལིབ་.༤༩༥ — I༡༨NI00000118X
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཇི་ས_ཧོསིཊ/སི་ཨར་སི/ལིབ་.༦༩༤༣ — I༡༨NI00000119X
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀ་ག་མི།/དཔེ་ཚད་/ཀོ་ཌེག་/འབྱུང་ཁུངས་.s:13 — `let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀ་ག་མི།/དཔེ་ཚད་/ཀོ་ཌེག་/སི་ཨར་སི་/མ་ནིན་:༣༥ — I༡༨NI0000000121X
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀ་ག་མི།/src/codec.s:393 — `env!("CARGO_MANIFEST_DIR"),`
- པྲོཌ་: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀ་ག་མི།/src/localnet.s:604 — `let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀ་ག་མི/བརྟག་དཔྱད་/ཀོཌི་སི།:11 — `const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཊེ་ལི་མི་ཊི་རི་/བརྟག་དཔྱད་/དྲིལ་_ལོག་.10 — `let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_དྲ་རྒྱ་/src/config.s:724 — `let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/fslock_ports.s.23 — `const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/fslock_ports.s.25 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:281 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_དཔེ་ཚད་/src/lib.s:204 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_དཔེ་ཚད་/src/lib.s:241 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི/ཌ/ཊེ/ཊེག་སི།:3490 — `let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི/སོ་རཕ་/པི་.ཨར་སི་:༤༠༥༣ — I༡༨NI00000133X
- བརྟག་དཔྱད།: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཊོ་རི/བརྟག་དཔྱད་/རྩིས་ཁྲ་_ཁ་བྱང་_ཝེག་ཊར་.༡༤༠ — `let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི།/བརྟག་དཔྱད་/རྩིས་ཐོ་_པོརཊ་ཕོ་ལིའོ་.༩༡ — I༡༨NI00000135X
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད་/སོ་རཕ་ས_ཌིསི་གསར་འཚོལ་.༡༠༥༦ — I༡༨NI00000136X
- བཟོ་བསྐྲུན་: ཀྲེ་ཊི།
- པྲོཌ་: ཀེརེ་ཊི/ཨེ་ཨེམ་/ཨེསི་ཨར་སི/བི/ཇེན་_བི_ཧཤ་_ཌོག་.rs:༢༨ — I༡༨NI000000138X
- པྲོཌ་: ཀེརེ་ཊི/ཨེ་ཨེམ་/ཨེསི་ཨར་སི/བི/ཇེན་_མགོ་ཡིག་_ཌོག་.༤༨ — I༡༨NI00000139X
- པྲོཌ་: ཀེརེ་ཊི/ཨཝ་ཨེམ་/ཨེསི་ཨར་སི/བི/ཇེན་_པོའིནཊར་_དབྱེ་བ་_ཌོག་.rs:༢༣ — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- པྲོཌ་: ཀེརེ་ཊི།/ཝི་ཨེམ་/ཨེསི་ཨར་སི/བི/ཇེན་_སི་ཀཱལསི་_ཌོག་.rs:24 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- པྲོཌ་: ཀེརེ་ཊི/ཨཝ་ཨེམ་/ཨེསི་ཨར་སི/བིན་/ཝི་ཨེམ་_པི་རི་བཱལཌ་.༡༦ — I༡༨NI0000142X
- པྲོཌ་: ཀེརེཊསི་/ཝི་ཨེམ་/ཨེསི་ཨར་སི/བིན་/ཝི་ཨེམ་_སྔོན་འགྲོའི་ཀོ་ཌར་_ཕྱིར་འདྲེན་.ཨར་སི་:༢༣ — I༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༡༤༣ཨེགསི་
- པྲོཌ་: ཀེརེ་ཊི/ཨཝ་ཨེམ་/ཨེསི་ཨར་སི/སྔོན་འགྲོའི་ཌི་ཀོ་ཌར་_ཕིག་ཅར་སི།
- བརྟག་དཔྱད།: ཀེརེ་ཊི/ཨཝ་ཨེམ་/བརྟག་དཔྱད/ཀླི་_སི་མོཀ.9 — `let manifest_dir = env!("CARGO_MANIFEST_DIR");`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད/ཀླི་_སི་མོཀ།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད/ཀླི་_ས་མོ།
- བརྟག་དཔྱད།: ཀེརེ་ཊི/ཨཝ་ཨེམ་/བརྟག་དཔྱད།/ཡིག་ཆ་_མཐུན་སྒྲིག
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།/ནོ་རི་ཊོ་_པོར་ཊལ་_སི་ནིཔ་ཊི་_བསྡུ་སྒྲིག་འབད་ནི་.༡༩ — `let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/དཔག་བྱེད་_དབྱེ་བ་_ཌོག་_འབྱུང་ཁུངས་.༧ — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ/བརྟག་དཔྱད_ཌོག་_ ཌོག་_བཟོ་བཀོད།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད་/སི་ཀཱལསི་_ཌོག་_འབྱུང་ཁུངས་.༧ — `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད་/སི་ཀཱལསི་_ཌོག་_སིན་ཀ་.༨ — I༡༨NI00000155X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད་/སི་ཀཱལསི་_གཱསི་_མིང་།
- bench: ཀྲེ་ཊིས་/ནོ་རི་ཊོ/བེན་ཆི་/མཉམ་བསྡོམས་_ཀམ་པ་ཡར་.༧༩ — `let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))`
- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ནོ་རི་ཊོ་/བའིལཌ་.17 — `PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));`
- པྲོཌ་: ཀྲེ་ཊི།/ནོ་རི་ཊོ།/src/bin/norito_regen_goldens.s:9 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ནོ་རི་ཊོ/བརྟག་དཔྱད/ཨཱོསི་_ཨེན་སི་བྷི་_གཱོལ་ཌེན་.rs:200 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ནོ་རི་ཊོ/བརྟག་དཔྱད།/ཇསོ་_གོ་ཌེན་_loader.s:14 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ/བརྟག་དཔྱད་/ncb_enum_iter_samples.353 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ/བརྟག་དཔྱད་/ncb_enum_iter_samples.387 — `let path = Path::new(env!("CARGO_MANIFEST_DIR"))`.
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ/བརྟག་དཔྱད་/ncb_enum_iter_samples.554 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ/བརྟག་དཔྱད་/ncb_enum_iter_samples.665 — `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ/བརྟག་དཔྱད་/ncb_enum_large_fixture.37 — `let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);`
- བརྟག་དཔྱད།: ཀེརེ་ཊི/སོ་རཕ་_ཀར་/སེརསི/བིན་/ཌ_བསྐྱར་བཟོ་.rs:434 — `let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/སེརསི/བིན་/ཌ_བསྐྱར་བཟོ.rs:710 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- པྲོཌ་: ཀེརེ་ཊི/སོ་རཕ་ས_ཀར་/ཨེསི་ཨར་/སོ་རོ་ནེན་_བློ་གཏད་མེད་པའི་_བདེན་དཔྱད་པ་.༡༤༠ — I༡༨NI000000169X- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད་/ནུས་སྟོབས་_ཚད་འཇུག_ལག་ཆས་ཀིཊི།:༩ — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད/ཕེཊ་ཆི་_ཀི་ལི་.s:50 — I༡༨NI00000171X
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད/ཕེཊ་ཆི་_ཀི་ལི.s:1043 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད/ཕེཆ་_ཀི་ལི་.1159 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད།/ཏའི་ཀའི་_ཀར་_ཀི་ལི.s:227 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད་/ཊེའི་ཀའི་_བལྟ་མི་_ཀི་ལི.s:22 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད་/བདེན་མེད་_ཝར་ཕིར་ཕིར་སི།:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- པྲོཌ་: ཀེརེཊསི་/སོ་རཕ་_ཆུར་/སིཨར་སི/བིན་/ཕྱིར་འདྲེན་_ཝེག་ཊར་.༡༧༥ — I༡༨NI000000177X
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་_ཆུན་ཀར་/བརྟག་དཔྱད/རྒྱབ་བསྐྱོད་.༨ — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་_ཆུན་ཀར་/བརྟག་དཔྱད་/ཝེག་ཊར་.༡༢ — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_མནར་གཅོད་/བརྟག་དཔྱད་/པོར_ཕིགསི་ཅར། 11 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་སི་_མན་ཆད་/བརྟག་དཔྱད་/མཁོ་སྤྲོད་_འཛུལ་ཞུགས་_ཕིག་ཅུ། 12 — `cmd.current_dir(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་སི་_མནར་གཅོད་/བརྟག་དཔྱད་/འདྲ་དཔེ་_བཀའ་རྒྱ་_ཕིག་ཅུ།:8 — `env!("CARGO_MANIFEST_DIR"),`
- པྲོཌ་: ཀེརེཊསི་/སོ་རཕ་ས_ནོཌི་/ཨེསི་ཨར་སི/སོ་རཕ་ས_གེ་ཊི་_གེ་ཊི་ཝེ་.s:55 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- པྲོཌ་: ཀེརེཊསི་/སོ་རཕ་ས_ནོཌི་/ཨེསི་ཨར་སི/སོ་རཕ་_གེ་ཊི་_གེ་ཊི་ཝེ་.s:༥༩ — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sorafs_gateway/1.0.0")`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ནོཌི་/སི་ཨར་སི/གཱེཊ་ལམ་.༢༠༠༦ — I༡༨NI00000185X
- བརྟག་དཔྱད།: ཀྲེ་ཊི།
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ནོཌི་/བརྟག་དཔྱད།/སྒོ་ལམ།: 14 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ནོཌི་/བརྟག་དཔྱད།/སྒོ་ལམ།:༣༠ — I༡༨NI00000188X
- བརྟག་དཔྱད།: ཀེརེཊསི།
- བརྟག་དཔྱད།: ཀེརེཊསི/སོ་རཕ་ས_ཨོར་ཀེཔ་ཊར/ཨེསི་ཨར་སི/ལིབ་.༦༤༤༩ — I༡༨NI00000190X
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཨོར་ཀེཔ་ཊར/ཨེསི་ཨར་སི/ལིབ་.༨༡༠༠ — I༡༨NI00000191X
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་ར་ཕིས་_ཨོར་ཀེསི་ཊི་ཊོར་/བརྟག་དཔྱད་/ཨར་ཀེསི་ཊར་_མཉམ་ཆ་:༡༨༠ — I༡༨ཨེན་ཨའི་༠༠༠༠༡༩༢ཨེགསི་
- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རེན་ནེཊ་_པི་ཀིའུ་/བརྟག་དཔྱད་/ཀཊ་_ཝེག་ཊར་.༩ — I༡༨NI0000193X
- བཟོ་བསྐྲུན་: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བཟོས་པ།:17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/src/sorafs_gateway_ལྕོགས་གྲུབ_བཀག་ཆ་.157 — `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/sorafs_gateway/capability_refusal")`
- བརྟག་དཔྱད་: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/src/sorafs_gateway_conformance.1032 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད། མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་_canonicisation.126 — `let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/རྒྱུ་ཆ།:49 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/མགྱོགས་དྲགས་_dsl_build.7 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/རིགས་མཚན་_json.s:16 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/རིགས་མཚན་_json.s:22 — `let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཨི་རོ་ཧ་_ཀི་ལི་.s:24 — `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/མགོ་ཡིག་_ཌི་ཀོཌ་.༤༩ — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/མགོ་མགོ་_སི་མོཀ། 24 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པལ་.༧༠ — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པལ་ཚུ་:123 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་སྡེབ་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པེལསི།:173 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཤུལ་མམ་/ནེག་སི།/སི་བི་ཌི་སི་_བོལ་ཨལཊི་_བཱན་ཌལ་.s:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཤུལ་མམ་/སི་བི་ཌི་སི་_དཀར་པོ།:26 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ནེགསི་/འཛམ་གླིང་_commit.17 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཤུལ་མམ་/ལམ་ཐིག་_ཐོ་བཀོད་.༡༢ — `let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN").map_err(|_| {`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ནོ་རི་ཊོ་_བཱརན་_ཕིག་ཅུར།
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/རེ་པོ་.31 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/རྒྱུན་ལམ་/མོ་ཌི་:339 — `let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པེར་ཝི་ཨོར་སི་:༢༢༠ — I༡༨NI00000215X
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པར་ཝི་ཨོར་སི་:༥༣༨ — I༡༨NI00000216X
- བརྟག་དཔྱད།: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི/ཊོ་རི་.༤༧༤༠ — I༡༨NI00000217X
- བརྟག་དཔྱད།: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི/ཊོ་རི་.4755 — `let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- བརྟག་དཔྱད།: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི/ཊོ་རི་.༤༧༧༠ — I༡༨NI00000219X
- བརྟག་དཔྱད།: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/ཊོ་རི་.༤༧༨༥ — I༡༨NI00000220X
- བརྟག་དཔྱད།: མོ་ཅི་/མོ་ཅི་-མཉམ་བསྡོམས་/བརྟག་དཔྱད་/བརྟག་དཔྱད་མཁན།:168 — `let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");`
- བརྟག་དཔྱད།: ལག་ཆས་/སོ་ནེན་-ལག་ཤེཀ་-ཧར་ནིས/བརྟག་དཔྱད་/ཕིགསི་ཅར་_བདེན་དཔྱད་.rs:6 — `let bin = env!("CARGO_BIN_EXE_koto_compile");`
- བརྟག་དཔྱད།: ལག་ཆས་/སོ་རེན་ནེཊ་-ལག་ཤེཀ་-ཧར་ནིས/བརྟག་དཔྱད/བརྟག་དཔྱད་/ཚོད་ལྟའི་_མཉམ་ཆ་:༧༧ — I༡༨NI00000223X
- བརྟག་དཔྱད།: ལག་ཆས་/སོ་རེན་ནེཊ་-ལག་ཤེཀ་-ཧར་ནེསི་/བརྟག་དཔྱད་/པར་f_gate.s:173 — `let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));`
- ལག་ཆས་: xtask/src/bin/canty_mock.s:362 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- ལག་ཆས་: xtask/src/main:11383 — `Path::new(env!("CARGO_MANIFEST_DIR"))`
- ལག་ཆས་: xtask/src/sorafs/gateway_fixture.s:28 — `env!("CARGO_MANIFEST_DIR"),`
- ལག་ཆས་: xtask/src/sorafs/gateway_fixture.s:32 — `env!("CARGO_MANIFEST_DIR"),`
- བརྟག་དཔྱད།: xtask/test/address_vectors.7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/android_dashboard_parity_cli.s:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/tec/codec_rans_tables.18 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`.
- བརྟག་དཔྱད།: xtask/tests/da_proof_bench.s:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/is_bridg_lint.s:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/བརྟག་དཔྱད་/ལྷན་ཁག་_Agnda.s:7 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/sns_catalo_བདེན་དཔྱད་.༥ — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soradns_cli.s:11 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/sorafs_ftch_fixture.9 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_bug_bounty.s:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_gateway_billing.s:12 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_gateway_billing_m0.s:30 — `let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_gateway_m1.s:11 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_gateway_m2.s:25 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_pop_template.s:10 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_pop_template.s:77 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_pop_template.131 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_pop_template.s:202 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_pop_template.s:306 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_pop_template.s:356 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/test/soranet_pop_template.s:489 — `let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/tests/stuing_bundle_check.s:9 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`
- བརྟག་དཔྱད།: xtask/tests/streaming_entropy_bench.s:8 — `PathBuf::from(env!("CARGO_MANIFEST_DIR"))`

## CARGO_PKG_VERSION (prod: 11, བརྟག་དཔྱད: ༡, ལག་ཆས་: ༡)- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་/སི་ཨར་སི་/ཀིལིནཊ་:༤༧༨ — I༡༨NI00000252X
- པྲོཌ་: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀིལི/སི་ཨར་སི་/བརྡ་བཀོད་/སོ་རཕ་སི་.༡༧༦༧ — `metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིལི/སི་ཨར་སི/མའེན་_ཤེར་ཌི་.༧༢ — I༡༨NI0000254X
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིལི/སི་ཨར་སི/མའེན་_ཤེར་ཌི་.༥༣༠ — I༡༨NI00000255X དང་།
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀིལི/བརྟག་དཔྱད་/ཀླི་_ས་མོ།:༣༥༩ — I༡༨NI0000256X
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/སི་ཨར་སི/ར་བི་སི་_སི་ཊོར་.༤༠ — I༡༨NI0000000257X
- པྲོཌ་: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཇི་ས_ཧོསཊི་/སི་ཨར་སི/ལིབ་.༢༩༩༨ — I༡༨NI00000258X
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཊེ་ལི་མི་ཊི་རི་/ཨེསི་ཨར་སི་/ཝསི་.༢༤༣ — I༡༨NI0000259X
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧད་/སི་ཨར་སི་/མའེན་:༥༠༢ — I༡༨NI00000260X
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧད་/སི་ཨར་སི/མའེན་:༣༩༨༦ — I༡༨NI00000261X
- པྲོཌ་: ཀེརེ་ཊི/སོ་རཕ་ས_ཀར་/ཨེསི་ཨར་/སོ་རཕ་_ ཕེཆ་.༡༡༡༡ — I༡༨NI0000000262X
- པྲོཌ་: ཀེརེ་ཊི/སོརཕ་སི་_ཨོར་ཀེཊ་ཊར་/ཨེསི་ཨར་སི/བིན་/སོ་རཕ་ས_ཀླི་.s:93 — I༡༨NI0000000263X:༩༣ —
- ལག་ཆས་: ལག་ཆས་/ལྕགས་རིགས་-ལས་འཆར་གྱི་ཁྱད་པར་/src/main.s:248 — `tool_version: format!("telemetry_schema_diff {}", env!("CARGO_PKG_VERSION")),`

## ཅར་གོ།

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/སོ་རེན་ནེཊ་_པི་ཀིའུ་/བཱལཌ་.s:6 — `if std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some() {`

## CARGO_TARGET_DIR (prod: 3, བརྟག་དཔྱད: ༢, ལག་ཆས་: ༡)

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:524 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:759 — `if let Ok(path) = std::env::var("CARGO_TARGET_DIR") {`
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པེར་ཝི་ཨོར་སི་:༥༧༥ — I༡༨NI00000268X
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པེར་ཝི་ཨོར་སི་:༦༢༩ — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པེར་ཝི་ཨོར་སི་:༦༨༣ — `let target_root = env::var_os("CARGO_TARGET_DIR")`
- ལག་ཆས་: xtask/src/mochi.s:383 — `if let Ok(dir) = env::var("CARGO_TARGET_DIR") {`

## CARGO_WORKSPACE_DIR (test: 1).

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/སི་ཨར་སི/མངའ་སྡེའི་.༡༢༠༡༥ — I༡༨NI0000272X

## CRYPTO_SM_INTRINSICS (bench: 1).

- བེནཆ་: ཀྲེ་ཊི།

## CUDA_HOME (སྒྲིང་ཁྱིམ་: ༢)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཕཱསིཊི་པི་q_prover/build.s:198 — `env::var_os("CUDA_HOME")`
- བཟོ་བསྐྲུན་: ཀེརེ་ཊི/ནོ་རི་ཊོ/ཨེག་ལེ་ཊི་ཊར་/ཇེ་སོན་སི་ཊེཇ་༡_ཀུ་ཌ་/བའིལཌི་.s:༦༣ — I༡༨ཨེན་ཨའི་༠༠༠༠༠༢༧༥ཨེགསི་།

## CUDA_PATH (སྒྲིང་ཁྱིམ་: ༢)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཕཱསིཊི་པི་ཀིའུ་_པོརོ་ཝར་/བཱུལ་ཌི་ཨེསི་:༡༩༩ — `.or_else(|| env::var_os("CUDA_PATH"))`
- བཟོ་བསྐྲུན་: ཀེརེ་ཊི/ནོ་རི་ཊོ/མགྱོགས་ཚད་༡_ཀུ་ཌ་/བའིལཌི་.s:༦༤ — I༡༨NI000000277X

## ཌེ་ཊ་ཨེ་པསི་_ADVERSARIFACT_DIR (test: 1).

- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཤུལ་མམ་/ཀོརོསི་_ལམ་.686 — `if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {`

## DOCS_RS (སྒྲིང་ཁྱིམ་: ༡)།

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ནོ་རི་ཊོ་/བཱུལ་ཌི་.rs:8 — `if env::var_os("DOCS_RS").is_some() {`

## ENUM_BENCH_N (bench: 1).

- bench: ཀྲེ་ཊིས་/ནོ་རི་ཊོ/བེན་ཆི་/ཨེ་ནམ་_བེན་ཇི་.༧༥ — `let n: usize = std::env::var("ENUM_BENCH_N")`

## FASTPQ_DEBUG_FUSED (prod: 1).

- པྲོཌ་: ཀེརེཊསི་/ཕཱསཊི་པི་ཀིའུ་_པོརོ་ཝར་/ཨེསི་ཨར་སི/ཊེསི་.༩༨ — I༡༨NI0000281X

## FASTPQ_EXPECTED_KIB (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་གཏམ།/བརྟག་དཔྱད་/པར་_ཐོན་སྐྱེད་.༨༨ — `env::var("FASTPQ_EXPECTED_KIB")`

## FASTPQ_EXPECTED_MS (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་རིས་/བརྟག་དཔྱད་/པར་_ཐོན་སྐྱེད་.༨༠ — `env::var("FASTPQ_EXPECTED_MS")`

## FASTPQ_PROOF_ROWS (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་གཏམ།/བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་།:72 — `env::var("FASTPQ_PROOF_ROWS")`

## FASTPQ_SKIP_GPU_BUILD (བཟོ་བསྐྲུན་: ༡)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཕཱསིཊི་པི་ཀིའུ་_པོརོ་ཝར་/བའིལཌ་.45 — `if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {`

## FASTPQ_UPDATE_FIXTURES (test: 6).- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་རིས་/བརྟག་དཔྱད་/རྒྱབ་རྟེན་_འགྱུར་ལྡོག་.47 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་གཏམ།/བརྟག་དཔྱད་/རྒྱབ་རྟེན་_འགྱུར་ལྡོག་.69 — `if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་གཏམ།/བརྟག་དཔྱད/བདེན་དཔང་_ཕིག་ཅུར.43 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་གཏམ།/བརྟག་དཔྱད/ཊེསི་_ཀམ་མིཊ།:༢༣ — I༡༨NI0000289X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཕཱསིཊི་པི་q_དཔེ་གཏམ།/བརྟག་དཔྱད/ཊེསི་_ཀམ་ཊིམ་.111 — `let update = std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok();`
- བརྟག་དཔྱད: ཀེརཊི/ཕཱསཊི་པི་q_དཔེ་བཤུས་/བརྟག་དཔྱད/ཊེན་ཀིརིཔཀྲི་_རི་པལ.s:67 — `if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {`

## GENESIS_DEBUG_MODE (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_དྲ་རྒྱ་/དཔེར་ན/རིགས་མཚན་_ཌི་བུག་.rs:15 — `if let Ok(mode) = std::env::var("GENESIS_DEBUG_MODE") {`

## GENESIS_DEBUG_PAYLOAD (བརྟག་དཔྱད: ༡)

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/དཔེར་ན/རིགས་མཚན་_ཌི་བུག་.rs:122 — `let payload = std::env::var("GENESIS_DEBUG_PAYLOAD")`

## གིཐུ་བ_STEP_SUMARY (prod: 2).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/ཨེསི་ཨར་སི/བིན་/གོས་ཊི་_པར་ཕི་_ཅེག་.s:22 — `let summary_target = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from);`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/ཨེསི་ཨར་སི/བིན་/ཨེསི་ཨེམ་_པར་ཕི་_ཅེག་.ཨར་སི་:༢༠༥ — I༡༨ཨེན་ཨའི་༠༠༠༠༢༩༥ཨེགསི་

## གི་IT_COMMIT_HASH (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/སི་ཨར་སི་/སུམ་ར་གི་/རྦ་སི་_སི་ཊོར་.༤༢ — I༡༨NI000000296X

## ཁྱིམ་ (prod: 1).

- པྲོཌ་: ཀྲེ་ཊི།

## IROHA_ALLOW_NET (test: 1).

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་ཟ་ན་མི།/src/ཆའོ་སི་.༣༧༤ — I༡༨NI00000298X

## IROHA_CONF_GAS_SEED (test: 1).

- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_དཔེ་ཚད་/src/lib.s:57 — `std::env::var("IROHA_CONF_GAS_SEED").ok()`

## IROHA_DA_SPOOL_DIR (test: 1).

- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/state.rs:9096 — `std::env::var_os("IROHA_DA_SPOOL_DIR").map(std::path::PathBuf::from)`

## IROHA_METRICS_PANIC_ON_DUPLICATE (བརྟག་དཔྱད་: ༢)

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཊེ་ལི་མི་ཊི་རི/སི་ཨར་སི་/མེ་ཊིགས། 11397 — `std::env::var("IROHA_METRICS_PANIC_ON_DUPLICATE")`.
- བརྟག་དཔྱད།: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད་/མེ་ཊིགསི་_ཐོ་བཀོད་.༣༣ — I༡༨NI00000302X

## IROHA_RUN_IGNORD (test: 71)- བརྟག་དཔྱད།: ཀེརེས་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_ato_close_prove.s:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད/gov_finalize_real_vk.s:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_min_duration.18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_mode_mismatch.s:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_mode_mismatch_zk.rs:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_fonle_ballots:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_fonl_ Conviction.20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད/gov_lifle_disable.s:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད/gov_lifle_missing_ref.s:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད/gov_lifle_monotonic.s:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_protected_gate.s:64 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད།
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_thresholds.20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_thresholds.82 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_thresholds_positive.20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_unlock_sweep.s:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_zk_ballot_lock_བདེན་དཔྱད་.8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_zk_ballot_real_vk.s:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/gov_zk_referendum_window_guard.s:16 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/ཛཀ་_རོ་ཊི་_གེ་ཊི་_ཀེཊ་_ཀེཔ་.s:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/ཛཀ་_ཝོ་ཊི་_གེཊ་_ཊ་ལི་.29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/སི་ཨར་སི/མར་ཀེལ་:༡༣༡༧ — I༡༨NI0000324X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/སེརསི/མར་ཀེལ་:༡༣༣༢ — I༡༨NI00000325X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/བརྟག་དཔྱད་/མར་ཀི་ལི་_ནོ་རི་ཊོ་_སྒོར་སྒོར་རིཔ་.༡༨ — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/བརྟག་དཔྱད་/མར་ཀི་ལི་_ནོ་རི་ཊོ་_སྒོར་སྒོརམ་ཊིཔ་.s:42 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/བཀག་ཆ་/མགོ་ཡིག་།:624 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/སི་ཨར་སི་/སི/མོ་ཌི་:༡༨༨༨ — I༡༨ཨེན་ཨའི་༠༠༠༠༠༣༢༩ཨེགསི་
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/ཨེསི་ཨར་སི/ཐོ་བཀོད་པ་.324 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/src/proof.s:798 — `std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/ཨེསི་ཨར་སི/བརྒྱུད་འཕྲིན་/རྟགས་བཀོད།:1048 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཐོ་བཀོད་_ཐོ་བཀོད་_ལཱ་ཟི་_ཨིནཊི་.s:8 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད: ཀེརེས་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཐོ་བཀོད་_ཐོ་བཀོད་_reset.s:7 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/མོ་ཌེལ་_ཌི་རི་ཝའི་_རི་པྲོ་.s:15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/མོ་ཌེལ་_ཌི་རི་ཝི་_རི་པྲོ་.37 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/མོ་ཌེལ་_ཌི་རའིབ་_རེ་པོར.s:58 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ/བརྟག་དཔྱད་/མོ་ཌེལ་_ཌི་རི་ཝའི་_རི་པྲོ་.s:84 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/བརྟག་དཔྱད་/ཐོ་བཀོད་_ཌི་ཀོཌི་_སྒོར་སྒོརཊིཔ་.s:9 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེས་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/བརྟག་དཔྱད་/རང་གཤིས་_དངོས་པོ་.༧ — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/བརྟག་དཔྱད་/རང་གཤིས་_དངོས་པོ་.༢༧ — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`- བརྟག་དཔྱད: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཛཀ་_ཨེ་སིལ་པི་_སྒོར་སྒོར:༦ — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཊོ་རི/བརྟག་དཔྱད་/ཁ་འབག་_ཤུགས་ལྡན་_མཉམ་བསྡོམས་.rs:23 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད་/ཁ་འབག་_ཤུགས་ལྡན་_མཉམ་བསྡོམས་.174 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི/བརྟག་དཔྱད་/ཁ་འབག་།
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད་/ཁ་འབག་།
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི/བརྟག་དཔྱད་/ཁ་འབག་_གནས་སྟངས།
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི/བརྟག་དཔྱད་/ཁ་འབག་_གནས་སྟངས།_ཐོ་ཡིག་_རའུ་ཊར་.17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི།/བརྟག་དཔྱད་/gov_councul_persist_མཉམ་བསྡོམས་.22 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི།/བརྟག་དཔྱད་/gov_councul_vrf.s:18 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི།/བརྟག་དཔྱད་/gov_enact_handler.12 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད་/gov_instans_list.s:14 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེས་/ཨི་རོ་ཧ་_ཊོ་རི/བརྟག་དཔྱད་/gov_mode_mismatch_and_autoclose.42 — `if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི།/བརྟག་དཔྱད་/gov_protected_endpoints.15 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེས་/ཨི་རོ་ཧ་_ཊོ་རི/བརྟག་དཔྱད་/gov_protected_endpoints_router.s:20 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ/བརྟག་དཔྱད/ཛཀ་_རུཊ་_དང་_ཝོ་ཊི་_སི་ཀཱལསི་.༡༦ — I༡༨NI0000000359X.
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད/ཛཀ་_རུཊ་_དང་_ཝོ་ཊི་_སི་ཀཱལ་སི་.༥༠ — I༡༨NI000000360X.
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/ལས་འཛིན།/བརྡ་ཆར།: 81 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཕྱིར་འཐེན།
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཕྱིར་འཐེན།
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཕྱིར་འཐེན།
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཕྱིར་འཐེན།
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཕྱིར་འཐེན།
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཆོག་མཆན།:202 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`.
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཆོག་མཆན།:265 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད། མཉམ་སྡེབ་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཆོག་མཆན།:328 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/བཀག་ཆ་_བཀག་ཆ་འབད་ཡོདཔ།:17 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/དབྱེ་བ།:29 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/འགྲུལ་བསྐྱོད་པ་/by_call_trigger.139 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/འགྲུལ་བཞུད་པ་/དུས་ཚོད་_ཊི་རི་གར་.149 — `if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {`

## IROHA_RUN_ZK_WRPPERS (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི/ཨཝ་ཨེམ་/བརྟག་དཔྱད/ཀོ་ཊོ་ཌ་མ་_ཝ་ར་པར་སི་.༢ — `std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")`.

## IROHA_SKIP_BIND_CHECKS (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:3098 — `if std::env::var_os("IROHA_SKIP_BIND_CHECKS").is_none() {`

## IROHA_SM_CLI (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ/བརྟག་དཔྱད་/sm_cli_matrix.s:47 — `let configured = env::var("IROHA_SM_CLI").ok().map(|value| {`

## IROHA_TEST_DUMP_GENESIS (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:6661 — `if let Ok(dump_path) = env::var("IROHA_TEST_DUMP_GENESIS") {`## IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR (བཟོ་བསྐྲུན་: བརྟག་དཔྱད་: ༡)

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/config.s:155 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`
- བཟོ་བསྐྲུན་: མཉམ་བསྡོམ་_བརྟག་དཔྱད་/བཟོས་པ།:205 — `if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")`

## IROHA_TEST_SKIP_BUILD (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:1244 — `std::env::var("IROHA_TEST_SKIP_BUILD")`

## IROHA_TEST_TARGET_DIR (test: 2)

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_དྲ་རྒྱ་/src/lib.s:521 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:765 — `if let Ok(path) = std::env::var(IROHA_TEST_TARGET_DIR_ENV) {`

## IROHA_TEST_USE_DEFAULT_EXECUTOR (བརྟག་དཔྱད་: ༣)

- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/Executer.s:2383 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/Executer.s:2520 — `std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/state.s:12008 — `if std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR").is_some() {`

## IROHA_TORII_OPENAPI_ACTUAL (test: 1)

- བརྟག་དཔྱད།: ཀེརེསི་/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད/རའུ་ཊར་_མེ་ཊིག་.s:94 — `if let Ok(actual_path) = std::env::var("IROHA_TORII_OPENAPI_ACTUAL") {`

## IROHA_TORII_OPENAPI_XPECTED (test: ༢)

- བརྟག་དཔྱད།: ཀེརེསི་/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད/རའུ་ཊར་_མེ་ཊིག་.s:88 — `std::env::var("IROHA_TORII_OPENAPI_EXPECTED").is_err(),`
- བརྟག་དཔྱད།: ཀེརེསི་/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད/རའུ་ཊར་_མེ་ཊིག་.s:104 — `let Ok(expected_path) = std::env::var("IROHA_TORII_OPENAPI_EXPECTED") else {`

## IROHA_TORII_OPENAPI_TANS (ལག་ཆས་: ༢)

- ལག་ཆས་: xtask/src/main:11025 — `if let Some(env_tokens) = std::env::var_os("IROHA_TORII_OPENAPI_TOKENS") {`
- ལག་ཆས་: xtask/src/main:11077 — `token_header = std::env::var("IROHA_TORII_OPENAPI_TOKENS")`

## IVM_BIN (བརྟག་དཔྱད: ༢)

- བརྟག་དཔྱད།: མཉམ་སྡེབ་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པལ་.60 — `let ivm_bin = env::var("IVM_BIN")`.
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པེལསི།:163 — `let ivm_bin = env::var("IVM_BIN")`.

## IVM_COMPILER_DEBUG (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ཀོ་ཊོ་ཌ་མ་_ལང་/ཀམ་ལི་ཡར་:༤༠༩༤ — `let compiler_debug = if std::env::var_os("IVM_COMPILER_DEBUG").is_some() {`

## IVM_CUDA_GENCODE (སྒྲིག་པ: ༡)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཝི་ཨེམ་/བའིལཌ་.༣༥ — I༡༨NI00000394X

## IVM_CUDA_NVCC (བཟོ་བསྐྲུན་: ༡)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཨཝ་ཨེམ་/བའིལཌ་.30 — `let nvcc = env::var("IVM_CUDA_NVCC")`

## IVM_CUDA_NVCC_EXTRA (བཟོ་བསྐྲུན་: ༡)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཝི་ཨེམ་/བའིལཌ་.༣༦ — I༡༨NI00000396X

## IVM_DEBUG_IR (test: 1)

- བརྟག་དཔྱད།: ཀེརེ་ཊི/ཨཝ་ཨེམ་/བརྟག་དཔྱད།/རྐྱེན་སེལ་.༡༥ — `if std::env::var_os("IVM_DEBUG_IR").is_some() {`

## IVM_DEBUG_METAL_ENUM (རྐྱེན་པ: ༡)

- རྐྱེན་སེལ་: ཀེརེ་ཊི/ཝི་ཨེམ་/ཨེསི་ཨར་སི་/ཝེག་ཊར་.༤༧༤ — I༡༨NI00000398X

## IVM_DEBUG_METAL_SELFTEST (རྐྱེན་སེལ་: ༡)

- རྐྱེན་སེལ་: ཀེརེ་ཊི/ཝི་ཨེམ་/ཨེསི་ཨར་སི་/ཝེག་ཊར་.1205 — `std::env::var("IVM_DEBUG_METAL_SELFTEST")`

## IVM_DISABLE_CUDA (རྐྱེན་སེལ་: ༡)

- རྐྱེན་སེལ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.༣༡༥ — I༡༨NI00000400X

## IVM_DISABLE_METAL (རྐྱེན་སེལ་: ༡)

- རྐྱེན་སེལ་: ཀེརེ་ཊི/ཝི་ཨེམ་/ཨེསི་ཨར་སི་/ཝེག་ཊར་.༢༩༩ — I༡༨NI00000401X

## IVM_FORCE_CUDA_SelFTEST_FAIL (རྐྱེན་སེལ་: ༡)

- རྐྱེན་སེལ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་:༣༢༦ — I༡༨NI00000402X

## IVM_FORCE_METAL_ENUM (རྐྱེན་སེལ་: ༡)

- རྐྱེན་སེལ་: ཀེརེ་ཊི/ཝི་ཨེམ་/ཨེསི་ཨར་སི་/ཝེག་ཊར་.༤༤༤ — I༡༨NI00000403X

## IVM_FORCE_METAL_SELFTEST_FAIL (རྐྱེན་སེལ་: ༡)

- རྐྱེན་སེལ་: ཀེརེ་ཊི/ཝི་ཨེམ་/ཨེསི་ཨར་སི་/ཝེག་ཊར་.༡༡༩༢ — I༡༨NI00000404X

## IVM_TOOL_BIN (test: 1)

- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པལ་.113 — `let ivm_tool = env::var("IVM_TOOL_BIN")`

## ཨི་ཛཱན་ཨེམ་_ཨེལ་ལོ་ཝ་_NET (test: 1).

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་ཟ་ན་མི།/src/ཆའོ་སི་.༣༧༣ — I༡༨NI00000406X

## ཨི་ཟན་ཨེམ་_ཊི་ཡུ་ཨའི་_ཨེ་ཨེལ་ཨེལ་ཨོ་_ཟེར་ཨོ་_ཨེསི་ཨི་ཌི་ (prod: 1)

- པྲོཌ་: ཀྲེ་ཊི།/ཨི་ཟ་ན་མི།/src/tui.s:134 — `if args.seed == Some(0) && std::env::var("IZANAMI_TUI_ALLOW_ZERO_SEED").is_err() {`

## JSONSTAGE1_CUDA_ARCH (བཟོ་བསྐྲུན་: ༡)

- བཟོ་བསྐྲུན་: ཀེརེ་ཊི/ནོ་རི་ཊོ/མགྱོགས་ཚད་༡_ཀུ་ཌ་/བའིལཌི་.s:40 — `if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {`

## JSONSTAGE1_CUDA_SKIP_BUILD (བཟོར: ༡)

- བཟོ་བསྐྲུན་: ཀེརེ་ཊི/ནོ་རི་ཊོ/མགྱོགས་ཚད་༡_ཀུ་ཌ་/བའིལཌི་.s:18 — `if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {`

## KOTO_BIN (བརྟག་དཔྱད: ༢)- བརྟག་དཔྱད།: མཉམ་སྡེབ་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པེལསི།:50 — `let koto_bin = env::var("KOTO_BIN")`
- བརྟག་དཔྱད།: མཉམ་སྡེབ་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པལ་.༡༥༤ — `let koto_bin = env::var("KOTO_BIN")`.

## ལང(བརྟག་པ: ༣)།

- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཝི་ཨེམ/སེརསི།/བིན་/ཀོ་ཊོ་_ལིནཊི་.༧༠༢ — I༡༨NI0000412X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།/i18n.s:11 — `let old_lang = env::var("LANG").ok();`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།/i18n.s:69 — `let old_lang = env::var("LANG").ok();`

## LC_ALL (བརྟག་དཔྱད: ༢)།

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།/i18n.s:12 — `let old_lc_all = env::var("LC_ALL").ok();`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད།/i18n.s:70 — `let old_lc_all = env::var("LC_ALL").ok();`

## LC_MESSAGES (བརྟག་དཔྱད: ༢)།

- བརྟག་དཔྱད།: ཀེརེཊསི/ཨཝ་ཨེམ་/བརྟག་དཔྱད།/i18n.s:13 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/i18n.s:71 — `let old_lc_messages = env::var("LC_MESSAGES").ok();`

## MAX_DEGREE (prod: 1).

- པྲོཌ་: ཀྲེ་ཊི།

## MOCHI_CONFIG (prod: 1).

- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཨུའི་ཨེ་གུའི་/སི་ཨར་སི/ཀོན་སི་/ཀོན་སི་:༣༢༥ — I༡༨NI0000420X

## MOCHI_DATA_ROOT (prod: 1).

- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པེར་ཝི་ཨོར་སི་:༢༠༦༨ — I༡༨NI0000421X

## MOCHI_TEST_USE_INTERNAL_GENESIS (prod: 1).

- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པར་ཝི་སར:1994 — `if std::env::var_os("MOCHI_TEST_USE_INTERNAL_GENESIS").is_some() {`

## ནོ་རི་ཊོ་_བེན་ཆི་_སུམ་ཨར་ (བེནཅ: ༡)

- bench: ཀྲེ་ཊིས་/ནོ་རི་ཊོ་/བེན་ཆི་/མཉམ་བསྡོམས་_ཀམ་པ་ཡར་.༧༠ — `if std::env::var("NORITO_BENCH_SUMMARY").ok().as_deref() == Some("1") {`

## ནོ་རི་ཊོ་_སི་པི་ཡུ་_ཨའི་ཨེན་ཨེཕ་ཨོ་ (ལག་ཆས་: ༡)

- ལག་ཆས་: xtask/src/stage1_bench.s:69 — `cpu: std::env::var("NORITO_CPU_INFO").ok(),`

## ནོ་རི་ཊོ་_སི་ཨར་སི་༦༤_GPU_LIB (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ནོ་རི་ཊོ/སི་ཨར་སི/སིམ་ཌི་_crc64.s:215 — `std::env::var("NORITO_CRC64_GPU_LIB").ok(),`

## ནོ་རི་ཊོ་_ཌི་སབ་བེལ་_PACKED_STRUCT (prod: 1, བརྟག་དཔྱད: ༡)

- པྲོཌ་: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཇི་ས_ཧོསཊི་/སེརསི/ལིབ་.༡༨༣ — I༡༨NI0000426X
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ།/src/lib.s:322 — `match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {`

## ནོ་རི་ཊོ་_GPU_CRC64_MIN_BYTES (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ནོ་རི་ཊོ/སི་ཨར་སི/སིམ་ཌི་_ཀར་སི་༦༤.s:༨༣ — I༡༨ཨེན་ཨའི་༠༠༠༠༠༤༢༨ཨེགསི་

## ནོ་རི་ཊོ་_པི་ཨར་_STAGE1_MIN (བརྟག་དཔྱད: ༡)

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ།/src/lib.s:4847 — `std::env::var("NORITO_PAR_STAGE1_MIN")`

## ནོ་རི་ཊོ་_ཨེསི་ཁི་_བཱའིན་ཌིངས_ཨེསི་ཝའི་ཨེན་སི་ (བཟོ་བསྐྲུན་: ༡)

- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ནོ་རི་ཊོ་/བའིལཌ་.12 — `if env::var_os("NORITO_SKIP_BINDINGS_SYNC").is_some() {`

## ནོ་རི་ཊོ་_STAGE1_GPU_MIN_BYTES (བརྟག་དཔྱད་: ༡)

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ།/src/lib.s:4881 — `std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")`

## ནོ་རི་ཊོ་_ཊི་རེསི་ (བརྟག་དཔྱད་: ༢)

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ།/src/lib.s:123 — `std::env::var_os("NORITO_TRACE").is_some()`
- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ནོ་རི་ཊོ།/src/lib.s:138 — `let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();`

## NO_pROXY (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_པི་༢པི་/ཨེསི་ཨར་སི/སྐྱེལ་འདྲེན་.༤༦༩ — I༡༨NI0000434X

## NVCC (བཟོར: ༡)།

- བཟོ་བསྐྲུན་: ཀྲེ་ཊི།

## OUT_DIR (བཟོ་བསྐྲུན་: ༤, prod: 12, བརྟག་དཔྱད: ༢)- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཕཱསིཊི་པི་ཀིའུ་_པོརོ་ཝར་/བཱུལ་ཌི་ཨེསི་:༩༩ — `let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);`
- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཨི་རོ་ཧ་_གནས་སྡུད་_མོ་ཌེལ་/བའིལཌ་.༡༢ — `let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));` .
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/src/lib.s:179 — `include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));`
- བཟོ་བསྐྲུན་: ཀེརེཊསི་/ཨཝ་ཨེམ་/བའིལཌ་.༢༧ — I༡༨NI000000439X .
- བཟོ་བསྐྲུན་: ཀྲེ་ཊི།
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.༡༢ — I༡༨NI00000441X
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.13 — `static VEC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/vector.ptx"));`
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.14 — `static SHA_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx"));`
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.15 — `let sum_asset = env::var("BLOCK_DUMP_SUM_ASSET")`
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.s:16 — `static POSEIDON_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/poseidon.ptx"));`
- པྲོཌ་: ཀེརེཊསི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.17 — `let running_under_cargo = std::env::var_os("CARGO").is_some();`
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.18 — `static AES_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/aes.ptx"));`
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.༡༩ — I༡༨NI00000448X
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.༢༠ — I༡༨NI000004449X
- པྲོཌ་: ཀེརེཊསི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.༢༡ — I༡༨NI000000450X
- པྲོཌ་: ཀེརེ་ཊི/ཝི་ཨེམ་/སི་ཨར་སི་/ཅུ་ཌ་.22 — `static BITONIC_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/bitonic_sort.ptx"));`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་ཝི་ཨེམ་/ཨེསི་ཨར་སི/པི་ཊི་ཨེགསི་_བརྟག་དཔྱད་.༧ — I༡༨NI000000452X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨཝ་ཨེམ་/བརྟག་དཔྱད/པི་ཊི་ཨེགསི་_ཀར་ནེལསི་.༥ — `let out_dir = env!("OUT_DIR");`

## P2P_TURN (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_པི་༢པི་/ཨེསི་ཨར་སི་/སྐྱེལ་འདྲེན་.296 — `let endpoint = std::env::var("P2P_TURN")`

## PHH (prod: 2, བརྟག་དཔྱད: ༡)

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིལི/སི་ཨར་སི་/ཀམ་མན་ཌི་/སོ་རཕ་སི།:7276 — `if let Some(path_var) = env::var_os("PATH") {`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/ཀོ་ཊོ་ཌ་མ་_ཨེགསི་པལ་.༡༧ — `let path = env::var_os("PATH")?;`.
- པྲོཌ་: མོ་ཅི་/མོ་ཅི་-ཀོར་/སི་ཨར་སི་/སུ་པེར་ཝི་ཨོར་སི་:༥༡༡ — `let path_var = env::var_os("PATH")?;`

## PRINT_SORACLES_FIXTURES (བརྟག་དཔྱད་: ༡)

- བརྟག་དཔྱད།: ཀེརེས་/ཨི་རོ་ཧ་_གནས་སྡུད་_ མོ་ཌེལ་/ཨེསི་ཨར་སི/ཨོ་རེ་ཀ/མོ་ཌི་:3249 — `if std::env::var_os("PRINT_SORACLES_FIXTURES").is_some() {`

## PRINT_TORI_SPEC (test: 1).

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི/སི་ཨར་སི/ཨོ་པེ་ན་པི་.༤༣༨༠ — I༡༨NI00000459X

## ༼སྒྲིང་ཁྱིམ་: ༡, prod: ༡, བརྟག་དཔྱད: ༡༽

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀོར་/སི་ཨར་སི/སུམ་ར་གི་/རྦ་སི་_སི་ཊོར་.༤༡ — I༡༨NI00000460X
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:995 — `let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());`
- བཟོ་བསྐྲུན་: མཉམ་བསྡོམ་_བརྟག་དཔྱད་/བང་སྒྲིག.180 — `let profile = match env::var("PROFILE").unwrap_or_default().as_str() {`

## PYTHON3 (བརྟག་དཔྱད: ༢)།

- བརྟག་དཔྱད།: ཀེརེཊསི་/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད།/ཏའི་ཀའི་_ཀར་_cli.s:235 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`
- བརྟག་དཔྱད།: ཀེརེ་ཊི/སོ་རཕ་ས_ཀར་/བརྟག་དཔྱད་/ཊི་ཀའི་_བལྟ་མི་_ཀི་ལི.s:30 — `let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());`

## PYTHONPATH (བརྟག་དཔྱད: ༡)།

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀིལི/བརྟག་དཔྱད་/ཀླི་_ས་མོ།:5245 — `match env::var("PYTHONPATH") {`

## RBC_SESSION_PATH (བརྟག་དཔྱད་: ༡)

- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/sumeragi/rbc_store.823 — `let path = std::env::var("RBC_SESSION_PATH").expect("set RBC_SESSION_PATH");`

## REPO_PROOF_DIGEST_OUT (བརྟག་དཔྱད་: ༡)

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/smart གན་རྒྱ།/isi/repo.2017 — `if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {`

## REPO_PROOF_SNAPSHOT_OUT (བརྟག་དཔྱད་: ༡)

- བརྟག་དཔྱད།: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཀོར།/src/smart གན་རྒྱ།/isi/repo.s:2005 — `if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {`

## RUST_LOG (prod: ༡, བརྟག་དཔྱད་: ༢)

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:5414 — `let original = env::var("RUST_LOG").ok();`
- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:5431 — `let original = env::var("RUST_LOG").ok();`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་ཟ་ནམ་མི/སི་ཨར་སི་/ཀོན་སིརསི་:༢༦༩ — I༡༨NI00000471X

## SM_PERF_CPU_LABEL (prod: 2).

- པྲོཌ་: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/ཨེསི་ཨར་སི/བིན་/ཨེསི་ཨེམ་_པར་ཕི་_ཅེག་.s:637 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {` .
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིརིཔ་ཊོ་/ཨེསི་ཨར་སི/བིན་/ཨེསི་ཨེམ་_པར་ཕི་_ཅེག་.s:679 — `if let Ok(cpu) = env::var("SM_PERF_CPU_LABEL") {`

## སོརཕསི་_NODE_SKIP_INGEST_TESTS (test: 1).

- བརྟག་དཔྱད།: ཀེརེ་ཊི/སོ་རཕ་ས_ནོཌི་/བརྟག་དཔྱད་/ཀླི་.s:17 — `std::env::var("SORAFS_NODE_SKIP_INGEST_TESTS").map_or(true, |value| value != "1")`

## སོརཕསི_ཊོརའིཨའི་_ཨེསི་ཀིཔ་_ཨིང་སིཊི་_ཊི་ཨི་ཨེསི་ཊི་ཨེསི་ (བརྟག་དཔྱད: ༡)།

- བརྟག་དཔྱད།: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི་/བརྟག་དཔྱད་/སོ་རཕ་ས_ཌིསི་གསར་འཚོལ་.༩༥ — I༡༨NI00000475X## ས་ཡོམ་_ADVERSARIAL_ARTIFACT_DIR (test: 1)

- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/སུམ་ར་གི་_ཨ་ཌི་ཝར་སི་ཡལ་.1222 — `let Ok(dir) = std::env::var("SUMERAGI_ADVERSARIAL_ARTIFACT_DIR") else {`

## སྤྱི་མཐུན་_BELINE_ARTIFACT_DIR (prod: 1, བརྟག་དཔྱད: ༡)

- པྲོཌ་: ཀེརེཊསི་/བཟོ་བསྐྲུན་-རྒྱབ་སྐྱོར་/ཨེསི་ཨར་སི/བིན་/སུ་མར་གྷི་_བེསི་ལིན་_སྙན་ཞུ་.40 — `let env = std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR").map_err(|_| {` .
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/སུམ་ར་གི་_ཨེན་པོ_ལས་འགན། 1396 — `let dir = match std::env::var("SUMERAGI_BASELINE_ARTIFACT_DIR") {`

## SUMERAGI_DA_ARTIFACT_DIR (prod: 1, བརྟག་དཔྱད་: 1).

- prod: ཀེརེཊསི་/བཟོ་བསྐྲུན་-རྒྱབ་སྐྱོར་/ཨེསི་ཨར་སི/བིན་/སུ་མར་གྷི་_ཌ_རེརཊ།:༤༡ — `let env = std::env::var("SUMERAGI_DA_ARTIFACT_DIR").map_err(|_| {`
- བརྟག་དཔྱད།: མཉམ་བསྡོམས་_བརྟག་དཔྱད་/བརྟག་དཔྱད་/བརྟག་དཔྱད་/སུམ་ར་གི་_དྲ་.༡༥༩༩ — `let Ok(dir) = std::env::var("SUMERAGI_DA_ARTIFACT_DIR") else {`

## རིམ་ལུགས་རུཊ་ (prod: 1).

- པྲོཌ་: ཀེརེཊསི་/ཕཱསཊི་པི་ཀིའུ་_ པོརོ་ཝར་/ཨེསི་ཨར་སི/བེག་བེནཌ།

## TARGET (ལག་ཆས་: ༢)

- ལག་ཆས་: xtask/src/poseidon_bench.s:87 — `target: std::env::var("TARGET")`
- ལག་ཆས་: xtask/src/stage1_bench.s:63 — `target: std::env::var("TARGET")`

## ཊི་ཨེསི་ཊི་_ཨེལ་ཨོ་ཇི་_ཨེཕ་ཨའི་ཨེལ་ཊར་ (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ལོག་གར་/src/lib.s:89 — `filter: std::env::var("TEST_LOG_FILTER")`

## TEST_LOG_LEVEL (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ལོག་གར་/src/lib.s:85 — `level: std::env::var("TEST_LOG_LEVEL")`

## TEST_NETWORK_CARGO (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_བརྟག་དཔྱད་_ཡོངས་འབྲེལ་/src/lib.s:865 — `std::env::var("TEST_NETWORK_CARGO").unwrap_or_else(|_| "cargo".to_owned());`

## TORII_DEBUG_SORT (prod: 1).

- པྲོཌ་: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི།/src/routing.s:12530 — `std::env::var("TORII_DEBUG_SORT").ok().is_some(),`

## TORII_MOCK_HARNES_METRICS_PATH (ལག་ཆས་: ༡)

- ལག་ཆས་: xtask/src/bin/tori_mock_harness.106 — `metrics_path: env::var("TORII_MOCK_HARNESS_METRICS_PATH")`

## TORII_MOCK_HARNESS_REPO_ROOT (ལག་ཆས་: ༡)

- ལག་ཆས་: xtask/src/bin/tori_mock_harness.109 — `repo_root: env::var("TORII_MOCK_HARNESS_REPO_ROOT")`

## TORII_MOCK_HARNESS_RETRY_TOTAL (ལག་ཆས་: ༡)

- ལག་ཆས་: xtask/src/bin/tori_mock_harness.297 — `env::var("TORII_MOCK_HARNESS_RETRY_TOTAL")`

## TORII_MOCK_HARNESS_RUNNER (ལག་ཆས་: ༡)

- ལག་ཆས་: xtask/src/bin/tori_mock_harness.112 — `runner: env::var("TORII_MOCK_HARNESS_RUNNER")`

## TORII_MOCK_HARNESS_SDK (ལག་ཆས་: ༡)

- ལག་ཆས་: xtask/src/bin/tori_mock_harness.104 — `sdk: env::var("TORII_MOCK_HARNESS_SDK").unwrap_or_else(|_| "android".to_string()),`

## TORII_OPENAPI_TOKEN (ལག་ཆས་: ༢)

- ལག་ཆས་: xtask/src/main:11020 — `if let Ok(single) = std::env::var("TORII_OPENAPI_TOKEN")`
- ལག་ཆས་: xtask/src/main.s:11073 — `let mut token_header = std::env::var("TORII_OPENAPI_TOKEN")`

## དུས་མཐུན་བཟོ་ནི་_FIXTURES (test: 1).

- བརྟག་དཔྱད།: ཀེརེཊསི་/ཨི་རོ་ཧ་_ཀོར་/བརྟག་དཔྱད་/པར་ལེན་ཚུ་།༤༢ — I༡༨NI0000495X

## UsERPROFILE (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་/སི་ཨར་སི་/ཀོན་སི་ཨར་ཨེསི་:༥༥ — I༡༨NI00000496X

## VERGEN_CARGO_FEATURES (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧད་/སི་ཨར་སི་/མ་ནིརས:༤༨༥༠ — I༡༨NI00000497X

## VERGEN_CARGO_TARGET_TRIPLE (prod: 1).

- པྲོཌ་: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཊེ་ལི་མི་ཊི་རི།/src/ws.s.238 — `let vergen_target = option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or("unknown");`

## VERGEN_GIT_SHA (prod: 4).

- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧ་_ཀིལི/སི་ཨར་སི/མའེན་_ཤེར་ཌི་.༥༧ — I༡༨NI0000499X དང་།
- པྲོཌ་: ཀྲེ་ཊི།
- པྲོཌ་: ཀྲེ་ཊི།/ཨི་རོ་ཧ་_ཊོ་རི།/src/routing.s:32515 — `let verbose = env::var("BLOCK_DUMP_VERBOSE").is_ok();`
- པྲོཌ་: ཀེརེ་ཊི/ཨི་རོ་ཧད་/སི་ཨར་སི་/མ་ནིརས:༤༨༤༥ — I༡༨NI00000502X

## བདེན་པ (བེནཆ: ༡)

- བེནཆ་: ཀེརེཊསི་/ཨཝ་ཨེམ་/བེན་ཆི་/བེནཆ་_ཝོ་ཊིང་།

## བདེན་པ(བེནཆ: ༡)

- བེནཆ་: ཀེརེ་ཊི/ཨཝ་ཨེམ/བེན་ཆི།

## VOTERS (bench: 1).

- བེནཆ་: ཀྲེ་ཊི།

## ནོ_པོརོ་སི་ (prod: 1).

- པྲོཌ་: ཀེརེ་ཊི།/ཨི་རོ་ཧ་_པི་༢པི་/ཨེསི་ཨར་སི་/སྐྱེལ་འདྲེན་.༤༧༡ — I༡༨NI00000506X