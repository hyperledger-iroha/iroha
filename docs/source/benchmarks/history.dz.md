---
lang: dz
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-12-29T18:16:35.920451+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ཇི་པི་ཡུ་བེན་ཀ་མཱརཀ་ ཡིག་ཆའི་ལོ་རྒྱུས། (FASTPQ WP5-B)

ཡིག་སྣོད་འདི་ `python3 scripts/fastpq/update_benchmark_history.py` གིས་བཟོ་བཏོན་འབད་ཡོདཔ་ཨིན།
འདི་གིས་ ཕེསི་ཊི་ཀིའུ་ གནས་རིམ་ ༧ WP5-B འདི་ ག་ར་བཀབ་ཡོད་པའི་ ཇི་པི་ཡུ་ བརྟག་ཞིབ་འབད་དེ་ བཀྲམ་སྤེལ་འབད་ཚུགསཔ་ཨིན།
བེན་ཀ་མཱརཀ་ artfact དང་ Poseidon microbench གཉིས་ འོག་ལུ་ གསལ་སྟོན་འབདཝ་ཨིན།
`benchmarks/`. འོག་ལུ་ཡོད་པའི་བཟུང་ཚུ་དུས་མཐུན་བཟོ་ཞིནམ་ལས་ གསརཔ་འབདཝ་ད་ ཡིག་ཚུགས་འདི་ལོག་གཡོག་བཀོལ།
བཱན་ཌལ་ས་ཞིང་ཡང་ན་ ཊེ་ལི་མི་ཊི་ལུ་ སྒྲུབ་བྱེད་གསརཔ་དགོཔ་ཨིན།

## ཁྱབ་ཁོངས་དང་དུས་མཐུན་བྱ་རིམ།

- ཇི་པི་ཡུ་ གསརཔ་ཚུ་ བཟོ་སྐྲུན་འབད་ནི་ ཡང་ན་ བཀབ་ནི་ (bia `scripts/fastpq/wrap_benchmark.py`), via `scripts/fastpq/wrap_benchmark.py`)
  འཛིན་བཟུང་མེ་ཊིགསི་ལུ་ མཐུད་ཞིནམ་ལས་ གསར་བསྐྲུན་འབད་ནིའི་དོན་ལུ་ གློག་ཤུགས་འདི་ ལོག་གཡོག་བཀོལ་ནི།
  ཐིག་ཁྲམ་ཚུ།
- པོ་སི་ཌོན་མའི་ཀོ་རོ་བེནཆ་གནས་སྡུད་ཡོད་པའི་སྐབས་ དང་གཅིག་ཁར་ཕྱིར་འདྲེན་འབད།
  `scripts/fastpq/export_poseidon_microbench.py` དང་ དེ་ལས་ གསལ་སྟོན་ལག་ལེན་འཐབ་ཐོག་ལས་ གསལ་སྟོན་འདི་ ལོག་བཟོ་བསྐྲུན་འབད།
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- གཤམ་གསལ་གྱི་ ཇེ་ཨེསི་ཨོ་ཨའུཊི་པུཊི་ཚུ་ འོག་ལུ་ གསོག་འཇོག་འབད་དེ་ མེར་ཀོལ་ཐིམ་ ཚུ་ ཐོ་བཀོད་འབད།
  `benchmarks/merkle_threshold/`; འདི་ གློག་ཤུགས་འཕྲུལ་ཆས་འདི་གིས་ ཤེས་ཡོད་པའི་ཡིག་སྣོད་ཚུ་ཐོ་བཀོད་འབདཝ་ཨིན་ དེ་འབདཝ་ལས་ རྩིས་ཞིབ་འབདཝ་ཨིན།
  cross-རྒྱབ་རྟེན་ CPU དང་ GPU འཐོབ་ཚུགསཔ་ཨིན།

## FASTPQ གོ་རིམ་ ༧ ཇི་པི་ཡུ་བེན་ཀ་མར།

