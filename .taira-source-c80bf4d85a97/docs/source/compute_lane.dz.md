---
lang: dz
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# གློག་རིག་ལམ་ (SSC-1)

རྩིས་སྟོན་ལམ་འདི་གིས་ གཏན་འབེབས་ཨེཆ་ཊི་ཊི་པི་-བཟོ་རྣམ་འབོད་བརྡ་ཚུ་ ངོས་ལེན་འབདཝ་ཨིནམ་དང་ དེ་ཚུ་ Kotodama ལུ་ སབ་ཁྲ་བཟོཝ་ཨིན།
འཛུལ་ཞུགས་ས་ཚིགས་ཚུ་དང་ དངུལ་འཛིན་དང་གཞུང་སྐྱོང་བསྐྱར་ཞིབ་ཀྱི་དོན་ལུ་ མི་ཊར་/འོང་འབབ་ཚུ་ཐོ་བཀོད་འབདཝ་ཨིན།
འདི་ RFC གིས་ གསལ་སྟོན་ལས་རིམ་དང་ ཁ་པར་/ལེན་ཡིག་གི་ཡིག་ཤུབས་ཚུ་ བྱེམ་སྒྲོམ་གྱི་སྲུང་རྒྱབ་ཚུ་ གྱང་ཤུགས་ཅན་སྦེ་བཟོཝ་ཨིན།
དང་ རིམ་སྒྲིག་གིས་ གསར་བཏོན་དང་པ་གི་དོན་ལུ་ སྔོན་སྒྲིག་འབདཝ་ཨིན།

## གསལ༌སྟོན༌འབད༌ནི

- ལས་འཆར་: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` འདི་ `1` ལུ་བཙུགས་ཡོདཔ་ཨིན། ཐོན་རིམ་སོ་སོ་ཅིག་དང་གཅིག་ཁར་ གསལ་སྟོན་ཚུ་ ངོས་ལེན་མ་འབད་བར་ཡོདཔ་ཨིན།
  བདེན་དཔྱད་ཀྱི་སྐབས།
- འགྲུལ་ལམ་རེ་རེ་གིས་གསལ་བསྒྲགས་འབདཝ་ཨིན།
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama འཛུལ་སྒོ་མིང་།)
  - ཀོ་ཌེཀ་ ཐོ་ཡིག་ (`codecs`)
  - ཊི་ཊི་ཨེལ་/གཱསི་/ཞུ་བ་/ལན་གསལ་གྱི་མགུ་ཏོག་ (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - གཏན་འབེབས་བཟོ་ནི/ལག་ལེན་འཐབ་པའི་སློབ་ཚན་ (`determinism`, `execution_class`)
  - SoraFS ingress/དཔེ་ཚད་འགྲེལ་བཤད་ (`input_limits`, གདམ་ཁའི་`model`)
  - གོང་ཚད་ཅན་གྱི་བཟའ་ཚང་ (`price_family`) + ཐོན་ཁུངས་གསལ་སྡུད་ (`resource_profile`)
  - བདེན་བཤད་སྲིད་བྱུས་ (`auth`)
- སེནཌ་བོགསི་ རེལ་རེལ་ཚུ་ གསལ་སྟོན་ནང་ `sandbox` བཀག་ཆ་ནང་ སྡོདཔ་ཨིནམ་དང་ ཆ་མཉམ་གྱིས་ བརྗེ་སོར་འབདཝ་ཨིན།
  ལམ་ཚུ་ (ཐབས་ལམ་/གང་བྱུང་/གསོག་འཇོག་དང་ གཏན་འབེབས་མེན་པའི་ རིམ་ལུགས་བཀག་ཆ་)།

དཔེར་ན་: `fixtures/compute/manifest_compute_payments.json`.

## འབོད་བརྡ་དང་ཞུ་བ་ དེ་ལས་ཐོབ་ཚུལ་ཚུ།

- ལས་འཆར་ - ལས་འཆར་ - ལས་འཆར་ - ལས་རིམ།, ལས་རིམ།, `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` ནང་།
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` གིས་ ཀེ་ནོ་ནིག་ཞུ་བ་ཧེཤ་ (མགོ་ཡིག་ཚུ་བཞག་ཡོདཔ་ཨིན།
  ནང་ གཏན་འབེབས་ `BTreeMap` ནང་ དེ་ལས་ པེ་ལོཌ་འདི་ `payload_hash` སྦེ་ འབག་འོང་།)
