---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: سجل معايرة الغاز السري
descripción: قياسات بجودة الاصدار تدعم جدول الغاز السري.
babosa: /nexus/calibracion-de-gas-confidencial
---

# خطوط اساس لمعايرة الغاز السري

يتتبع هذا السجل المخرجات المعتمدة لمعايرات معايرة الغاز السري. كل صف يوثق مجموعة قياسات بجودة اصدار تم التقاطها وفق الاجراء الموضح في [Activos confidenciales y transferencias ZK] (./confidential-assets#calibration-baselines--acceptance-gates).

| التاريخ (UTC) | Comprometerse | الملف التعريفي | `ns/op` | `gas/op` | `ns/gas` | الملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | línea de base-neón | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (información del host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pendiente | línea base-simd-neutral | - | - | - | تشغيل محايد x86_64 مجدول على مضيف CI `bench-x86-neon0`; Utilice el GAS-214. ستضاف النتائج بعد اكتمال نافذة bench (قائمة pre-merge تستهدف اصدار 2.1). |
| 2026-04-13 | pendiente | línea de base-avx2 | - | - | - | معايرة AVX2 لاحقة باستخدام نفس commit/build للتشغيل المحايد؛ Utilice el código `bench-x86-avx2a`. El GAS-214 está equipado con un filtro de aire `baseline-neon`. |

`ns/op` يجمع الوسيط لوقت الجدار لكل تعليمة مقاس بواسطة Criterion؛ `gas/op` هو المتوسط ​​الحسابي لكلف الجدول المقابلة من `iroha_core::gas::meter_instruction`; و`ns/gas` يقسم مجموع النانوثانية على مجموع الغاز عبر مجموعة التعليمات التسع.*ملاحظة.* المضيف arm64 الحالي لا يصدر ملخصات Criterio `raw.csv` بشكل افتراضي؛ اعد التشغيل مع `CRITERION_OUTPUT_TO=csv` او اصلاح upstream قبل وسم الاصدار لكي يتم ارفاق artefactos المطلوبة في قائمة القبول. Aquí está `target/criterion/`, que está instalado en Linux y que está instalado en Linux. الكونسول في حزمة الاصدار كحل مؤقت. Utilice arm64 para conectar el dispositivo a `docs/source/confidential_assets_calibration_neon_20251018.log`.

Nombre del usuario (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| التعليمة | Idioma `ns/op` | Número `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDominio | 3.46e5 | 200 | 1.73e3 |
| RegistrarseCuenta | 3.15e5 | 200 | 1.58e3 |
| RegistrarAssetDef | 3.41e5 | 200 | 1.71e3 |
| Establecer cuentaKV_small | 3.28e5 | 67 | 4.90e3 |
| Función de cuenta de concesión | 3.33e5 | 96 | 3.47e3 |
| Revocar función de cuenta | 3.12e5 | 96 | 3.25e3 |
| EjecutarTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| Activo de menta | 1.56e5 | 150 | 1.04e3 |
| Transferir activo | 3.68e5 | 180 | 2.04e3 |

Ver más التصحيحات المستقبلية القياس دون تحديث accesorios المعايرة.