| བུནཌལ་ | རྒྱབ་ལྗོངས་ | ཐབས་ལམ་ | GPU རྒྱབ་ལྗོངས། | GPU འཐོབ་ཚུགསཔ་ | ཐབས་འཕྲུལ་འཛིན་གྲྭ་ | GPU | LDE ms (CPU/GPU/) | པོ་སི་ཌོན་ ms (CPU/GPU/SU) |
| | ------|-----|----------------------|---------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | ཀུ་དཱ | གཔུ་ | ཅུད་-སམ་༨༠ | ཡིན། | xeon-rtx | NVIDIA RTX 6000 ཨ་ཌ | ༡༥༡༢.༩/༨༨༠.༧/༡.༧༢ | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | ལྕགས་ | གཔུ་ | ནོ་ | ཡིན། | ཨེ་པཱལ་-ཨེམ་༤ | ཨེ་པཱལ་ཇི་པི་ཡུ་ ༤༠-ཀོར་ | ༧༨༥.༦/༧༣༥.༦/༡.༠༧ | ༡༨༠༣.༨/༡༨༩༧.༥/༠.༩༥ |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | ལྕགས་ | གཔུ་ | ལྕགས་ | ཡིན། | apple-m2-ultra | ཨེ་པཱལ་ཨེམ་༢ ཨུལ་ཊར། | ༡༥༨༡.༡/༡༦༠༤.༥/༠.༩༨ | ༣༥༨༩.༩/༣༦༩༧.༣/༠.༩༧ |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | ལྕགས་ | གཔུ་ | ལྕགས་ | ཡིན། | apple-m2-ultra | ཨེ་པཱལ་ཨེམ་༢ ཨུལ་ཊར། | ༡༨༠༤.༥/༡༦༦༦.༤/༡.༠༨ | ༣༩༣༩.༥/༤༠༨༣.༣/༠.༩༦ |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | ལྕགས་ | གཔུ་ | ལྕགས་ | ཡིན། | apple-m2-ultra | ཨེ་པཱལ་ཨེམ་༢ ཨུལ་ཊར། | ༡༨༠༤.༥/༡༦༦༦.༤/༡.༠༨ | ༣༩༣༩.༥/༤༠༨༣.༣/༠.༩༦ |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | ཁ་ཕྱེ་ | གཔུ་ | ཁ་ཕྱེ་ | ཡིན། | neoverse-mi300 | ཨེ་ཨེམ་ཌི་ རང་གཤིས་ MI300A | ༤༥༡༨.༥/༦༨༨.༩/༦.༥༦ | ༢༧༨༠.༤/༩༠༥.༦/༣.༠༧ |

> ཀེར་ཐིག་ཚུ་: `Backend` འདི་ བཱན་ཌལ་མིང་ལས་འབྱུང་ཡོདཔ་ཨིན། `Mode`/`GPU backend`/`GPU available`
> སི་པི་ཡུ་ ཕོལ་བེག་ཚུ་ ཕྱིར་བཏོན་འབད་ནི་དང་ ཡང་ན་ ཇི་པི་ཡུ་ མ་ཐོབ་ནིའི་དོན་ལུ་ བཀབ་བཞག་མི་ `benchmarks` སྡེབ་ཚན་ལས་ འདྲ་བཤུས་རྐྱབ་ཡོདཔ་ཨིན།
> གསར་འཚོལ་ (དཔེར་ན་ `gpu_backend=none` `Mode=gpu`) ཡིན་ནའང་།) SU = མགྱོགས་ཚད་ཆ་ཚད་ (སི་པི་ཡུ་/ཇི་པི་ཡུ་)།

## པོ་སི་ཌོན་ མའི་ཀོརོ་བེནཆ་ པར་ལེན་ཚུ།

