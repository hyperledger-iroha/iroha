---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 13b023d0ac69fdb575fde5638f0bce4aadd02f591c725c82629ec8b1a4114a16
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# تقرير تكافؤ GA لمنسق SoraFS

يتم الآن تتبع تكافؤ multi-fetch الحتمي لكل SDK كي يتمكن مهندسو الإطلاق من تأكيد أن
بايتات payload وإيصالات chunk وتقارير provider ونتائج scoreboard تبقى متطابقة عبر
التطبيقات. كل harness يستهلك الحزمة القياسية متعددة المزوّدين تحت
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`، والتي تجمع خطة SF1 وبيانات
provider الوصفية ولقطة telemetry وخيارات orchestrator.

## خط الأساس في Rust

- **الأمر:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** يشغّل خطة `MultiPeerFixture` مرتين عبر orchestrator داخل العملية، مع التحقق
  من بايتات payload المجمعة وإيصالات chunk وتقارير provider ونتائج scoreboard. كما
  تتعقب أدوات القياس ذروة التوازي وحجم working-set الفعال (`max_parallel x max_chunk_length`).
- **حاجز الأداء:** يجب أن يكتمل كل تشغيل خلال 2 s على عتاد CI.
- **سقف مجموعة العمل:** مع ملف تعريف SF1 يفرض harness `max_parallel = 3`، مما ينتج
  نافذة <= 196608 بايت.

مثال لمخرجات السجل:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harness لحزمة JavaScript SDK

- **الأمر:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **النطاق:** يعيد تشغيل نفس fixture عبر `iroha_js_host::sorafsMultiFetchLocal`، ويقارن
  payloads وreceipts وتقارير provider ولقطات scoreboard عبر تشغيلين متتاليين.
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s؛ يطبع harness المدة المقاسة وسقف
  البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

سطر ملخص مثال:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harness لحزمة Swift SDK

- **الأمر:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق:** يشغّل مجموعة التكافؤ المعرّفة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`،
  مع إعادة تشغيل fixture SF1 مرتين عبر جسر Norito (`sorafsLocalFetch`). يتحقق harness
  من بايتات payload وإيصالات chunk وتقارير provider ومدخلات scoreboard باستخدام نفس
  بيانات provider الوصفية الحتمية ولقطات telemetry الخاصة بحزم Rust/JS.
- **تهيئة الجسر:** يفك harness ضغط `dist/NoritoBridge.xcframework.zip` عند الطلب ويحمل
  شريحة macOS عبر `dlopen`. عند غياب xcframework أو افتقاره لروابط SoraFS، يتراجع إلى
  `cargo build -p connect_norito_bridge --release` ويربط مع
  `target/release/libconnect_norito_bridge.dylib`، لذا لا يلزم إعداد يدوي في CI.
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s على عتاد CI؛ ويطبع harness المدة
  المقاسة وسقف البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

سطر ملخص مثال:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harness لربط Python

- **الأمر:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **النطاق:** يمارس wrapper عالي المستوى `iroha_python.sorafs.multi_fetch_local` وdataclasses
  المقيّدة بالأنواع كي تمر fixture القياسية عبر نفس الواجهة التي يستخدمها مستهلكو
  wheel. يعيد الاختبار بناء بيانات provider الوصفية من `providers.json`، ويحقن
  لقطة telemetry، ويتحقق من بايتات payload وإيصالات chunk وتقارير provider ومحتوى
  scoreboard مثل حزم Rust/JS/Swift.
- **متطلب مسبق:** شغّل `maturin develop --release` (أو ثبّت wheel) كي يعرض `_crypto` ربط
  `sorafs_multi_fetch_local` قبل تشغيل pytest؛ يتخطى harness تلقائيا عندما لا يكون
  الربط متاحا.
- **حاجز الأداء:** نفس حد <= 2 s مثل حزمة Rust؛ يسجل pytest عدد البايتات المجمعة
  وملخص مشاركة providers لقطعة الإصدار.

يجب أن يلتقط مسار اعتماد الإصدار مخرجات الملخص من كل harness (Rust وPython وJS وSwift)
حتى يتمكن التقرير المؤرشف من مقارنة إيصالات payload والقياسات بشكل موحد قبل ترقية
build. شغّل `ci/sdk_sorafs_orchestrator.sh` لتنفيذ كل مجموعات التكافؤ (Rust وPython
bindings وJS وSwift) في تمريرة واحدة؛ ينبغي أن تُرفق مخرجات CI مقتطف السجل من هذا
المساعد بالإضافة إلى ملف `matrix.md` المُنشأ (جدول SDK/الحالة/المدة) بتذكرة الإصدار
حتى يتمكن المراجعون من تدقيق مصفوفة التكافؤ دون إعادة تشغيل المجموعة محليا.