- `ComputeCall` མིང་གནས་/ལམ་ལུགས་ ཀོ་ཌེཀ་ ཊི་ཊི་ཨེལ་/རླངས་རླུང་/ལན་འདེབས་ཀྱི་ ཀེབ་ཚུ་ བཟུང་ཡོདཔ་ཨིན།
  ཐོན་ཁུངས་གསལ་སྡུད་ + གོང་ཚད་བཟའ་ཚང་།
  `ComputeAuthn`), གཏན་འབེབས་བཟོ་(`Strict` vs `BestEffort`), བཀོལ་སྤྱོད་འཛིན་གྲྭ།
  བརྡ་སྟོན་ཚུ་ (སི་པི་ཡུ་/ཇི་པི་ཡུ་/ཊི་ཨི་) གསལ་བསྒྲགས་འབད་ཡོདཔ། SoraFS ཨིན་པུཊི་བཱའིཊི་/ཆུམ་ཚུ་, གདམ་ཁའི་རྒྱབ་སྐྱོར་པ།
  འཆར་དངུལ་དང་ ཁྲིམས་ལུགས་ཀྱི་ཞུ་བ་ཡིག་ཤུབས་ཁང་། ཞུ་བ་ཧེཤ་འདི་ གི་དོན་ལུ་ལག་ལེན་འཐབ་ཨིན།
  replay ཉེན་སྐྱོབ་དང་ འགྲུལ་བཞུད།
- རའུཊི་ཚུ་གིས་ གདམ་ཁ་ཅན་གྱི་ SoraFS དཔེ་ཚད་གཞི་བསྟུན་དང་ ཨིན་པུཊི་ཚད་ཚུ་ བཙུགས་ཚུགས།
  (inline/chunk caps); base sandbox ཁྲིམས་ལུགས་ GPU/TEE བརྡ་སྟོན་ཚུ།
- `ComputePriceWeights::charge_units` གིས་ མི་ཊར་ལིང་གནས་སྡུད་འདི་ བིལཌི་ཌིལ་ཀལ་ལུ་གཞི་བསྒྱུར་འབདཝ་ཨིན།
  འཁོར་རིམ་དང་ eregt bytes གི་ཐོག་ལས་ ཐོག་ཁར་གྱི་ ཆ་ཚད་ཚུ་ བརྒྱུད་དེ་ཨིན།
- `ComputeOutcome` སྙན་ཞུ་ `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted`, ཡང་ན་ Norito དང་ གདམ་ཁ་ཅན་གྱི་ཐོག་ལས་ ལན་འདེབས་ཧ་ཤེ་/ ཚུད་ཡོད།
  རྩིས་ཞིབ་ཀྱི་དོན་ལུ་ ཚད་/ཨང་།

དཔེར་ན་:
- ཁ་པར་ཨང་: `fixtures/compute/call_compute_payments.json`
- ངོས་ལེན: `fixtures/compute/receipt_compute_payments.json`

## སེང་སྒྲོམ་དང་ཐོན་ཁུངས་གསལ་སྡུད།- SoraFS གིས་ སྔོན་སྒྲིག་གིས་ `IvmOnly` ལུ་ ལག་ལེན་འཐབ་ཐངས་འདི་ བསྡམ་བཞགཔ་ཨིན།
  སོན་ཚུ་གིས་ ཞུ་བ་ཧེ་ཤི་ལས་ གང་བྱུང་གཏན་འབེབས་བཟོ་སྟེ་ ལྷག་ནི་རྐྱངམ་ཅིག་ SoraFS འབད་བཅུགཔ་ཨིན།
  འཛུལ་སྤྱོད་དང་ གཏན་འབེབས་མེན་པའི་ syscalls ཚུ་ ངོས་ལེན་འབདཝ་ཨིན། GPU/TEE བརྡ་སྟོན་ཚུ་ ༡ གིས་ འཛུལ་ཚུགས།
  `allow_gpu_hints`/`allow_tee_hints` འདི་ ལག་ལེན་འཐབ་ནིའི་ཐག་བཅད་བཞག་ནིའི་དོན་ལུ་ཨིན།