`benchmarks/poseidon/manifest.json` གིས་ སྔོན་སྒྲིག་-vs-scalar པོ་སི་ཌོན་འདི་ བསྡོམས་འབདཝ་ཨིན།
ལྕགས་རིགས་ཀྱི་ བང་རིམ་རེ་རེ་ལས་ ཕྱིར་ཚོང་འཐབ་མི་ མའི་ཀོ་བེནཆ་ཚུ་ གཡོག་བཀོལཝ་ཨིན། འོག་གི་ཐིག་ཁྲམ་འདི་ ༡ གིས་ གསར་བསྐྲུན་འབད་ཡོདཔ་ཨིན།
གློག་ཤུགས་འཕྲུལ་ཆས་ཀྱི་ཡིག་ཆ་འདི་ དེ་འབདཝ་ལས་ CI དང་གཞུང་སྐྱོང་བསྐྱར་ཞིབ་ཚུ་གིས་ བྱུང་རབས་ཀྱི་མགྱོགས་ཚད་ཚུ་ དབྱེ་བ་ཕྱེ་ཚུགས།
བཀབ་ཡོད་པའི་ FASTPQ སྙན་ཞུ་ཚུ་ མ་ཕྱེ་བར་ཡོདཔ།

| བཅུད་བསྡུས་ | བུནཌལ་ | དུས་ཚོད་མཚོན་རྟགས་ | སྔོན་སྒྲིག་ཨེམ་ཨེསི་ | སཀ་ལར་ཨེམ་ཨེསི་ | གསུང་བཤད་ |
|----------------------------------------------------------------- |།།།
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | ༢༡༥༢.༢ | ༠.༩༩ |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | ༡༩༩༠.༥ | ༡༩༩༤.༥ | ༡.༠༠ |

## མེར་ཀལ་ཐེརེས་གཤོག་པ།གཞི་བསྟུན་འཛིན་བཟུང་ཚུ་ བརྒྱུད་དེ་བསྡུ་ལེན་འབད་ཡོདཔ།
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
`benchmarks/merkle_threshold/` གི་འོག་ལུ་སྡོད་ཡོདཔ་ཨིན། ཐོ་ཡིག་ཐོ་བཀོད་ཚུ་གིས་ ཧོསིཊི་འདི་ཨིན་ན་མེན་ན།
ཕྱགས་བརྡར་འབད་བའི་སྐབས་ ཕྱི་ཁར་ཐོན་པའི་ལྕགས་རིགས་ཚུ། ཇི་པི་ཡུ་ལྕོགས་ཅན་བཟུང་མི་ཚུ་གིས་སྙན་ཞུ་འབད་དགོ།
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

ཨེ་པཱལ་སི་ལི་ཀོན་གྱི་འཛིན་བཟུང་ (`takemiyacStudio.lan_25.0.0_arm64`) འདི་ `docs/source/benchmarks.md` ནང་ལག་ལེན་འཐབ་མི་ ཀེ་ནོ་ནིག་ཇི་པི་ཡུ་གཞི་རྟེན་ཨིན། macOS 14 ཐོ་བཀོད་ཚུ་ ལྕགས་རིགས་ཚུ་ ཕྱིར་བཏོན་འབད་མ་ཚུགས་པའི་ མཐའ་འཁོར་ཚུ་གི་དོན་ལུ་ སི་པི་ཡུ་རྐྱངམ་གཅིག་གི་གཞི་རྟེན་སྦེ་ ལུས་ཡོདཔ་ཨིན།

## གྲལ་ཐིག་ལག་ལེན་གྱི་ཐིག་ལེན།

`scripts/fastpq/check_row_usage.py` བརྒྱུད་དེ་ དཔང་པོ་ཡོད་པའི་ ཌེ་ཀོཌ།
gadget’s གྲལ་ཐིག་འཇོན་ཐང་། JSON ཅ་མཛོད་ཚུ་ `artifacts/fastpq_benchmarks/` གི་འོག་ལུ་བཞག།
དང་ འ་ནི་གློག་ཤུགས་འཕྲུལ་ཆས་འདི་གིས་ རྩིས་ཞིབ་པ་ཚུ་གི་དོན་ལུ་ ཐོ་བཀོད་འབད་ཡོད་པའི་ གནས་སོར་གྱི་ཆ་ཚད་ཚུ་ བཅུད་བསྡུ་འོང་།

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — batches=2, transform_ratio avg=0.629 (min=0.625, max=0.633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — batches=2, transfort_ratio avg=0.619 (min=0.613, max=0.625)