---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Livro de calibracao de Gas Confidential
الوصف: أطباء مؤهلون للإفراج عن الغازات بشكل سري.
سبيكة: /nexus/Confidential-gas-calibration
---

# خطوط الأساس لمعايرة الغاز السرية

يرافق هذا السجل النتائج المعتمدة من معايير معايرة الغاز السرية. تتضمن كل وثيقة مجموعة من الأدوية ذات جودة الإصدار الملتقطة مع الإجراء الموصوف في [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| البيانات (التوقيت العالمي) | الالتزام | الملف الشخصي | `ns/op` | `gas/op` | `ns/gas` | نوتاس |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | خط الأساس النيون | 2.93e5 | 1.57e2 | 1.87e3 | داروين 25.0.0 Arm64e (hostinfo)؛ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | في انتظار | خط الأساس-سيمد-محايد | - | - | - | تنفيذ برنامج x86_64 محايد بدون مضيف CI `bench-x86-neon0`; نسخة التذكرة GAS-214. ستتم إضافة النتائج عند نهاية الجدول النهائي (قائمة مرجعية للدمج المسبق أو الإصدار 2.1). |
| 2026-04-13 | في انتظار | خط الأساس-avx2 | - | - | - | مرافقة Calibracao AVX2 تستخدم أو نفس الالتزام/إنشاء التنفيذ المحايد؛ اطلب المضيف `bench-x86-avx2a`. يتم تنفيذ GAS-214 كعمليتين مع مقارنة دلتا بـ `baseline-neon`. |

`ns/op` يجمع وسط ساعة الحائط من خلال التعليمات باستخدام معيار الشعر؛ `gas/op` وتقنية الوسائط المخصصة لجدول المراسلات `iroha_core::gas::meter_instruction`; `ns/gas` يقسم الأجزاء النانوية من الجزء السفلي من الغاز إلى مجموعة من التعليمات الجديدة.

*ملحوظة.* يا مضيف Arm64 لا يصدر استئنافًا `raw.csv` وفقًا للمعيار ؛ ركب بشكل جديد مع `CRITERION_OUTPUT_TO=csv` أو قم بتصحيح المنبع قبل التصريح بإصدار حتى تتمكن هذه المصنوعات الرائعة من وضع قائمة التحقق من الصلاحيات المرفقة. إذا كان `target/criterion/` موجودًا بالفعل بعد `--save-baseline`، فقم بالتنفيذ على مضيف Linux أو قم بإجراء تسلسل لوحدة التحكم بدون حزمة إصدار مؤقتة مؤقتة. للمرجعية، سجل وحدة التحكم Arm64 للتنفيذ الأخير في `docs/source/confidential_assets_calibration_neon_20251018.log`.

وسائل تعليم رسالة التنفيذ (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| تعليمات | ميديانا `ns/op` | الجدول الزمني `gas` | `ns/gas` |
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

جدول زمني ومدفوع لـ `gas::tests::calibration_bench_gas_snapshot` (إجمالي 1,413 غاز بدون مجموعة من التعليمات الجديدة) ويمكن أن يحدث تصحيحات مستقبلية أو قياس دون تحديث تركيبات المعايرة.