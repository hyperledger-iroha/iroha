---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Libro Mayor de calibracion de Gas Confidential
الوصف: أدوية ذات جودة عالية تستجيب لتقويم الغاز السري.
سبيكة: /nexus/Confidential-gas-calibration
---

# خطوط قاعدة معايرة الغاز السرية

يقوم هذا بتسجيل النتائج المعتمدة لمعايير معايرة الغاز السرية. كل ملف يوثق مجموعة من الأدوية عالية الجودة التي تم التقاطها باستخدام الإجراء الموصوف في [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| فيتشا (UTC) | الالتزام | الملف الشخصي | `ns/op` | `gas/op` | `ns/gas` | نوتاس |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | خط الأساس النيون | 2.93e5 | 1.57e2 | 1.87e3 | داروين 25.0.0 Arm64e (hostinfo)؛ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | في انتظار | خط الأساس-سيمد-محايد | - | - | - | إخراج محايد x86_64 مبرمج على المضيف CI `bench-x86-neon0`; نسخة التذكرة GAS-214. يتم تجميع النتائج عندما تنتهي نافذة المقعد (قائمة التحقق المسبقة للدمج تصل إلى الإصدار 2.1). |
| 2026-04-13 | في انتظار | خط الأساس-avx2 | - | - | - | تستخدم معايرة AVX2 نفس الالتزام/البناء بحيث لا يكون المسار محايدًا؛ يتطلب المضيف `bench-x86-avx2a`. GAS-214 نطاقات مكعبة مقارنة بالدلتا مع `baseline-neon`. |

`ns/op` يجمع متوسط ​​وقت الجدار من خلال التعليمات المتوسطة من خلال المعيار؛ `gas/op` هو وسيلة الإعلام لتكاليف الجدولة المقابلة لـ `iroha_core::gas::meter_instruction`؛ `ns/gas` قم بتقسيم الأجزاء النانوية المجمعة بين الغاز المجمع إلى مجموعة من التعليمات الجديدة.

*ملحوظة.* El host Arm64 فعليًا لا يصدر استئنافات `raw.csv` من معيار الخلل؛ قم بتنفيذ `CRITERION_OUTPUT_TO=csv` أو إجراء تصحيح أولي قبل إصدار إصدار حتى يتم طلب العناصر الإضافية من خلال قائمة التحقق من القبول. إذا كان `target/criterion/` يفشل بعد `--save-baseline`، فاسترجع المسار في مضيف Linux أو قم بتسلسل خروج وحدة التحكم في حزمة الإصدار كإيقاف مؤقت. كمرجع، سجل وحدة التحكم Arm64 لآخر مسار حي في `docs/source/confidential_assets_calibration_neon_20251018.log`.

الوسائط لتعليم نفس المسار (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

عمود الجدول الزمني هذا يفرض `gas::tests::calibration_bench_gas_snapshot` (إجمالي 1,413 غازًا ومجموعة التعليمات الجديدة) ويؤدي إلى حدوث تغييرات مستقبلية في القياس دون تحديث تركيبات المعايرة.