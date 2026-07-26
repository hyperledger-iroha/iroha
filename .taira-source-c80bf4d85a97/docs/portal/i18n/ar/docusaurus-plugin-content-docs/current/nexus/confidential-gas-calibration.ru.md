---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: غاز سري من عيار العيار
الوصف: حجم النسخة الجديدة، وهو رسم بياني سري للغاز.
سبيكة: /nexus/Confidential-gas-calibration
---

# الخطوط الأساسية للغاز العياري السري

هذا المسجل يراقب النتائج المؤكدة لعيارات الغاز السرية. تتضمن كل خطوة توثيقًا لإصدار جديد مصغر، تحت عنوان إجراء من [الأصول السرية وتحويلات ZK](./confidential-assets#calibration-baselines--acceptance-gates).

| البيانات (التوقيت العالمي) | الالتزام | الملف الشخصي | `ns/op` | `gas/op` | `ns/gas` | مساعدة |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | خط الأساس النيون | 2.93e5 | 1.57e2 | 1.87e3 | داروين 25.0.0 Arm64e (hostinfo)؛ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | في انتظار | خط الأساس-سيمد-محايد | - | - | - | تم التخطيط للإطار المحايد x86_64 لمضيف CI `bench-x86-neon0`; سم. تذكرة GAS-214. ستتم إضافة النتائج بعد إغلاقها بنقطة واحدة (موجه الاختيار المسبق للإصدار 2.1). |
| 2026-04-13 | في انتظار | خط الأساس-avx2 | - | - | - | العيار التالي AVX2 هو الالتزام/البناء، وهو مسار محايد؛ مطلوب المضيف `bench-x86-avx2a`. تم إنشاء GAS-214 مؤخرًا باستخدام الإضافات الدقيقة `baseline-neon`. |

`ns/op` agregiruet mediianu ساعة حائط للتعليمات، معيار واضح؛ `gas/op` - هذا عبارة عن سلسلة صوتية رائعة من `iroha_core::gas::meter_instruction`؛ `ns/gas` هو غاز ثاني أكسيد الكربون قصير المدى للعمل وفقًا للتعليمات.

*التطبيق.* لا يضخ الماء إلى المعيار `raw.csv`; قم بالتحويل مع `CRITERION_OUTPUT_TO=csv` أو قم بتحسين التطوير الأولي قبل الإصدار التجريبي الذي يحتوي على قطع أثرية تحتاج إلى التحقق منها نصائح تم طرحها. إذا تم الرد على `target/criterion/` مرة أخرى بعد `--save-baseline`، فيمكنك استخدام مطور Linux أو إجراء تسلسل لوحدات التحكم في نطاق متجدد مثل الفجوة المؤقتة. في الحقيقة، تم إدخال سجل التحكم Arm64 مؤخرًا في `docs/source/confidential_assets_calibration_neon_20251018.log`.

الوسائط المبنية على التعليمات من هذا القبيل (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| تعليمات | الوسيط `ns/op` | الجدول الزمني `gas` | `ns/gas` |
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

يتم تحديد جدول كولونكا `gas::tests::calibration_bench_gas_snapshot` (حوالي 1,413 غازًا في مجموعة من التعليمات) وإخراج أوشيبكو، إذا كنت ترغب في ذلك يؤدي التحسين إلى تقليل القياس بدون معايرة المعايرة.