---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: سجل معايرة الغاز السري
description: قياسات بجودة الاصدار تدعم جدول الغاز السري.
slug: /nexus/confidential-gas-calibration
---

# خطوط اساس لمعايرة الغاز السري

يتتبع هذا السجل المخرجات المعتمدة لمعايرات معايرة الغاز السري. كل صف يوثق مجموعة قياسات بجودة اصدار تم التقاطها وفق الاجراء الموضح في [Ativos Confidenciais & ZK Transferências](./confidential-assets#calibration-baselines--acceptance-gates).

| التاريخ (UTC) | Confirmar | الملف التعريفي | `ns/op` | `gas/op` | `ns/gas` | الملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| 18/10/2025 | 3c70a7d3 | linha de base-néon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (informações do host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 12/04/2026 | pendente | linha de base-simd-neutro | - | - | - | تشغيل محايد x86_64 مجدول على مضيف CI `bench-x86-neon0`; Use GAS-214. ستضاف النتائج بعد اكتمال نافذة bench (قائمة pré-mesclar versão 2.1). |
| 13/04/2026 | pendente | linha de base-avx2 | - | - | - | O AVX2 deve ser usado para commit/build para o servidor Verifique o código `bench-x86-avx2a`. O GAS-214 está equipado com um sensor `baseline-neon`. |

`ns/op` يجمع الوسيط لوقت الجدار لكل تعليمة مقاس بواسطة Critério; `gas/op` é um código de barras de `iroha_core::gas::meter_instruction`; و`ns/gas` يقسم مجموع النانوثانية على مجموع الغاز عبر مجموعة التعليمات التسع.

*ملاحظة.* المضيف arm64 الحالي لا يصدر ملخصات Critério `raw.csv` بشكل افتراضي؛ Use o `CRITERION_OUTPUT_TO=csv` e o upstream para obter os artefatos mais importantes do mundo. O `target/criterion/` é um dispositivo de armazenamento `--save-baseline` que funciona no Linux e no Linux Verifique se há algum problema com isso. A versão arm64 do dispositivo é instalada em `docs/source/confidential_assets_calibration_neon_20251018.log`.

O código de barras do seu computador (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| التعليمة | Nome `ns/op` | Cabo `gas` | `ns/gas` |
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

O número de telefone é `gas::tests::calibration_bench_gas_snapshot` (cerca de 1.413 vezes o tamanho do arquivo) e غيرت التصحيحات المستقبلية القياس دون تحديث fixtures المعايرة.