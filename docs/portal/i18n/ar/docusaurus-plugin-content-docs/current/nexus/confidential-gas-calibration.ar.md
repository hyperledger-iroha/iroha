---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: سجل معايرة الغاز السري
description: قياسات بجودة الاصدار تدعم جدول الغاز السري.
slug: /nexus/confidential-gas-calibration
---

# خطوط اساس لمعايرة الغاز السري

يتتبع هذا السجل المخرجات المعتمدة لمعايرات معايرة الغاز السري. كل صف يوثق مجموعة قياسات بجودة اصدار تم التقاطها وفق الاجراء الموضح في [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| التاريخ (UTC) | Commit | الملف التعريفي | `ns/op` | `gas/op` | `ns/gas` | الملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | - | - | - | تشغيل محايد x86_64 مجدول على مضيف CI `bench-x86-neon0`; انظر التذكرة GAS-214. ستضاف النتائج بعد اكتمال نافذة bench (قائمة pre-merge تستهدف اصدار 2.1). |
| 2026-04-13 | pending | baseline-avx2 | - | - | - | معايرة AVX2 لاحقة باستخدام نفس commit/build للتشغيل المحايد؛ تتطلب المضيف `bench-x86-avx2a`. تغطي GAS-214 التشغيلين مع مقارنة الفروقات مقابل `baseline-neon`. |

`ns/op` يجمع الوسيط لوقت الجدار لكل تعليمة مقاس بواسطة Criterion؛ `gas/op` هو المتوسط الحسابي لكلف الجدول المقابلة من `iroha_core::gas::meter_instruction`; و`ns/gas` يقسم مجموع النانوثانية على مجموع الغاز عبر مجموعة التعليمات التسع.

*ملاحظة.* المضيف arm64 الحالي لا يصدر ملخصات Criterion `raw.csv` بشكل افتراضي؛ اعد التشغيل مع `CRITERION_OUTPUT_TO=csv` او اصلاح upstream قبل وسم الاصدار لكي يتم ارفاق artefacts المطلوبة في قائمة القبول. اذا كان `target/criterion/` ما زال مفقودا بعد `--save-baseline` فاجمع التشغيل على مضيف Linux او قم بتسلسل مخرجات الكونسول في حزمة الاصدار كحل مؤقت. كمرجع، سجل الكونسول arm64 لاخر تشغيل موجود في `docs/source/confidential_assets_calibration_neon_20251018.log`.

الوسيطات لكل تعليمة من نفس التشغيل (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| التعليمة | الوسيط `ns/op` | جدول `gas` | `ns/gas` |
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

عمود الجدول مفروض بواسطة `gas::tests::calibration_bench_gas_snapshot` (اجمالي 1,413 غاز عبر مجموعة التعليمات التسع) وسيفشل اذا غيرت التصحيحات المستقبلية القياس دون تحديث fixtures المعايرة.
