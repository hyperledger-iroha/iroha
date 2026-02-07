---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: سجل معايرة الغاز السري
الوصف: قياسات جودة الإطلاق التي تحدد رزنامة الغاز السرية.
سبيكة: /nexus/Confidential-gas-calibration
---

# خطوط قاعدة معايرة الغاز الواثقة

هذا التسجيل يناسب الطلعات الصالحة لمعايير معايرة الغاز السرية. كل خط يوثق لعبة تدابير تحرير عالية الجودة مع الإجراء المحدد في [الأصول السرية وتحويلات ZK](./confidential-assets#calibration-baselines--acceptance-gates).

| التاريخ (التوقيت العالمي) | الالتزام | الملف الشخصي | `ns/op` | `gas/op` | `ns/gas` | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | خط الأساس النيون | 2.93e5 | 1.57e2 | 1.87e3 | داروين 25.0.0 Arm64e (hostinfo)؛ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | في انتظار | خط الأساس-سيمد-محايد | - | - | - | تنفيذ محايد x86_64 Planifiee sur l'hote CI `bench-x86-neon0`; تذكرة شراء GAS-214. سيتم إضافة النتائج مرة أخرى إلى نافذة المقعد المنتهية (قائمة التحقق المسبقة للدمج في الإصدار 2.1). |
| 2026-04-13 | في انتظار | خط الأساس-avx2 | - | - | - | تستخدم معايرة AVX2 للمتابعة تنفيذ/إنشاء تنفيذ محايد؛ يتطلب `bench-x86-avx2a`. GAS-214 يغطي الجزء المزدوج مع مقارنة دلتا مع `baseline-neon`. |

`ns/op` يوافق على ساعة الحائط المتوسطة حسب التعليمات حسب المعيار؛ `gas/op` هو أفضل وسيلة حسابية للجداول الزمنية المقابلة لـ `iroha_core::gas::meter_instruction`; `ns/gas` يقسم النانو ثانية على أساس الغاز على مجموعة التعليمات الجديدة.

*ملاحظة.* لا يتم إنتاج السيرة الذاتية لـ hote Arm64 فعليًا `raw.csv` de Criterion par defaut؛ قم بالارتباط مع `CRITERION_OUTPUT_TO=csv` أو تصحيح أولي قبل تحرير إصدار حتى تكون العناصر التي تتطلبها قائمة التحقق من القبول مرفقة أيضًا. إذا قمت بإعادة `target/criterion/` بعد `--save-baseline`، فالتقط التنفيذ على نظام Linux ساخن أو قم بتسلسل وحدة التحكم في حزمة الإصدار كإيقاف مؤقت. عنوان مرجعي، تم العثور على ذراع وحدة التحكم في السجل 64 للتنفيذ الأخير في `docs/source/confidential_assets_calibration_neon_20251018.log`.

الوسائط حسب تعليمات تنفيذ meme (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| تعليمات | متوسط ​​`ns/op` | الجدول الزمني `gas` | `ns/gas` |
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

تم فرض الجدول الزمني العمودي وفقًا لـ `gas::tests::calibration_bench_gas_snapshot` (إجمالي 1,413 غازًا في مجموعة التعليمات الجديدة) وتكرار ما إذا كانت التصحيحات المستقبلية تعدل القياس بدون ضبط تركيبات المعايرة يوميًا.