- `ComputeResourceBudget` ཆ་ཚན་ཚུ་ འཁོར་རིམ་ཚུ་གུ་ཡོད་པའི་ གསལ་སྡུད་རེ་ལུ་ རིམ་འགྲོས་དྲན་ཚད་ བང་རིམ་ཚུ།
  sidet དང་ IO འཆར་དངུལ་དང་ ཕྱིར་ཐོན་ དེ་ལས་ GPU བརྡ་སྟོན་དང་ WASI-lite གྲོགས་རམ་གྱི་དོན་ལུ་ བསྒྱུར་བཅོས་འབདཝ་ཨིན།
- སྔོན་སྒྲིག་ཚུ་གིས་ གསལ་སྡུད་གཉིས་ (`cpu-small`, `cpu-balanced`) འོག་ལུ་བཏང་ཡོདཔ་ཨིན།
  `defaults::compute::resource_profiles` དང་བཅས་ གཏན་འབེབས་ཀྱི་ མར་ཕབ་ཚུ་ འབད་ཡོདཔ།

## རིན་གོང་དང་བིལ་ལིང་ཆ་ཚན།

- གོང་ཚད་བཟའ་ཚང་ (`ComputePriceWeights`) སབ་ཁྲ་འཁོར་རིམ་དང་ ཕྱིར་ཐོན་བཱའིཊི་ཚུ་ རྩིས་རྐྱབ་ནི།
  Units; སྔོན་སྒྲིག་གློག་ཤུགས་ `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` དང་།
  `unit_label = "cu"`. བཟའ་ཚང་ཚུ་ གསལ་སྟོན་ནང་ `price_family` གིས་ ལྡེ་མིག་བཀོད་དེ་ཡོདཔ་ཨིན།
  འཛུལ་ཞུགས་ནང་ བསྟར་སྤྱོད་འབད་ཡོདཔ།
- མི་ཊར་གྱི་དྲན་ཐོ་ཚུ་ `charged_units` དང་ འཁོར་རིམ་/འཁོར་རིམ་/ཐོན་ཤུགས་/དུས་ཡུན་ཚུ་ འབག་འོང་།
  མཐུན་སྒྲིལ་གྱི་དོན་ལུ་ བསྡོམས་འབོར། གློག་ཚད་ཚུ་ བཀོལ་སྤྱོད་-དབྱེ་རིམ་དང་ རྒྱ་སྐྱེད་འབདཝ་ཨིན།
  determinism སྒྱུར་རྩིས་ (`ComputePriceAmplifiers`) དང་ མཐའ་བཅད་ཀྱི་ གིས་ ༢༠༡༨ ཅན་མ།
  `compute.economics.max_cu_per_call`; eregs འདི་ བསྡམས།
  `compute.economics.max_amplification_ratio` མཐུད་འབྲེལ་ལན་རྒྱ་བསྐྱེད་ལུ་།
- མ་དངུལ་རྒྱབ་སྐྱོར་གྱི་འཆར་དངུལ་ (`ComputeCall::sponsor_budget_cu`) འདི་ ངོ་རྒོལ་འབདཝ་ཨིན།
  ཁ་པར་རེ་ལུ་ ཉིན་བསྟར་གྱི་ཁ་དོག་ཚུ། བྱུང་འཛིན་གྱི་ ཡན་ལག་ཚུ་ གསལ་བསྒྲགས་འབད་ཡོད་པའི་ མ་དངུལ་རྒྱབ་སྐྱོར་འབད་མི་ འཆར་དངུལ་ལས་ བརྒལ་མི་ཆོག།
- གཞུང་ཚབ་ཀྱི་གོང་ཚད་དུས་མཐུན་བཟོ་ནི་ཚུ་ སྤྱི་ལོ་༢༠༠༨ ལུ་ ཉེན་ཁ་ཅན་གྱི་གནས་རིམ་གྱི་མཐའ་མཚམས་ཚུ་ལག་ལེན་འཐབ་ཨིན།
  `compute.economics.price_bounds` དང་གཞི་རྩའི་བཟའ་ཚང་ཚུ་ སྤྱི་ལོ་༢༠༠༨ ལུ་ ཐོ་བཀོད་འབད་ཡོདཔ་ཨིན།
  `compute.economics.price_family_baseline`; ལག་ལེན་འཐབ་ནི་
  `ComputeEconomics::apply_price_update` དུས་མཐུན་བཟོ་པའི་ཧེ་མ་ ཌེལ་ཊ་ཚུ་ བདེན་དཔྱད་འབད་ནི་ལུ་ཨིན།
  ཤུགས་ཅན་བཟའ་ཚང་གི་ས་ཁྲ། Torii རིམ་སྒྲིག་དུས་མཐུན་བཀོད།
  `ConfigUpdate::ComputePricing`, དང་ ཀི་སོ་གིས་ མཐའ་མཚམས་གཅིག་པ་ལུ་ འཇུག་སྤྱོད་འབདཝ་ཨིན།
  གཞུང་སྐྱོང་ཞུན་དག་འབད་དེ་ གཏན་འབེབས་བཟོཝ་ཨིན།

