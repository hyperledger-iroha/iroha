---
lang: ar
direction: rtl
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2026-01-03T18:07:57.621942+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# الثوابت المعدنية المشتركة بوسيدون

يجب مشاركة النوى المعدنية، ونواة CUDA، ومثبت الصدأ، وكل تركيبات SDK
نفس معلمات Poseidon2 بالضبط من أجل الحفاظ على تسريع الأجهزة
التجزئة الحتمية. يسجل هذا المستند اللقطة الأساسية، وكيفية القيام بذلك
إعادة إنشائها، وكيف من المتوقع أن تستوعب خطوط أنابيب GPU البيانات.

## بيان اللقطة

يتم نشر المعلمات كمستند `PoseidonSnapshot` RON. النسخ هي
يتم الاحتفاظ بها تحت التحكم في الإصدار حتى لا تعتمد سلاسل أدوات GPU وحزم SDK على وقت البناء
توليد الكود.

| المسار | الغرض | شا-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` | لقطة قانونية تم إنشاؤها من `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}`؛ مصدر الحقيقة لبناء GPU. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | يعكس اللقطة الأساسية بحيث تقوم اختبارات وحدة Swift وحزام الدخان XCFramework بتحميل نفس الثوابت التي تتوقعها النوى المعدنية. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | تشترك تركيبات Android/Kotlin في نفس البيان لاختبارات التكافؤ والتسلسل. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

يجب على كل مستهلك التحقق من التجزئة قبل توصيل الثوابت بوحدة معالجة الرسومات
خط أنابيب. عندما يتغير البيان (مجموعة المعلمات الجديدة أو الملف الشخصي)، فإن SHA و
يجب تحديث المرايا السفلية بخطوة القفل.

## التجديد

يتم إنشاء البيان من مصادر Rust عن طريق تشغيل `xtask`
مساعد. يكتب الأمر كلاً من الملف المتعارف عليه ومرايا SDK:

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

استخدم `--constants <path>`/`--vectors <path>` لتجاوز الوجهات أو
`--no-sdk-mirror` عند إعادة إنشاء اللقطة الأساسية فقط. سوف المساعد
عكس المصنوعات اليدوية في شجرتي Swift وAndroid عند حذف العلامة،
الذي يحافظ على محاذاة التجزئة لـ CI.

## تغذية المعادن/بنيات CUDA

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` و
  يجب إعادة إنشاء `crates/fastpq_prover/cuda/fastpq_cuda.cu` من ملف
  يظهر كلما تغير الجدول.
- يتم تنظيم الثوابت المقربة وثوابت MDS إلى `MTLBuffer`/`__constant` المتجاورة
  المقاطع التي تطابق تخطيط البيان: `round_constants[round][state_width]`
  تليها مصفوفة MDS 3x3.
- يقوم `fastpq_prover::poseidon_manifest()` بتحميل اللقطة والتحقق من صحتها
  وقت التشغيل (أثناء عملية إحماء المعدن) بحيث يمكن للأدوات التشخيصية التأكد من أن
  تتطابق ثوابت التظليل مع التجزئة المنشورة عبر
  `fastpq_prover::poseidon_manifest_sha256()`.
- قارئات أدوات SDK (Swift `PoseidonSnapshot`، Android `PoseidonSnapshot`) و
  تعتمد أدوات Norito غير المتصلة بالإنترنت على نفس البيان، مما يمنع استخدام وحدة معالجة الرسومات فقط
  شوكة المعلمة

## التحقق من الصحة

1. بعد إعادة إنشاء البيان، قم بتشغيل `cargo test -p xtask` لممارسة
   اختبارات وحدة توليد تركيبات بوسيدون.
2. قم بتسجيل SHA-256 الجديد في هذا المستند وفي أي لوحات معلومات يتم مراقبتها
   التحف GPU.
3. يوزع `cargo test -p fastpq_prover poseidon_manifest_consistency`
   `poseidon2.metal` و`fastpq_cuda.cu` في وقت الإنشاء ويؤكد أنهما
   تتطابق الثوابت المتسلسلة مع البيان، مع الاحتفاظ بجداول CUDA/Metal و
   اللقطة الأساسية في خطوة القفل.إن الاحتفاظ بالبيان جنبًا إلى جنب مع تعليمات إنشاء GPU يمنح Metal/CUDA
سير العمل عبارة عن مصافحة حتمية: تتمتع النواة بالحرية في تحسين ذاكرتها
التخطيط طالما أنهم يستوعبون الثوابت المشتركة ويكشفون عن التجزئة
القياس عن بعد للتحقق من التكافؤ.