---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: خفیہ گیس کیلیبریشن لیجر
description: ریلیز معیار کی پیمائشیں جو خفیہ گیس شیڈول کی پشت پناہی کرتی ہیں۔
slug: /nexus/confidential-gas-calibration
---

# خفیہ گیس کیلیبریشن بیس لائنز

یہ لیجر خفیہ گیس کیلیبریشن بینچ مارکس کے تصدیق شدہ نتائج ٹریک کرتا ہے۔ ہر قطار ریلیز معیار کی پیمائشوں کا سیٹ دستاویز کرتی ہے جو [Ativos confidenciais e transferências ZK](./confidential-assets#calibration-baselines--acceptance-gates) میں بیان کردہ طریقہ کار سے حاصل کیا گیا تھا۔

| Tempo (UTC) | Confirmar | پروفائل | `ns/op` | `gas/op` | `ns/gas` | Não |
| --- | --- | --- | --- | --- | --- | --- |
| 18/10/2025 | 3c70a7d3 | linha de base-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (informações do host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 12/04/2026 | pendente | linha de base-simd-neutro | - | - | - | Host CI `bench-x86-neon0` پر x86_64 نیوٹرل رن شیڈیول ہے؛ ٹکٹ GAS-214 دیکھیں۔ نتائج bench ونڈو مکمل ہونے پر شامل ہوں گے (pre-merge چیک لسٹ ریلیز 2.1 کو ہدف بناتی ہے)۔ |
| 13/04/2026 | pendente | linha de base-avx2 | - | - | - | Não há necessidade de commit/build کے ساتھ فالو اپ AVX2 کیلیبریشن؛ host `bench-x86-avx2a` درکار ہے۔ GAS-214 دونوں رنز کو `baseline-neon` کے مقابلے ڈیلٹا کمپیریزن کے ساتھ کور کرتا ہے۔ |

Critério `ns/op` کے ذریعے ماپا گیا فی انسٹرکشن وال کلاک ماپا گیا فی انسٹرکشن وال کلاک میڈین agregado کرتا ہے؛ `gas/op` `iroha_core::gas::meter_instruction` کے متعلقہ شیڈول اخراجات کا حسابی اوسط ہے؛ `ns/gas` não é um produto de alta qualidade que você pode usar ہے۔

*نوٹ.* موجودہ host arm64 ڈیفالٹ طور پر Critério `raw.csv` خلاصے emitir نہیں کرتا؛ ریلیز ٹیگ کرنے سے پہلے `CRITERION_OUTPUT_TO=csv` کے ساتھ دوبارہ چلائیں یا upstream فکس لگائیں تاکہ aceitação lista de verificação کے مطلوبہ artefatos منسلک ہوں۔ اگر `target/criterion/` `--save-baseline` کے بعد بھی غائب ہو تو Host Linux پر رن جمع کریں یا کنسول آؤٹ پٹ کو ریلیز pacote میں serialize کر دیں بطور عارضی paliativo۔ Você pode usar o arm64 `docs/source/confidential_assets_calibration_neon_20251018.log` para obter o valor de arm64

O que você está procurando é um cartão de crédito (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instrução | mediana `ns/op` | agendar `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDomínio | 3.46e5 | 200 | 1.73e3 |
| Registrar conta | 3.15e5 | 200 | 1.58e3 |
| RegistrarAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevogarAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| Transferir ativo | 3.68e5 | 180 | 2.04e3 |

O valor `gas::tests::calibration_bench_gas_snapshot` é um valor de 1,413 dólares. اگر آئندہ پیچز میٹرنگ کو بدل دیں مگر کیلیبریشن فکسچرز اپ ڈیٹ نہ ہوں تو ٹیسٹ فیل ہو جائے گا۔