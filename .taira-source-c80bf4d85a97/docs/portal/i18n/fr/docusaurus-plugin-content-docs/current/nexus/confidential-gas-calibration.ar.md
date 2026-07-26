---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : سجل معايرة الغاز السري
description: قياسات بجودة الاصدار تدعم جدول الغاز السري.
limace : /nexus/confidential-gas-calibration
---

# خطوط اساس لمعايرة الغاز السري

يتتبع هذا السجل المخرجات المعتمدة لمعايرات معايرة الغاز السري. Vous trouverez ci-dessous des informations sur les actifs confidentiels et les transferts ZK (./confidential-assets#calibration-baselines--acceptance-gates).

| التاريخ (UTC) | S'engager | الملف التعريفي | `ns/op` | `gas/op` | `ns/gas` | الملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (infohôte); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018` ; `cargo test -p iroha_core bench_repro -- --ignored` ; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5` ; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | en attente | baseline-simd-neutre | - | - | - | تشغيل محايد x86_64 مجدول على مضيف CI `bench-x86-neon0`; انظر التذكرة GAS-214. Il s'agit d'un banc d'essai (avant la fusion depuis la version 2.1). |
| 2026-04-13 | en attente | ligne de base-avx2 | - | - | - | AVX2 est utilisé pour commettre/construire pour la mise à jour تتطلب المضيف `bench-x86-avx2a`. Le GAS-214 est compatible avec le `baseline-neon`. |

`ns/op` يجمع الوسيط لوقت الجدار لكل تعليمة مقاس بواسطة Critère؛ `gas/op` et `iroha_core::gas::meter_instruction`; و`ns/gas` يقسم مجموع النانوثانية على مجموع الغاز عبر مجموعة التعليمات التسع.*ملاحظة.* المضيف arm64 الحالي لا يصدر ملخصات Critère `raw.csv` pour افتراضي؛ Il s'agit de l'`CRITERION_OUTPUT_TO=csv` et des artefacts en amont pour les artefacts. `target/criterion/` est compatible avec `--save-baseline` pour Linux et Linux. الكونسول في حزمة الاصدار كحل مؤقت. Vous avez utilisé arm64 pour créer un lien vers `docs/source/confidential_assets_calibration_neon_20251018.log`.

Liens vers le site Web (`cargo bench -p iroha_core --bench isi_gas_calibration`) :

| التعليمة | الوسيط `ns/op` | `gas` | `ns/gas` |
| --- | --- | --- | --- |
| S'inscrireDomaine | 3.46e5 | 200 | 1.73e3 |
| S'inscrireCompte | 3.15e5 | 200 | 1.58e3 |
| S'inscrireAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRôle | 3.33e5 | 96 | 3.47e3 |
| Révoquer le rôle de compte | 3.12e5 | 96 | 3.25e3 |
| ExécuterTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| Transfert d'actifs | 3.68e5 | 180 | 2.04e3 |

عمود الجدول مفروض بواسطة `gas::tests::calibration_bench_gas_snapshot` (اجمالي 1,413 غاز عبر مجموعة التعليمات التسع) et اذا غيرت Les matchs à venir de la ville de قياس دون تحديث rencontres.