---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/nexus/confidential-gas-calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7120a4303f513c0dbedb6d2f26bb2345fc8906d3fcde8b42c3ddc94d4844f070
source_last_modified: "2025-11-14T04:43:20.324171+00:00"
translation_last_reviewed: 2026-01-30
---

# خفیہ گیس کیلیبریشن بیس لائنز

یہ لیجر خفیہ گیس کیلیبریشن بینچ مارکس کے تصدیق شدہ نتائج ٹریک کرتا ہے۔ ہر قطار ریلیز معیار کی پیمائشوں کا سیٹ دستاویز کرتی ہے جو [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates) میں بیان کردہ طریقہ کار سے حاصل کیا گیا تھا۔

| تاریخ (UTC) | Commit | پروفائل | `ns/op` | `gas/op` | `ns/gas` | نوٹس |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | - | - | - | CI host `bench-x86-neon0` پر x86_64 نیوٹرل رن شیڈیول ہے؛ ٹکٹ GAS-214 دیکھیں۔ نتائج bench ونڈو مکمل ہونے پر شامل ہوں گے (pre-merge چیک لسٹ ریلیز 2.1 کو ہدف بناتی ہے)۔ |
| 2026-04-13 | pending | baseline-avx2 | - | - | - | نیوٹرل رن کے اسی commit/build کے ساتھ فالو اپ AVX2 کیلیبریشن؛ host `bench-x86-avx2a` درکار ہے۔ GAS-214 دونوں رنز کو `baseline-neon` کے مقابلے ڈیلٹا کمپیریزن کے ساتھ کور کرتا ہے۔ |

`ns/op` Criterion کے ذریعے ماپا گیا فی انسٹرکشن وال کلاک میڈین aggregate کرتا ہے؛ `gas/op` `iroha_core::gas::meter_instruction` کے متعلقہ شیڈول اخراجات کا حسابی اوسط ہے؛ `ns/gas` نو انسٹرکشن سیٹ کے مجموعی نینو سیکنڈز کو مجموعی گیس سے تقسیم کرتا ہے۔

*نوٹ.* موجودہ arm64 host ڈیفالٹ طور پر Criterion `raw.csv` خلاصے emit نہیں کرتا؛ ریلیز ٹیگ کرنے سے پہلے `CRITERION_OUTPUT_TO=csv` کے ساتھ دوبارہ چلائیں یا upstream فکس لگائیں تاکہ acceptance checklist کے مطلوبہ artefacts منسلک ہوں۔ اگر `target/criterion/` `--save-baseline` کے بعد بھی غائب ہو تو Linux host پر رن جمع کریں یا کنسول آؤٹ پٹ کو ریلیز bundle میں serialize کر دیں بطور عارضی stopgap۔ حوالہ کے طور پر، تازہ ترین رن کا arm64 کنسول لاگ `docs/source/confidential_assets_calibration_neon_20251018.log` میں موجود ہے۔

اسی رن سے فی انسٹرکشن میڈینز (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instruction | median `ns/op` | schedule `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| RegisterAccount | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

شیڈول کالم `gas::tests::calibration_bench_gas_snapshot` کے ذریعے نافذ ہوتا ہے (نو انسٹرکشن سیٹ میں کل 1,413 گیس) اور اگر آئندہ پیچز میٹرنگ کو بدل دیں مگر کیلیبریشن فکسچرز اپ ڈیٹ نہ ہوں تو ٹیسٹ فیل ہو جائے گا۔
