---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Orchestrator GA للتقرير

حتمية الجلب المتعدد للاختبار باستخدام حساب SDK لبوابات الإنترنت وتتيح للمهندسين وبحثًا عن الألعاب
بايتات الحمولة، وإيصالات القطع، وتقارير الموفر، ولوحة النتائج هي نتائج مختلفة لتطبيقات الأجهزة.
تسخير `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` تحت استخدام حزمة موفر متعددة الكنسي كرتا،
خطة SF1، وبيانات تعريف الموفر، ولقطات القياس عن بعد، وخيارات المنسق.

## الصدأ عبر الإنترنت

- **مانڈ:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **التسجيل:** خطة `MultiPeerFixture` هي المنسق قيد التشغيل الذي يبلغ طوله ذرتين، بايتات الحمولة المجمعة،
  إيصالات القطع وتقارير المزود ولوحة النتائج هي النتائج الحقيقية. انسٹرومنتٹیشن ذروة التزامن و
  أكثر مجموعة عمل ممتازة (`max_parallel x max_chunk_length`) هي الأفضل.
- **Performance Guard:** يتم تشغيل CI لمدة ثانيتين بواسطة اندر مكمل.
- **سقف مجموعة العمل:** حزام الأمان SF1 SF1 `max_parallel = 3` نافذ كرتا ہے، جس سے ونڈو <= 196608 بايت بنتی ہے.

التضخيم:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## أداة JavaScript SDK

- **مانڈ:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **السكوب:** هذه التركيبة `iroha_js_host::sorafsMultiFetchLocal` تتكرر مرة أخرى، ومسلسل تحاليل طبية
  الحمولات والإيصالات وتقارير المزودين ولقطات لوحة النتائج ذات أبعاد متوازية.
- **حارس الأداء:** آخر 2 ثانية مكتملة؛ تسخير ماپی گئی مدت وسقف البايتات المحجوزة
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ كرتا ہے.مثال على خلاصة الإنترنت:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير سويفت SDK

- **مانڈ:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **السكوب:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` عرض تكافؤ جناح التكافؤ،
  جسر Norito (`sorafsLocalFetch`) مثبت على SF1 مثبت مرتين. تسخير بايت الحمولة، إيصالات قطعة،
  تقارير الموفر وإدخالات لوحة النتائج هي بيانات تعريف الموفر الحتمية ولقطات القياس عن بعد التي يتم إجراؤها يوميًا
  توجد أجنحة Gu Rust/JS.
- **Bridge bootstrap:** الحزام ضروري بما يتوافق مع `dist/NoritoBridge.xcframework.zip` لتفريغ الحزمة و`dlopen`.
  تنزيل شريحة macOS. لم يتم العثور على روابط xcframework غدًا أو SoraFS الموجودة
  `cargo build -p connect_norito_bridge --release` للرجوع الاحتياطي و
  `target/release/libconnect_norito_bridge.dylib` لا يوجد اتصال به، ولا يوجد به CI على هذا الموقع.
- **Performance Guard:** يتم تشغيل CI لمدة ثانيتين فقط؛ تسخير ماپی گئی مدت وسقف البايتات المحجوزة
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ كرتا ہے.

مثال على خلاصة الإنترنت:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير ربط بايثون- **مانڈ:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **السكوب:** المجمع العالمي `iroha_python.sorafs.multi_fetch_local` وفئات البيانات المكتوبة التي تستخدم كرتا
  يتم استخدام أداة التثبيت المتعارف عليها API باستخدام عجلة جيس. ٹیستٹ `providers.json` هو البيانات الوصفية لموفر الخدمة مرة أخرى،
  حقن لقطة القياس عن بعد، وبايتات الحمولة، وإيصالات القطع، وتقارير الموفر، ومحتويات لوحة النتائج في مجموعات Rust/JS/Swift
  هذه خطة فائضة للكرتا.
- **الطلب المسبق:** `maturin develop --release` ملزمة (أو عجلة عجلة) حتى `_crypto` `sorafs_multi_fetch_local` ملزمة؛
  إذا لم يكن هناك جهاز ربط، فلا يمكنك استخدام الحزام للتخطي أو التخطي.
- **حارس الأداء:** جناح الصدأ جیسا <= 2 ق بجٹ؛ pytest عدد البايتات المجمعة وملخص مشاركة المزود وإصدار قطعة أثرية
  کے لئے لاگ كرتا ہے۔

ريليز گیٹنگ کو ہر تسخير (Rust، Python، JS، Swift) ملخص الإخراج محفوظات کرنی چاہیے محفوظات تم نشرها بناء کو تعزيز
يمكن مقارنة إيصالات ومقاييس الحمولة الصافية بالكامل. تمام مجموعات التكافؤ (Rust، Python Bindings، JS، Swift)
الذي يحتوي على رقم واحد `ci/sdk_sorafs_orchestrator.sh`؛ عناصر CI التي هي بمثابة مساعد للكابلات والسجلات
`matrix.md` (SDK/جدول الحالة/المدة) يبرز عدد كبير من المراجعين الذين قاموا بتدقيق مصفوفة التكافؤ للتكرار المنخفض
كر سكي.