---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : خفیہ گیس کیلیبریشن لیجر
description: ریلیز معیار کی پیمائشیں جو خفیہ گیس شیڈول کی پشت پناہی کرتی ہیں۔
limace : /nexus/confidential-gas-calibration
---

# خفیہ گیس کیلیبریشن بیس لائنز

یہ لیجر خفیہ گیس کیلیبریشن بینچ مارکس کے تصدیق شدہ نتائج ٹریک کرتا ہے۔ [Actifs confidentiels et transferts ZK] (./confidential-assets#calibration-baselines--acceptance-gates) میں بیان کردہ طریقہ کار سے حاصل کیا گیا تھا۔

| تاریخ (UTC) | S'engager | پروفائل | `ns/op` | `gas/op` | `ns/gas` | نوٹس |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (infohôte); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018` ; `cargo test -p iroha_core bench_repro -- --ignored` ; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5` ; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | en attente | baseline-simd-neutre | - | - | - | Hôte CI `bench-x86-neon0` pour x86_64 en anglais Produit GAS-214 Banc de banc d'essai pour la version 2.1 de la version 2.1 (pré-fusion de la version 2.1) |
| 2026-04-13 | en attente | ligne de base-avx2 | - | - | - | Vous avez besoin de commit/build pour AVX2 hôte `bench-x86-avx2a` درکار ہے۔ GAS-214 est un appareil `baseline-neon` qui est en vente libre. |`ns/op` Critère pour l'agrégation de l'agrégation `gas/op` `iroha_core::gas::meter_instruction` کے متعلقہ شیڈول اخراجات کا حسابی اوسط ہے؛ `ns/gas` نو انسٹرکشن سیٹ کے مجموعی نینو سیکنڈز کو مجموعی گیس سے تقسیم کرتا ہے

*نوٹ.* L'hôte arm64 est activé par le critère `raw.csv` qui émet un message d'erreur. Liste de contrôle d'acceptation en amont `CRITERION_OUTPUT_TO=csv` pour la liste de contrôle d'acceptation کے مطلوبہ artefacts منسلک ہوں۔ Le `target/criterion/` `--save-baseline` est un hôte Linux pour votre hôte Linux. ریلیز bundle میں sérialiser کر دیں بطور عارضی palliatif۔ Il s'agit d'un projet de loi sur arm64 pour `docs/source/confidential_assets_calibration_neon_20251018.log` en cours d'exécution.

اسی رن سے فی انسٹرکشن میڈینز (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instructions | médiane `ns/op` | horaire `gas` | `ns/gas` |
| --- | --- | --- | --- |
| S'inscrireDomaine | 3.46e5 | 200 | 1.73e3 |
| S'inscrireCompte | 3.15e5 | 200 | 1.58e3 |
| S'inscrireAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRôle | 3.33e5 | 96 | 3.47e3 |
| Révoquer le rôle de compte | 3.12e5 | 96 | 3.25e3 |
| ExécuterTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| Transfert d'actifs | 3.68e5 | 180 | 2.04e3 |Le prix du produit `gas::tests::calibration_bench_gas_snapshot` représente un montant total de 1 413 dollars (1 413 dollars) pour آئندہ پیچز میٹرنگ کو بدل دیں مگر کیلیبریشن فکسچرز اپ ڈیٹ نہ ہوں تو ٹیسٹ فیل ہو جائے گا۔