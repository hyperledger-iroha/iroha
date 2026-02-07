---
lang: am
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: db113723dbedefdb89333e2e8fb1483aed9b5603cd2e9bb93fb46f7206533518
source_last_modified: "2025-12-29T18:16:35.121357+00:00"
translation_last_reviewed: 2026-02-07
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
slug: /nexus/confidential-gas-calibration
translator: machine-google-reviewed
---

# ሚስጥራዊ የጋዝ መለኪያ መሰረታዊ መስመሮች

ይህ ደብተር ሚስጥራዊ የጋዝ መለኪያውን የተረጋገጡ ውጤቶችን ይከታተላል
መለኪያዎች. እያንዳንዱ ረድፍ የተቀረጸውን የመልቀቂያ-ጥራት መለኪያ ስብስብ ያቀርባል
በ [ሚስጥራዊ ንብረቶች እና ZK ዝውውሮች] (./confidential-assets#calibration-baselines--acceptance-gates) ውስጥ የተገለጸው አሰራር።

| ቀን (UTC) | ቁርጠኝነት | መገለጫ | `ns/op` | I18NI0000002X | `ns/gas` | ማስታወሻ |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | መነሻ-ኒዮን | 2.93e5 | 1.57e2 | 1.87e3 | ዳርዊን 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | በመጠባበቅ ላይ | መነሻ-ሲምድ-ገለልተኛ | - | - | - | በ CI አስተናጋጅ I18NI0000008X ላይ x86_64 ገለልተኛ ሩጫ; ቲኬት GAS-214 ይመልከቱ. የቤንች መስኮቱ እንደተጠናቀቀ ውጤቶች ይታከላሉ (ቅድመ-ውህደት የፍተሻ ዝርዝር ዒላማዎች ልቀት 2.1)። |
| 2026-04-13 | በመጠባበቅ ላይ | መነሻ-avx2 | - | - | - | ልክ እንደ ገለልተኛ አሂድ ተመሳሳይ ቁርጠኝነት/ግንባታ በመጠቀም የAVX2 ልኬትን መከታተል። አስተናጋጅ `bench-x86-avx2a` ይፈልጋል። GAS-214 ሁለቱንም በዴልታ ንጽጽር ከ `baseline-neon` ጋር ይሸፍናል። |

`ns/op` በመመዘኛ በሚለካው መመሪያ የመካከለኛውን ግድግዳ ሰዓቱን ይሰበስባል;
`gas/op` ተዛማጅ የጊዜ ሰሌዳ ወጪዎች የሂሳብ አማካይ ነው
`iroha_core::gas::meter_instruction`; `ns/gas` የተጠቃለሉ ናኖሴኮንዶችን በ ይከፋፍላቸዋል
በዘጠኙ የመማሪያ ናሙና ስብስብ ውስጥ የተጨመረው ጋዝ.

*ማስታወሻ።* የአሁኑ arm64 አስተናጋጅ መስፈርት `raw.csv` ማጠቃለያዎችን አያወጣም
ሳጥኑ; ሀ መለያ ከመስጠትዎ በፊት በ`CRITERION_OUTPUT_TO=csv` ወይም በላይ ዥረት ማስተካከል እንደገና ያሂዱ
በቅበላ ማረጋገጫው ዝርዝር የሚፈለጉት ቅርሶች ተያይዘዋል።
`target/criterion/` አሁንም ከ I18NI0000018X በኋላ የሚጎድል ከሆነ ሩጫውን ይሰብስቡ
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

የጊዜ ሰሌዳው አምድ በ `gas::tests::calibration_bench_gas_snapshot` ተፈጻሚ ነው።
(በአጠቃላይ 1,413 ጋዝ በዘጠኙ መመሪያ ስብስብ) እና ወደፊት ከተጣበቀ ይወድቃል
የመለኪያ መሳሪያዎችን ሳያዘምኑ መለኪያን ይቀይሩ.