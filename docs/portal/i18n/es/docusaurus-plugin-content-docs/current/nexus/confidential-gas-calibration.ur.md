---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: خفیہ گیس کیلیبریشن لیجر
descripción: ریلیز معیار کی پیمائشیں جو خفیہ گیس شیڈول کی پشت پناہی کرتی ہیں۔
babosa: /nexus/calibracion-de-gas-confidencial
---

# خفیہ گیس کیلیبریشن بیس لائنز

یہ لیجر خفیہ گیس کیلیبریشن بینچ مارکس کے تصدیق شدہ نتائج ٹریک کرتا ہے۔ ہر قطار ریلیز معیار کی پیمائشوں کا سیٹ دستاویز کرتی ہے جو [Activos confidenciales y transferencias ZK](./confidential-assets#calibration-baselines--acceptance-gates) میں بیان کردہ طریقہ کار سے حاصل کیا گیا تھا۔

| تاریخ (UTC) | Comprometerse | پروفائل | `ns/op` | `gas/op` | `ns/gas` | نوٹس |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | línea de base-neón | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (información del host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pendiente | línea base-simd-neutral | - | - | - | Host CI `bench-x86-neon0` پر x86_64 نیوٹرل رن شیڈیول ہے؛ ٹکٹ GAS-214 دیکھیں۔ نتائج banco ونڈو مکمل ہونے پر شامل ہوں گے (pre-fusionado چیک لسٹ ریلیز 2.1 کو ہدف بناتی ہے)۔ |
| 2026-04-13 | pendiente | línea de base-avx2 | - | - | - | نیوٹرل رن کے اسی commit/build کے ساتھ فالو اپ AVX2 کیلیبریشن؛ host `bench-x86-avx2a` درکار ہے۔ GAS-214 دونوں رنز کو `baseline-neon` کے مقابلے ڈیلٹا کمپیریزن کے ساتھ کور کرتا ہے۔ |`ns/op` Criterio کے ذریعے ماپا گیا فی انسٹرکشن وال کلاک میڈین agregado کرتا ہے؛ `gas/op` `iroha_core::gas::meter_instruction` کے متعلقہ شیڈول اخراجات کا حسابی اوسط ہے؛ `ns/gas` نو انسٹرکشن سیٹ کے مجموعی نینو سیکنڈز کو مجموعی گیس سے تقسیم کرتا ہے۔

*نوٹ.* موجودہ arm64 host ڈیفالٹ طور پر Criterion `raw.csv` خلاصے emit نہیں کرتا؛ Lista de verificación de aceptación de lista de verificación de aceptación de lista de verificación de aceptación کے مطلوبہ artefactos منسلک ہوں۔ اگر `target/criterion/` `--save-baseline` کے بعد بھی غائب ہو تو Host Linux پر رن جمع کریں یا کنسول آؤٹ پٹ کو ریلیز paquete میں serializar کر دیں بطور عارضی recurso provisional۔ حوالہ کے طور پر، تازہ ترین رن کا arm64 کنسول لاگ `docs/source/confidential_assets_calibration_neon_20251018.log` میں موجود ہے۔

اسی رن سے فی انسٹرکشن میڈینز (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instrucción | mediana `ns/op` | horario `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDominio | 3.46e5 | 200 | 1.73e3 |
| RegistrarseCuenta | 3.15e5 | 200 | 1.58e3 |
| RegistrarAssetDef | 3.41e5 | 200 | 1.71e3 |
| Establecer cuentaKV_small | 3.28e5 | 67 | 4.90e3 |
| Función de cuenta de concesión | 3.33e5 | 96 | 3.47e3 |
| Revocar función de cuenta | 3.12e5 | 96 | 3.25e3 |
| EjecutarTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| Activo de menta | 1.56e5 | 150 | 1.04e3 |
| Transferir activo | 3.68e5 | 180 | 2.04e3 |شیڈول کالم `gas::tests::calibration_bench_gas_snapshot` کے ذریعے نافذ ہوتا ہے (نو انسٹرکشن سیٹ میں کل 1,413 گیس) اور اگر آئندہ پیچز میٹرنگ کو بدل دیں مگر کیلیبریشن فکسچرز اپ ڈیٹ نہ ہوں تو ٹیسٹ فیل ہو جائے گا۔