---
lang: ar
direction: rtl
source: docs/references/configuration.md
status: complete
translator: manual
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-11-02T04:40:39.795595+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/references/configuration.md (Acceleration) -->

# التسريع (Acceleration)

تتحكم مقطع الإعدادات `[accel]` في تفعيل التسريع الاختياري بواسطة العتاد
لـ IVM والوحدات المساعدة (helpers). لدى جميع المسارات المعجّلة fallbacks
حتمية على المعالج (CPU)؛ فإذا فشل backend في اختبار golden ذاتي أثناء
التشغيل، يتم تعطيله تلقائيًا وتستمر عملية التنفيذ على الـ CPU.

- `enable_cuda` (القيمة الافتراضية: `true`) – استخدام CUDA عند تجميعها
  وتوافرها.
- `enable_metal` (القيمة الافتراضية: `true`) – استخدام Metal على macOS
  عند توافره.
- `max_gpus` (القيمة الافتراضية: `0`) – الحد الأقصى لعدد وحدات GPU التي
  تتم تهيئتها؛ القيمة `0` تعني auto/بدون حد صريح.
- `merkle_min_leaves_gpu` (القيمة الافتراضية: `8192`) – أقل عدد من أوراق
  Merkle الذي يتم عنده ترحيل (offload) عملية تجزئة الأوراق إلى الـ GPU.
  لا يُنصح بتقليل القيمة إلا مع وحدات GPU سريعة على نحو غير معتاد.
- خيارات متقدمة (اختيارية؛ غالبًا ما ترث قيمًا افتراضية مناسبة):
  - `merkle_min_leaves_metal` (الافتراضي: يرث قيمة
    `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (الافتراضي: يرث قيمة
    `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (الافتراضي: `32768`) – تفضيل
    تنفيذ SHA‑2 على الـ CPU حتى هذا العدد من الأوراق على معمارية ARMv8 مع
    دعم SHA2.
  - `prefer_cpu_sha2_max_leaves_x86` (الافتراضي: `32768`) – تفضيل
    تنفيذ SHA‑NI على الـ CPU حتى هذا العدد من الأوراق على x86/x86_64.

ملاحظات
- الأولوية للحتمية: التسريع لا يغيّر النتائج المرصودة؛ إذ تُجري الـ
  backends اختبارات golden عند التهيئة، وتعود إلى مسارات scalar/SIMD عندما
  تُرصَد اختلافات.
- يوصى بضبط الإعدادات عبر `iroha_config` فقط؛ تجنّب الاعتماد على
  متغيرات البيئة في مسارات الإنتاج.

</div>