## རིམ་སྒྲིག་འབད་ནི།

གློག་རིག་རིམ་སྒྲིག་གསརཔ་ `crates/iroha_config/src/parameters` ནང་:

- ལག་ལེན་པའི་མཐོང་སྣང་: `Compute` (`user.rs`) envreds:
  - `COMPUTE_ENABLED` (སྔོན་སྒྲིག་ Kotodama)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - ```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```
- གོང་ཚད་/དཔལ་འབྱོར་: `compute.economics` འཛིན་བཟུང་།
  `max_cu_per_call`/`max_amplification_ratio`, fie style, མ་དངུལ་རྒྱབ་སྐྱོར་ཁེབ།
  (ཁ་པར་རེ་ལུ་དང་ ཉིན་བསྟར་གྱི་ CU) དང་ གོང་ཚད་བཟའ་ཚང་གཞི་རྟེན་ + ཉེན་ཁ་ཅན་གྱི་དབྱེ་རིམ་/མཐའ་མཚམས་ཚུ་ གི་དོན་ལུ་ཨིན།
  གཞུང་སྐྱོང་དུས་མཐུན་དང་ ལག་ལེན་འཐབ་ཐངས་-དབྱེ་རིམ་སྒྱུར་རྩིས་ (GPU/TEE/beest-fufort)།
- ངོ་མའི་/སྔོན་སྒྲིག་ཚུ་: `actual.rs` / `defaults.rs::compute` གིས་ དབྱེ་དཔྱད་འབད་ཡོདཔ་ཨིན།
  `Compute` སྒྲིག་སྟངས་ (མིང་གི་ས་སྒོ་དང་ གསལ་སྡུད་ གོང་ཚད་ཀྱི་བཟའ་ཚང་ བྱེམ་སྒམ་ཚུ་)།
- ནུས་མེད་རིམ་སྒྲིག་ཚུ་ (མིང་གནས་སྟོངམ་ཚུ་ སྔོན་སྒྲིག་གསལ་སྡུད་/བཟའ་ཚང་བརླག་སྟོར་ཞུགས་མི་ ཊི་ཊི་ཨེལ་ ཁབ་ལེན།
  མིང་དཔྱད་འབད་བའི་སྐབས་ `InvalidComputeConfig` འདི་ ཕྱིར་འཐེན་འབད་ཡོདཔ་ཨིན།

## བརྟག་དཔྱད་དང་སྒྲིག་ཆ།

- གཏན་འཁེལ་གྱི་གྲོགས་རམ་པ་ (`request_hash`, གོང་ཚད་) དང་ བདེ་སྒྲིག་གི་ rounsfrips འདི་ ༢༠༠༨ ལུ་སྡོད་དོ་ཡོདཔ་ཨིན།
  `crates/iroha_data_model/src/compute/mod.rs` (`fixtures_round_trip`, ལ་གཟིགས།
  `crates/iroha_data_model/src/compute/mod.rs`, `pricing_rounds_up_units`).
- JSON སྒྲིག་བཀོད་ཚུ་ `fixtures/compute/` ནང་ལུ་སྡོད་ཡོདཔ་དང་ གནས་སྡུད་-དཔེ་ཚད་ཀྱིས་ ལག་ལེན་འཐབ་ཨིན།
  འགྱུར་ལྡོག་ཁྱབ་ཚད་ཀྱི་དོན་ལུ་བརྟག་དཔྱད་ཚུ།

## SLO ཧར་ནིས་དང་སྔོན་རྩ།- `compute.slo.*` རིམ་སྒྲིག་གིས་ གཱེཊི་ཝེ་ཨེསི་ཨེལ་ཨོ་ མཛུབ་གནོན་ཚུ་ (འཕུར་འགྲུལ་གྱི་གྱལ་ནང་ ཀིའུ་ མཛུབ་གནོན་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན།
  གཏིང་ཚད་དང་ཨར་པི་ཨེསི་ཁ་དོག་དང་ བར་ཆད་དམིགས་ཚད་ཚུ་) ནང་།
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. སྔོན་སྒྲིག་ཚུ་: ༣༢
  འཕུར་འགྲུལ་ནང་, ༥༡༢ ལམ་རེ་ལུ་ ༥༡༢, ༢༠༠ RPS, p50 25ms, p95 75ms, p99 120ms.
