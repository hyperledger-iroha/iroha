---
lang: ar
direction: rtl
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2026-01-03T18:07:57.759135+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# خطوط الأساس السرية لمعايرة الغاز

يتتبع دفتر الأستاذ هذا المخرجات التي تم التحقق من صحتها لمعايرة الغاز السرية
المعايير. يوثق كل صف مجموعة قياس جودة الإصدار التي تم التقاطها باستخدام
الإجراء الموضح في `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`.

| التاريخ (التوقيت العالمي) | الالتزام | الملف الشخصي | `ns/op` | `gas/op` | `ns/gas` | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | خط الأساس النيون | 2.93e5 | 1.57e2 | 1.87e3 | داروين 25.0.0 Arm64e (hostinfo)؛ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | خط الأساس-نيون-20260428 | 4.29ه6 | 1.57e2 | 2.73e4 | داروين 25.0.0 ذراع 64 (`rustc 1.91.0`). الأمر: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`؛ سجل في `docs/source/confidential_assets_calibration_neon_20260428.log`. تتم جدولة عمليات تشغيل التكافؤ x86_64 (SIMD-محايد + AVX2) في فتحة معمل زيوريخ بتاريخ 2026-03-19؛ سيتم وضع المصنوعات اليدوية تحت `artifacts/confidential_assets_calibration/2026-03-x86/` مع الأوامر المطابقة وسيتم دمجها في الجدول الأساسي بمجرد التقاطها. |
| 2026-04-28 | — | خط الأساس-سيمد-محايد | — | — | — | **تم التنازل عنه** على Apple Silicon — يفرض `ring` NEON على النظام الأساسي ABI، لذا يفشل `RUSTFLAGS="-C target-feature=-neon"` قبل أن يتمكن البرنامج الأساسي من التشغيل (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). تظل البيانات المحايدة مغلقة على مضيف CI `bench-x86-neon0`. |
| 2026-04-28 | — | خط الأساس-avx2 | — | — | — | **مؤجل** حتى يتوفر مشغل x86_64. لا يمكن لـ `arch -x86_64` إنتاج الثنائيات على هذا الجهاز ("نوع وحدة المعالجة المركزية غير صالح قابل للتنفيذ"؛ راجع `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). يظل مضيف CI `bench-x86-avx2a` هو مصدر السجل. |

يقوم `ns/op` بتجميع ساعة الحائط المتوسطة لكل تعليمات يتم قياسها بواسطة Criterion؛
`gas/op` هو الوسط الحسابي لتكاليف الجدول المقابلة من
`iroha_core::gas::meter_instruction`; `ns/gas` يقسم مجموع النانو ثانية على
الغاز المجمع عبر مجموعة العينات المكونة من تسعة تعليمات.

*ملاحظة.* لا يُصدر مضيف Arm64 الحالي ملخصات المعيار `raw.csv` من
الصندوق أعد التشغيل باستخدام `CRITERION_OUTPUT_TO=csv` أو إصلاح المنبع قبل وضع علامة على a
قم بتحريرها بحيث يتم إرفاق العناصر المطلوبة في قائمة التحقق من القبول.
إذا كان `target/criterion/` لا يزال مفقودًا بعد `--save-baseline`، فاجمع التشغيل
على مضيف Linux أو إجراء تسلسل لإخراج وحدة التحكم في حزمة الإصدار كملف
فجوة توقف مؤقتة. كمرجع، سجل وحدة التحكم Arm64 من آخر تشغيل
يعيش في `docs/source/confidential_assets_calibration_neon_20251018.log`.

