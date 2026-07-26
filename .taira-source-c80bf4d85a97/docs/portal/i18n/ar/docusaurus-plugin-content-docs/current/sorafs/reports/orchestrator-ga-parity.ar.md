---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#تقرير تكافؤ GA لمنسق SoraFS

يتم الآن تعقب تكافؤ الجلب المتعدد لكل SDK، مما يؤدي إلى هندسة جديدة من المؤكد
بايتات الحمولة وإيصالات القطعة وتقارير المزود ونتائج لوحة النتائج تبقى متطابقة عبر
التطبيقات. كل الحزام يستهلك الحزمة القياسية المتعددة المتحكمين تحتها
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`، والتي تضم خطة SF1 وبيانات
مزود اللقطة الوصفية والقياس عن بعد وخيارات الأوركسترا.

## خط الأساس في الصدأ

- **الأمر:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** خطة يغغّل `MultiPeerFixture` مرتين عبر الأوركسترا داخل الموصل، مع التحقق
  من بايتات الحمولة المجمعة وإيصالات القطعة والتقارير مزود ونتائج لوحة النتائج. كما
  تتعقب أدوات القياس للقياس التوازي وحجم مجموعة العمل الفعالة (`max_parallel x max_chunk_length`).
- **حاجز الأداء:** يجب أن يكتمل التشغيل بالكامل خلال 2 ثانية على جير CI.
- **سقف مجموعة العمل:** مع ملف تعريف SF1 يفترض تسخير `max_parallel = 3`، مما ينتج
  نافذة <= 196608 بايت.

مثال لمخرجات السجل:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harness لزمة JavaScript SDK

- **الأمر:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **نطاق التشغيل:** يعيد نفس التركيبة عبر `iroha_js_host::sorafsMultiFetchLocal`، ويقارن
  مزود الحمولات والإيصالات والتقارير ولقطات لوحة النتائج عبر تشغيل متتاليين.
- **حاجات الأداء:** يجب أن يتم تنفيذ كل شيء خلال ثانيتين؛ يطبع الحزام المدة وسقف
  البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

مثال على ملخص سطر:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harness لحزمة Swift SDK- **الأمر:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق:** يشغّل مجموعة التكافؤ المعرّفة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`،
  مع إعادة تشغيل تركيبات SF1 معًا عبر الجسر Norito (`sorafsLocalFetch`). متى تسخير
  من بايتات الحمولة وإيصالات القطعة ومزود التقارير ومدخلات لوحة النتائج باستخدام نفس
  مزود البيانات الوصفية الحتمية ولقطات القياس عن بعد الخاصة بحزم Rust/JS.
- ** تهيئة الجسر:** فيك تسخير الضغط `dist/NoritoBridge.xcframework.zip` عند الطلب ويحمل
  شريحة macOS عبر `dlopen`. عند غياب xcframework أو افتقاره لروابط SoraFS، يتراجع إلى
  `cargo build -p connect_norito_bridge --release` ويربط مع
  `target/release/libconnect_norito_bridge.dylib`، لذلك لا يتطلب الإعداد اليدوي في CI.
- **حاجات الأداء:** يجب أن يتم تنفيذ كل شيء خلال ثانيتين على Gear CI؛ ويطبع تسخير المدة
  مثالية وسقف البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

مثال على ملخص سطر:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير لربط بايثون- **الأمر:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **النطاق:** التفاف المجمع عالي المستوى `iroha_python.sorafs.multi_fetch_local` وdataclasses
  المقيّدة بالأنواع كي تمر بالتركيبات القياسية عبر الواجهة التي يستخدمها مستهلكو
  عجلة. إعادة اختبار بناء بيانات المزود الوصفية من `providers.json`، ويحقن
  لقطة القياس عن بعد، ويتحقق من بايتات الحمولة وإيصالات القطعة وموفر التقارير والمحتوى
  لوحة النتائج مثل حزم Rust/JS/Swift.
- **متطلب المسبق:** شغّل `maturin develop --release` (أو ثبّت العجلة) يعرض كي `_crypto`
  `sorafs_multi_fetch_local` قبل تشغيل pytest؛ يتخطى تسخير تلقائيا عندما لا يكون
  حرية الاختيار
- **حاجز الأداء:** نفس حد <= 2 ثانية مثل حزمة Rust؛ سجل pytest عدد البايتات المجمعة
  وملخص مشاركة مقدمي قطعة الإصدار.

يجب أن تختار مسارًا يعتمد على نسخة المخرجات الملخص من كل تسخير (Rust وPython وJS وSwift)
حتى يتم تقديم التقرير المؤرشف من مقارنة إيصالات الحمولة والمقاييس بشكل موحد قبل الترقية
بناء. شغّل `ci/sdk_sorafs_orchestrator.sh` يجتمع كل مجموعات التكافؤ (Rust وPython)
الارتباطات وJS وSwift) في TBTB واحدة؛ ينبغي أن تُنتج مخرجات CI مقتطف السجل من هذا
المساعدة بالإضافة إلى ملف `matrix.md` المُنشأ (جدول SDK/الحالة/المدة) بتذكرة الإصدار
حتى أخذ المراجعون من تدقيق مصفوفة التكافؤ دون إعادة تشغيل المجموعة المحلية.