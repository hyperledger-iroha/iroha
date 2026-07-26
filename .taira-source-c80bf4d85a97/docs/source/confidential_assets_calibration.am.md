---
lang: am
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ሚስጥራዊ የጋዝ መለኪያ መሰረታዊ መስመሮች

ይህ ደብተር ሚስጥራዊ የጋዝ መለኪያውን የተረጋገጡ ውጤቶችን ይከታተላል
መለኪያዎች. እያንዳንዱ ረድፍ የተቀረጸውን የመልቀቂያ-ጥራት መለኪያ ስብስብ ያቀርባል
በ `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates` ውስጥ የተገለጸው አሰራር.

| ቀን (UTC) | ቁርጠኝነት | መገለጫ | `ns/op` | `gas/op` | `ns/gas` | ማስታወሻ |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | መነሻ-ኒዮን | 2.93e5 | 1.57e2 | 1.87e3 | ዳርዊን 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | መነሻ-ኒዮን-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | ዳርዊን 25.0.0 arm64 (`rustc 1.91.0`)። ትዕዛዝ: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; በ `docs/source/confidential_assets_calibration_neon_20260428.log` ላይ ይግቡ። x86_64 እኩል ሩጫዎች (SIMD-ገለልተኛ + AVX2) ለ2026-03-19 ዙሪክ ላብራቶሪ ማስገቢያ መርሐግብር ተይዞላቸዋል። ቅርሶች በ `artifacts/confidential_assets_calibration/2026-03-x86/` ስር የሚያርፉ ተዛማጅ ትዕዛዞች እና አንዴ ከተያዙ ወደ መነሻ ሰንጠረዥ ይቀላቀላሉ። |
| 2026-04-28 | - | መነሻ-ሲምድ-ገለልተኛ | - | - | - | ** የተሰረዘ ** በ Apple Silicon - `ring` NEON ን ለመድረኩ ኤቢአይ ያስገድዳል ፣ ስለዚህ `RUSTFLAGS="-C target-feature=-neon"` አግዳሚ ወንበር ከመጀመሩ በፊት አይሳካም (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`)። በ CI አስተናጋጅ `bench-x86-neon0` ላይ ገለልተኛ መረጃ ተዘግቶ ይቆያል። |
| 2026-04-28 | - | መነሻ-avx2 | - | - | - | **የዘገየ** x86_64 ሯጭ እስኪገኝ ድረስ። `arch -x86_64` በዚህ ማሽን ላይ ሁለትዮሽዎችን መፍጠር አይችልም ("መጥፎ የሲፒዩ አይነት በ executable"፤ `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log` ይመልከቱ)። የCI አስተናጋጅ `bench-x86-avx2a` የመዝገብ ምንጭ ሆኖ ይቆያል። |

`ns/op` በየመስፈርት በሚለካው የመካከለኛውን ግድግዳ ሰዓቱን ይሰበስባል;
`gas/op` ተዛማጅ የጊዜ ሰሌዳ ወጪዎች የሂሳብ አማካይ ነው
`iroha_core::gas::meter_instruction`; `ns/gas` የተጠቃለሉ ናኖሴኮንዶችን በ ይከፋፍላቸዋል
በዘጠኙ የመማሪያ ናሙና ስብስብ ውስጥ የተጨመረው ጋዝ.

*ማስታወሻ።* የአሁኑ arm64 አስተናጋጅ መስፈርት `raw.csv` ማጠቃለያዎችን አያወጣም
ሳጥኑ; መለያ ከመስጠትዎ በፊት በ`CRITERION_OUTPUT_TO=csv` ወይም በላይ ዥረት ማስተካከል እንደገና ያሂዱ ሀ
በቅበላ ማረጋገጫው ዝርዝር የሚፈለጉት ቅርሶች ተያይዘዋል።
`target/criterion/` አሁንም ከ `--save-baseline` በኋላ የሚጎድል ከሆነ ሩጫውን ይሰብስቡ
በሊኑክስ አስተናጋጅ ላይ ወይም የኮንሶል ውጤቱን ወደ ተለቀቀው ጥቅል እንደ ሀ
ጊዜያዊ የማቆሚያ ክፍተት. ለማጣቀሻ፣ የ arm64 ኮንሶል ምዝግብ ማስታወሻ ከቅርቡ ሩጫ
በ `docs/source/confidential_assets_calibration_neon_20251018.log` ይኖራል።

ከተመሳሳይ ሩጫ (`cargo bench -p iroha_core --bench isi_gas_calibration`) በአንድ መመሪያ ውስጥ ያሉ አማካዮች፡-

| መመሪያ | ሚዲያን `ns/op` | መርሐግብር `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| ይመዝገቡ መለያ | 3.15e5 | 200 | 1.58e3 |
| ይመዝገቡAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| መለያ ሚናን መሻር | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_ባዶ_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

### 2026-04-28 (Apple Silicon፣ NEON ነቅቷል)

ለ2026-04-28 አድስ (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`) ሚዲያን መዘግየት፡-| መመሪያ | ሚዲያን `ns/op` | መርሐግብር `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 8.58e6 | 200 | 4.29e4 |
| ይመዝገቡ መለያ | 4.40e6 | 200 | 2.20e4 |
| ይመዝገቡAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| መለያ ሚናን መሻር | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_ባዶ_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

ከላይ ባለው ሠንጠረዥ ውስጥ `ns/op` እና `ns/gas` ድምር የተገኙት ከ ድምር ነው።
እነዚህ ሚዲያን (ጠቅላላ `3.85717e7`ns በመላው ዘጠኝ መመሪያ ስብስብ እና 1,413
የጋዝ ክፍሎች).

የጊዜ ሰሌዳው አምድ በ `gas::tests::calibration_bench_gas_snapshot` ተፈጻሚ ነው።
(በአጠቃላይ 1,413 ጋዝ በዘጠኙ መመሪያ ስብስብ) እና ወደፊት ከተጣበቀ ይወድቃል
የመለኪያ መሳሪያዎችን ሳያዘምኑ መለኪያን ይቀይሩ.

## የቁርጠኝነት ዛፍ ቴሌሜትሪ ማስረጃ (M2.2)

በእያንዳንዱ የመንገድ ካርታ ተግባር **M2.2**፣ እያንዳንዱ የካሊብሬሽን ሩጫ አዲሱን መያዝ አለበት።
የመርክል ድንበር መቆየቱን ለማረጋገጥ ቁርጠኝነት-የዛፍ መለኪያዎች እና የማስወጣት ቆጣሪዎች
በተዋቀሩ ወሰኖች ውስጥ፡-

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

ከማስተካከያው የሥራ ጫና በፊት እና በኋላ ወዲያውኑ እሴቶቹን ይመዝግቡ። ሀ
ነጠላ ትዕዛዝ በንብረት ላይ በቂ ነው; ምሳሌ ለ `4cuvDVPuLBKJyN6dPbRQhmLh68sU`፡

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

የጥሬውን ውጤት (ወይም Prometheus ቅጽበተ ፎቶ) ከመለኪያ ትኬቱ ጋር ያያይዙ
የአስተዳደር ገምጋሚ የስር-ታሪክ ክዳን እና የፍተሻ ነጥብ ክፍተቶች መሆናቸውን ማረጋገጥ ይችላል።
ተከበረ። የቴሌሜትሪ መመሪያ በ `docs/source/telemetry.md#confidential-tree-telemetry-m22`
የሚጠበቁትን እና የተቆራኙ Grafana ፓነሎችን በማስጠንቀቅ ላይ ያሰፋል።

ገምጋሚዎች ማረጋገጥ እንዲችሉ የማረጋገጫ መሸጎጫ ቆጣሪዎችን በተመሳሳይ መቧጨር ያካትቱ
የጠፋው ጥምርታ ከ40% የማስጠንቀቂያ ገደብ በታች ቆየ፡-

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

የተገኘውን ሬሾ (`miss / (hit + miss)`) በማስታወሻው ውስጥ ይመዝግቡ
ከሲምዲ-ገለልተኛ ወጪ ሞዴል ልምምዶች ይልቅ እንደገና ጥቅም ላይ የዋሉ የሞቀ መሸጎጫዎችን ለማሳየት
የHalo2 አረጋጋጭ መዝገብ ቤትን በማፍረስ ላይ።

## ገለልተኛ እና AVX2 ነፃ

የኤስዲኬ ካውንስል ለPhaseC በር ለሚያስፈልገው ጊዜያዊ መልቀቂያ ሰጠ
`baseline-simd-neutral` እና `baseline-avx2` መለኪያዎች፡-

- ** SIMD-ገለልተኛ:** በአፕል ሲሊኮን ላይ `ring` crypto backend NEON ለ
  የ ABI ትክክለኛነት። ባህሪውን በማሰናከል ላይ (`RUSTFLAGS="-C target-feature=-neon"`)
  የቤንች ሁለትዮሽ (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`) ከመፈጠሩ በፊት ግንባታውን ያቋርጣል.
- ** AVX2: ** የአካባቢው የመሳሪያ ሰንሰለት x86_64 ሁለትዮሾችን (`arch -x86_64 rustc -V`) ማመንጨት አይችልም
  → "መጥፎ የሲፒዩ አይነት በ executable"; ተመልከት
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`)።

CI አስተናጋጆች `bench-x86-neon0` እና `bench-x86-avx2a` ኦንላይን እስኪሆኑ ድረስ NEON ይሰራል
ከላይ እና የቴሌሜትሪ ማስረጃዎች የPhaseC ተቀባይነት መስፈርቶችን ያሟላሉ።
ማቋረጡ በ`status.md` ውስጥ ተመዝግቧል እና x86 ሃርድዌር ከተጠናቀቀ በኋላ እንደገና ይጎበኛል።
ይገኛል ።