- ཨེསི་ཨེལ་ཨོ་བཅུད་བསྡུས་དང་ཞུ་བ་/ཕྱིར་ཐོན་འཛིན་བཟུང་འབད་ནི་ལུ་ ལྗིད་མེད་བེང་ཅི་ སྲབ་འཁོར་འདི་གཡོག་བཀོལ།
  snapshot: `cargo ran -p xtask --bin copit_gate --bench [pathp].
  [རྣམ་གྲངས་] [ཨའུཊ་_ཌིར་]` (defaults: `fixtures/compope/མ་རབས་_གློག་རིག་_གླ་ཆ་.json`,
  ༡༢༨ བསྐྱར་ལོག་, མཉམ་བསྡོམས་ ༡༦, ཐོན་འབྲས་ཚུ།
  `artifacts/compute_gateway/bench_summary.{json,md}`). བང་ཁྲི་ལག་ལེན་འཐབ་ཨིན།
  གཏན་འཁེལ་སྤྲོད་ལེན་ (`fixtures/compute/payload_compute_payments.json`) དང་།
  ལུས་སྦྱོང་འབད་བའི་སྐབས་ ཁ་ཐུག་རྐྱབ་ནི་ལས་ བཀག་ཐབས་ལུ་ per-red reques ཞུ་བ་འབདཝ་ཨིན།
  `echo`/`uppercase`/`sha3` འཛུལ་སྒོ་ཚུ།

## SDK/CLI ཆ་སྙོམས་སྒྲིག་ཆས།

- ཀེན་ནོ་ཀཱལ་སྒྲིག་བཀོད་ཚུ་ `fixtures/compute/` གི་འོག་ལུ་སྡོད་ཡོདཔ་ཨིན།
  འཛུལ་སྒོ་གི་བཟོ་རྣམ་ལན་/ལེན་བཀོད་སྒྲིག། གླ་ཆའི་ཧ་ཤེ་ཚུ་ འབོད་བརྡ་དང་མཐུན་དགོཔ་ཨིན།
  `request.payload_hash`; རོགས་རམ་འབད་མི་ གླ་ཆ་འདི་ ༢༠ ནང་ཡོདཔ་ཨིན།
  `fixtures/compute/payload_compute_payments.json`.
- སི་ཨེལ་ཨའི་གྲུ་གཟིངས་ `1` དང་ `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`method`/Torii ནང་སྡོད་ཡོད།
  `javascript/iroha_js/src/compute.js` འོག་ལུ་ འགྱུར་ལྡོག་བརྟག་དཔྱད་དང་མཉམ་དུ།
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` གིས་ སྒྲིག་ཆས་གཅིག་པ་མངོན་གསལ་འབདཝ་ཨིན་ དེ་གིས་ པེ་ལོཌི་ཧ་ཤེ་ཚུ་བདེན་དཔྱད་འབདཝ་ཨིན།
  དང་ ༢༠༠༨ ལུ་ འཛུལ་སྒོ་ཚུ་ བརྟག་དཔྱད་ཚུ་དང་གཅིག་ཁར་ དཔེ་སྟོན་འབདཝ་ཨིན།
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- CLI/JS/Swift གི་གྲོགས་རམ་པ་ཆ་མཉམ་གྱིས་ Norito ཅོག་འཐདཔ་འདི་བགོ་བཤའ་རྐྱབ་སྟེ་ SDKs འབད་ཚུགས།
  ཞུ་བ་བཟོ་བསྐྲུན་དང་ ཧེ་ཤི་གིས་ ཨོཕ་ལཱའིན་ལུ་ མ་བརྡུང་བར་ བདེན་དཔྱད་འབད།
  རྒྱུག་པའི་སྒོ་ར།