المتوسطات لكل تعليمات من نفس التشغيل (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

### 28-04-2026 (Apple Silicon، مع تمكين NEON)

زمن الوصول المتوسط لتحديث 28-04-2026 (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| تعليمات | الوسيط `ns/op` | الجدول الزمني `gas` | `ns/gas` |
| --- | --- | --- | --- |
| تسجيل النطاق | 8.58e6 | 200 | 4.29e4 |
| تسجيل الحساب | 4.40ه6 | 200 | 2.20e4 |
| تسجيل AssetDef | 4.23ه6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79ه6 | 67 | 5.66e4 |
| دور حساب المنحة | 3.60ه6 | 96 | 3.75e4 |
| إبطال الحساب | 3.76ه6 | 96 | 3.92e4 |
| تنفيذTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| منتأسيت | 3.92e6 | 150 | 2.61e4 |
| نقل الأصول | 3.59ه6 | 180 | 1.99e4 |

يتم اشتقاق مجاميع `ns/op` و`ns/gas` في الجدول أعلاه من مجموع
هذه المتوسطات (إجمالي `3.85717e7`ns عبر مجموعة التعليمات التسعة و1,413
وحدات الغاز).

يتم فرض عمود الجدول بواسطة `gas::tests::calibration_bench_gas_snapshot`
(إجمالي 1413 غازًا عبر مجموعة التعليمات التسعة) وسوف يتعثر في حالة التصحيحات المستقبلية
تغيير القياس دون تحديث تركيبات المعايرة.

## دليل القياس عن بعد لشجرة الالتزام (M2.2)

لكل مهمة خريطة الطريق **M2.2**، يجب أن تلتقط كل عملية معايرة الجديد
مقاييس شجرة الالتزام وعدادات الإخلاء لإثبات بقاء حدود ميركل
ضمن الحدود التي تم تكوينها:

-`iroha_confidential_tree_commitments{asset_id}`
-`iroha_confidential_tree_depth{asset_id}`
-`iroha_confidential_root_history_entries{asset_id}`
-`iroha_confidential_frontier_checkpoints{asset_id}`
-`iroha_confidential_frontier_last_checkpoint_height{asset_id}`
-`iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
-`iroha_confidential_root_evictions_total{asset_id}`
-`iroha_confidential_frontier_evictions_total{asset_id}`
-`iroha_zk_verifier_cache_events_total{cache,event}`

سجل القيم مباشرة قبل وبعد عبء عمل المعايرة. أ
أمر واحد لكل أصل يكفي؛ مثال لـ `4cuvDVPuLBKJyN6dPbRQhmLh68sU`:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

قم بإرفاق الإخراج الأولي (أو لقطة Prometheus) بتذكرة المعايرة حتى يتم
يمكن لمراجع الإدارة التأكد من الحدود القصوى لسجل الجذر والفواصل الزمنية لنقاط التفتيش
تكريم. دليل القياس عن بعد في `docs/source/telemetry.md#confidential-tree-telemetry-m22`
يتوسع في تنبيه التوقعات ولوحات Grafana المرتبطة بها.

قم بتضمين عدادات ذاكرة التخزين المؤقت للتحقق في نفس المسح حتى يتمكن المراجعون من التأكيد
ظلت نسبة الخطأ أقل من عتبة التحذير البالغة 40٪:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

توثيق النسبة المشتقة (`miss / (hit + miss)`) داخل مذكرة المعايرة
لإظهار تمارين نمذجة التكلفة المحايدة لـ SIMD، تم إعادة استخدام ذاكرات التخزين المؤقت الدافئة بدلاً من
سحق تسجيل التحقق Halo2.

## التنازل عن الحياد وAVX2

منح مجلس SDK تنازلاً مؤقتًا عن متطلبات بوابة PhaseC
قياسات `baseline-simd-neutral` و`baseline-avx2`:

- **SIMD محايد:** في Apple Silicon، تعمل الواجهة الخلفية للتشفير `ring` على فرض NEON لـ
  صحة أبي. تعطيل الميزة (`RUSTFLAGS="-C target-feature=-neon"`)
  إحباط الإنشاء قبل إنتاج ثنائي البدلاء (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** لا يمكن لسلسلة الأدوات المحلية إنتاج ثنائيات x86_64 (`arch -x86_64 rustc -V`
  → "نوع وحدة المعالجة المركزية غير صالح قابل للتنفيذ"; انظر
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

حتى يصبح مضيفو CI `bench-x86-neon0` و`bench-x86-avx2a` متصلين بالإنترنت، يتم تشغيل NEON
أعلاه بالإضافة إلى أدلة القياس عن بعد تفي بمعايير قبول المرحلة C.
تم تسجيل التنازل في `status.md` وستتم إعادة النظر فيه بمجرد تثبيت أجهزة x86
متاح.