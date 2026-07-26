---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير paridad GA del Orchestrator SoraFS

تم الآن نشر مقياس الحتمية المتعددة الجلب بواسطة SDK لهم
يمكن لمهندسي الإصدار تأكيد وحدات بايت الحمولة وإيصالات القطع،
تقارير المزودين ونتائج لوحة النتائج قابلة للتعديل بشكل دائم
التنفيذ. كل تسخير يستهلك الحزمة متعددة الموفر canonico bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`، الذي يتضمن الخطة SF1،
البيانات الوصفية للموفر ولقطات القياس عن بعد وخيارات المنسق.

## الصدأ الأساس

- **الأمر:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** تنفيذ الخطة `MultiPeerFixture` مرتين عبر المنسق قيد التشغيل،
  التحقق من بايتات الحمولة المجمعة وإيصالات القطع وتقارير الموفر
  نتائج لوحة النتائج. يتم أيضًا استخدام الأجهزة بشكل متزامن
  والحجم الفعال لمجموعة العمل (`max_parallel x max_chunk_length`).
- **حارس الأداء:** يجب إكمال كل عملية تنفيذ في ثانيتين مع CI في الأجهزة.
- **سقف مجموعة العمل:** مع ملف SF1 el Harbour المطبق `max_parallel = 3`،
  نافذة مفتوحة <= 196608 بايت.

نموذج لإخراج السجل:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## أداة JavaScript SDK- **الأمر:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **النطاق:** إعادة إنتاج تركيبات الميسمو عبر `iroha_js_host::sorafsMultiFetchLocal`،
  مقارنة الحمولات والإيصالات وتقارير المزود ولقطات لوحة النتائج بينهما
  القذف المتتالي.
- **Performance Guard:** يجب الانتهاء من كل عملية تنفيذ في ثانيتين؛ إل تسخير imprime لا
  المدة المتوسطة وتقنية وحدات البايت المحفوظة (`max_parallel = 3`، `peak_reserved_bytes <= 196608`).

مثال على سطر التلخيص:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير سويفت SDK

- **الأمر:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق:** تنفيذ مجموعة المعايير المحددة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`،
  يتم إعادة إنتاج تركيبات SF1 مرتين عبر الجسر Norito (`sorafsLocalFetch`). الحزام
  التحقق من بايتات الحمولة وإيصالات القطع وتقارير الموفر وإدخالات لوحة النتائج باستخدام
  يوفر نفس الشيء البيانات الوصفية الحتمية ولقطات القياس عن بعد التي تتضمنها أجنحة Rust/JS.
- **جسر التمهيد:** El Harbour descomprime `dist/NoritoBridge.xcframework.zip` bajo requesta y carga
  شريحة macOS عبر `dlopen`. إذا كان xcframework falta أو لا توجد روابط SoraFS، فيجب الرجوع إلى
  `cargo build -p connect_norito_bridge --release` ورابط مضاد
  `target/release/libconnect_norito_bridge.dylib`، بدون دليل الإعداد وCI.
- **حارس الأداء:** يجب إنهاء كل عملية تنفيذ خلال ثانيتين في CI الخاص بالأجهزة؛ إل تسخير imprime لا
  المدة المتوسطة وتقنية وحدات البايت المحفوظة (`max_parallel = 3`، `peak_reserved_bytes <= 196608`).

مثال على سطر التلخيص:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير ربط بايثون- **الأمر:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **النطاق:** تنفيذ المجمع عالي المستوى `iroha_python.sorafs.multi_fetch_local` وفئات البيانات الخاصة به
  نصائح لكي تتدفق وحدة التحكم Canon بواسطة واجهة برمجة التطبيقات نفسها التي تستهلك العجلات. اختبار ال
  إعادة إنشاء البيانات الوصفية للموفر من `providers.json`، وإدخال لقطة القياس عن بعد والتحقق منها
  بايتات الحمولة وإيصالات القطع وتقارير الموفر ومحتوى لوحة النتائج المشابهة لها
  أجنحة Rust/JS/Swift.
- **الطلب المسبق:** قم بتشغيل `maturin develop --release` (تثبيت العجلة) لعرض `_crypto`
  ملزمة `sorafs_multi_fetch_local` قبل الاستدعاء pytest؛ يتم تشغيل الحزام تلقائيًا عندما يتم تشغيله
  ملزمة لا يمكن الوصول إليها.
- **حارس الأداء:** نفس الشيء المفترض <= 2 ثانية من الصدأ؛ pytest تسجيل الحساب
  وحدات البايت المجمعة واستئناف مشاركة مقدمي الخدمة لإصدار المنتج.

يجب أن يلتقط الإصدار الملخص الناتج من كل تسخير (Rust، Python، JS، Swift) لذلك
يمكن أرشفة التقرير مقارنة إيصالات الحمولة ومقاييس الشكل الموحد قبل
المروج الأمم المتحدة بناء. قم بتشغيل `ci/sdk_sorafs_orchestrator.sh` لتصحيح كل مجموعة من الباريداد
(Rust وPython Bindings وJS وSwift) بخطوة واحدة فقط؛ يجب إضافة القطع الأثرية لـ CI
يساعد استخراج السجل هذا على إنشاء `matrix.md` (جدول SDK/الحالة/المدة) على التذكرة
أطلق سراحهم حتى يتمكن المراجعون من مراجعة مصفوفة الجدار دون إعادة تشغيل المجموعة محليًا.