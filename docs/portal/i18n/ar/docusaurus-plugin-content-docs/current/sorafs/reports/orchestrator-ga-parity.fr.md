---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de parité GA SoraFS Orchestrator

يتم تعطيل التكافؤ المحدد متعدد الجلب بواسطة SDK بسبب ذلك
يؤكد مهندسو الإصدار بشكل قوي على بايتات الحمولة وإيصالات القطع،
تظل تقارير المزود ونتائج لوحة النتائج محاذية بينهما
التطبيقات. Chaque Harness يستهلك حزمة متعددة الموفر Canonique dans
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`، الذي أعاد تجميع الخطة SF1،
مزود البيانات الوصفية ولقطة القياس عن بعد وخيارات المنسق.

## الصدأ الأساس

- **الأمر:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** تنفيذ الخطة `MultiPeerFixture` مرتين عبر المُنسق قيد التشغيل،
  التحقق من وحدات بايت الحمولة المجمعة وإيصالات القطع وتقارير الموفر وما إلى ذلك
  نتائج لوحة النتائج. تتناسب الأجهزة أيضًا مع تزامن النقطة
  والتفاصيل الفعالة لمجموعة العمل (`max_parallel × max_chunk_length`).
- **Performance Guard:** Chaque execution doit end en 2 s sur le hardware CI.
- **سقف مجموعة العمل:** Avec le profil SF1, le Harvest `max_parallel = 3`,
  قم بإنشاء نافذة ≥ 196608 بايت.

نموذج لإخراج السجل:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## أداة JavaScript SDK- **الأمر:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **النطاق:** استمتع بنفس التركيبة عبر `iroha_js_host::sorafsMultiFetchLocal`،
  الحمولات المقارنة والإيصالات وتقارير المزودين ولقطات لوحة النتائج كلها
  عمليات الإعدام المتتابعة.
- **حارس الأداء:** Chaque exécution doit finir en 2 s ; لو تسخير imprime لا
  مدة القياس ووحدة البايت المحفوظة (`max_parallel = 3`، `peak_reserved_bytes ≤ 196 608`).

مثال على سطر التلخيص:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير سويفت SDK

- **الأمر:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق:** تنفيذ مجموعة التكافؤ المحددة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`،
  تجدد تركيبات SF1 مرتين عبر الجسر Norito (`sorafsLocalFetch`). تحقق من الحزام
  بايتات الحمولة وإيصالات القطع وتقارير الموفر وإدخالات لوحة النتائج باستخدام البيانات
  يقوم مزود البيانات الوصفية أيضًا بتحديد ولقطات القياس عن بعد من مجموعات Rust/JS.
- **Bridge bootstrap:** أداة فك الضغط `dist/NoritoBridge.xcframework.zip` حسب الطلب والشحن
  شريحة macOS عبر `dlopen`. عندما يصبح xcframework معطلاً أو غير قادر على الارتباطات SoraFS، فإنه
  أساسي على `cargo build -p connect_norito_bridge --release` وهو موجود
  `target/release/libconnect_norito_bridge.dylib`، بدون الإعداد اليدوي وCI.
- **Performance Guard:** Chaque exécution doit finir en 2 s sur le hardware CI ; لو تسخير imprime لا
  مدة القياس ووحدة البايت المحفوظة (`max_parallel = 3`، `peak_reserved_bytes ≤ 196 608`).

مثال على سطر التلخيص:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير ربط بايثون- **الأمر:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **النطاق:** ممارسة المجمع ذو المستوى العالي `iroha_python.sorafs.multi_fetch_local` وفئات البيانات الخاصة به
  حتى تتمكن الأداة الأساسية من استخدام نفس واجهة برمجة التطبيقات (API) التي يستخدمها مستهلكو العجلة. إعادة بناء الاختبار
  موفر البيانات الوصفية من `providers.json`، أدخل لقطة القياس عن بعد وتحقق من بايتات الحمولة،
  إيصالات مقطوعة وتقارير المزودين ومحتوى لوحة النتائج مثل مجموعات Rust/JS/Swift.
- **الطلب المسبق:** تنفيذ `maturin develop --release` (أو تثبيت العجلة) حتى يكشف `_crypto` عن الرابط
  `sorafs_multi_fetch_local` قبل استدعاء pytest ; يتم تخطي الحزام تلقائيًا عندما يصبح الربط غير قابل للاستبدال.
- **Performance Guard:** نفس الميزانية ≥ 2 ثانية من Rust ; pytest سجل عدد البايتات المجمعة
  والسيرة الذاتية لمشاركة مقدمي الخدمة من أجل الإصدار.

يجب أن تقوم عملية تحرير البوابة بالتقاط ملخص إخراج كل تسخير (Rust، Python، JS، Swift) لتتمكن من ذلك
يمكن أرشفة التقرير مقارنة إيصالات الحمولة ومقاييس الطريقة الموحدة مسبقًا
الترويج للبناء. قم بتنفيذ `ci/sdk_sorafs_orchestrator.sh` لرمح كل جناح من التكافؤ
(Rust، Python Bindings، JS، Swift) بكلمة مرور واحدة ؛ يجب أن تنضم القطع الأثرية CI إلى الخارج
سجل هذا المساعد بالإضافة إلى `matrix.md` الذي تم إنشاؤه (جدول SDK/الحالة/المدة) في تذكرة الإصدار حتى تتمكن من ذلك
يمكن للمراجعين مراجعة مصفوفة التكافؤ دون إعادة النظر في المجموعة المحلية.