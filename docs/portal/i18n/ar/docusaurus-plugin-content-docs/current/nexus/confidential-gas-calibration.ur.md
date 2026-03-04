---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/confidential-gas-calibration.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: أخفى جيس كيليبريشن ليجر
الوصف: قم بتخفيف الوزن الزائد الذي تم نشره بعد ذلك.
سبيكة: /nexus/Confidential-gas-calibration
---

# خفيف جيس كيليبريشن بيس لاينز

لقد تم تسليط الضوء على علامة كيليبريشن التي تم رصد نتائجها النهائية. تم الحصول على هذه القطارات من خلال إنشاء قاعدة قروض [الأصول السرية & تحويلات ZK](./confidential-assets#calibration-baselines--acceptance-gates).

| تاريخ (UTC) | الالتزام | پروفيل | `ns/op` | `gas/op` | `ns/gas` | أخبار |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | خط الأساس النيون | 2.93e5 | 1.57e2 | 1.87e3 | داروين 25.0.0 Arm64e (hostinfo)؛ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | في انتظار | خط الأساس-سيمد-محايد | - | - | - | مضيف CI `bench-x86-neon0` لـ x86_64 نيوترل جديد؛ ٹکٹ GAS-214 ديك. تم دمج نتائج مقاعد البدلاء في وقت واحد (الدمج المسبق هو جزء من 2.1 من عدد البنات). |
| 2026-04-13 | في انتظار | خط الأساس-avx2 | - | - | - | يتم تنفيذ/بناء الجديد بواسطة AVX2 كيليبريشن؛ المضيف `bench-x86-avx2a` درکار ہے۔ GAS-214 يقدم معلومات عن `baseline-neon` للمقارنة مع كل ما هو جديد. |

`ns/op` المعيار الذي تم تسجيله في الخريطة المجمعة والنقرة; `gas/op` `iroha_core::gas::meter_instruction` علاقة شيڈول بحساباتها في الوسط؛ `ns/gas` هو موقع جديد يضم مجموعة انترنت نينو التي تم تصنيفها على نطاق واسع.

*ملاحظة.* مضيف Arm64 موجود وفقًا للمعيار `raw.csv` الخلاصة لا تنبعث منها؛ لقد تم إعادة إنشاء الملف `CRITERION_OUTPUT_TO=csv` من خلال إعادة التدوير أو المنبع لقائمة التحقق من قبول المصنوعات اليدوية المطلوبة. إذا كان `target/criterion/` `--save-baseline` الذي يأتي بعد ذلك مضيف Linux لمجموعات الكريات أو المراكز، فإن حزمة الريليز ستجري تسلسلًا في عرض مؤقت. أصبح الآن أحدث إصدار من Arm64 كنسول لاغ `docs/source/confidential_assets_calibration_neon_20251018.log` متاحًا.

اس اي رن في انستركشن ماينز (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

شيلد كالم `gas::tests::calibration_bench_gas_snapshot` ذو رأس نافذ أوتا (غير مسجل في كل 1,413 قدم) وما إلى ذلك من أحدث إصدارات كيليبرن لم تعد الفاكسات متاحة لك الآن.