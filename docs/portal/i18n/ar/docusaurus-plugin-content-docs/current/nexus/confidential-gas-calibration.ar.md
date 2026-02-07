---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: سجل تجديد الغاز السري
الوصف: قياسات بجودة الاصدار تدعم جدول الغاز السري.
سبيكة: /nexus/Confidential-gas-calibration
---

#تشكيلات أساسية لتعديل الغاز السري

يتتبع هذا السجل المدخلات المعتمدة لمعايرات تجديد الغاز السري. كل صف يوثق مجموعة القياسات بجودة اصدار تم التقاطها وفق الاجراء الموضح في [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| التاريخ (التوقيت العالمي) | الالتزام | الملف التعريفي | `ns/op` | `gas/op` | `ns/gas` | مذكرة |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | خط الأساس النيون | 2.93e5 | 1.57e2 | 1.87e3 | داروين 25.0.0 Arm64e (hostinfo)؛ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | في انتظار | خط الأساس-سيمد-محايد | - | - | - | تشغيل جواد x86_64 مجدول على مضيف CI `bench-x86-neon0`; انظر التذكرة GAS-214. ستتم إضافة النتائج بعد نافذة القانون (قائمة ما قبل الدمج ومنها اصدار 2.1). |
| 2026-04-13 | في انتظار | خط الأساس-avx2 | - | - | - | قم بتعديل AVX2 لاحقًا باستخدام نفس Commit/build لتشغيل الماكينة؛ المتطلبات المحلية `bench-x86-avx2a`. تغطي GAS-214 الكرومين مع مقارنة الفروقات مقابل `baseline-neon`. |

`ns/op` جمع الوسيط لوقت الجدار لكل تعليم بواسطة المعيار؛ `gas/op` هو المعدل الحسابي لكلف قاضي مقابل `iroha_core::gas::meter_instruction`; و`ns/gas` يقسم المجموعة النانوية الثانية على مجموع الغاز عبر مجموعة التعليمات التسعين.

*ملاحظة.* استقبلت Arm64 حاضر لا تصدر ملخصات Criterion `raw.csv` بشكل افتراضي؛ اعد التشغيل مع `CRITERION_OUTPUT_TO=csv` او اصلاح upstream قبل اتصال الاصدار حتى يتم اختفاء القطع الأثرية المطلوبة في قائمة القبول. اذا كان `target/criterion/` ما اكتشفه مفقودا بعد `--save-baseline` فاجمع التشغيل على مضيف لينكس او قم ببطولات المخرج الكونسول في حزمة الاصدار كحل مؤقت. كمرجع، سجل تشغيل الكونسول Arm64 لاخر موجود في `docs/source/confidential_assets_calibration_neon_20251018.log`.

الوسيطات لكل تعليمة من نفس التشغيل (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| التعليم | الوسيط `ns/op` | جدول `gas` | `ns/gas` |
| --- | --- | --- | --- |
| تسجيل النطاق | 3.46e5 | 200 | 1.73e3 |
| تسجيل الحساب | 3.15ه5 | 200 | 1.58e3 |
| تسجيل AssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| دور حساب المنحة | 3.33ه5 | 96 | 3.47e3 |
| إبطال الحساب | 3.12e5 | 96 | 3.25ه3 |
| تنفيذTrigger_empty_args | 1.42e5 | 224 | 6.33ه2 |
| منتأسيت | 1.56e5 | 150 | 1.04e3 |
| نقل الأصول | 3.68e5 | 180 | 2.04e3 |

عمود الجدول مفروض بواسطة `gas::tests::calibration_bench_gas_snapshot` (اجمالي 1,413 غاز عبر مجموعة التعليمات التسعة) سيفشل إذا لم يتم التصحيح المستقبلي للقياس دون تحديث تركيبات التجديد.