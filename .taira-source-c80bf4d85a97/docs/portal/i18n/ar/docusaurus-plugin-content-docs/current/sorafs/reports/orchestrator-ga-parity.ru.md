---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# قم بإلغاء ربط GA لـ SoraFS Orchestrator

يتم تحديد نطاق الجلب المتعدد من خلال SDK، لذلك
يمكن لمهندسي الإصدار معرفة الحمولة البايتة وإيصالات القطع والموفر
يتم دعم التقارير ولوحة النتائج من خلال تحقيق النتائج.
يستخدم تسخير الكلاب حزمة أساسية متعددة الموفرين من
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`، والتي تتضمن خطة SF1،
بيانات تعريف الموفر، لقطة القياس عن بعد، ومنسق الخيارات.

## الصدأ الأساس

- **الأمر:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** كتابة الخطة `MultiPeerFixture` يتم تنفيذها من خلال المنسق قيد التشغيل،
  التحقق من بايتات الحمولة الصافية وإيصالات القطع وتقارير الموفر والنتائج
  لوحة النتائج. تعمل الأدوات أيضًا على تحسين التوافق والفعالية
  حجم مجموعة العمل (`max_parallel × max_chunk_length`).
- **حارس الأداء:** يتم إغلاق كل مرحلة لمدة ثانيتين على أجهزة CI.
- **سقف مجموعة العمل:** لملف تعريف SF1، `max_parallel = 3`،
  تم الضغط عليه ≥ 196608 بايت.

نموذج لإخراج السجل:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## أداة JavaScript SDK- **الأمر:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **النطاق:** يمكنك تثبيته وتركيبه عبر `iroha_js_host::sorafsMultiFetchLocal`،
  الحمولات الحقيقية والإيصالات وتقارير الموفر ولقطات لوحة النتائج من خلال
  التخفيضات اللاحقة.
- **حارس الأداء:** يتم إغلاق كل ثانية لمدة ثانيتين؛ تسخير печатаеt
  تقليص حجم البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

مثال على سطر التلخيص:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير سويفت SDK

- **الأمر:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق:** شراء مجموعة التكافؤ من `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`،
  يتم تثبيت تركيبات SF1 عبر جسر Norito (`sorafsLocalFetch`). التحقق من تسخير
  بايتات الحمولة، وإيصالات القطع، وتقارير الموفر، ولوحة تسجيل الإدخالات، وما إلى ذلك
  تحديد البيانات الوصفية لموفر الخدمة ولقطات القياس عن بعد، والتي تتضمن Rust/JS.
- **Bridge bootstrap:** Harness распаковывает `dist/NoritoBridge.xcframework.zip` по тебованию и
  قم بتنزيل شريحة macOS من خلال `dlopen`. عندما يخرج xcframework أو لا ينضم إلى روابط SoraFS،
  يتم استخدام الخيار الاحتياطي لـ `cargo build -p connect_norito_bridge --release` والارتباط به
  `target/release/libconnect_norito_bridge.dylib`، بدون إعدادات بسيطة في CI.
- **حارس الأداء:** يتم إغلاق كل مرة لمدة ثانيتين على أجهزة CI؛ تسخير печатаеt
  تقليص حجم البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

مثال على سطر التلخيص:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير ربط بايثون- **الأمر:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **النطاق:** التحقق من المجمع عالي المستوى `iroha_python.sorafs.multi_fetch_local` وهو مكتوب
  فئات البيانات التي يتم تقديمها من خلال واجهة برمجة التطبيقات (API) التي يتم اختيارها
  عجلة المستهلكين. يختبر الاختبار بيانات تعريف الموفر من `providers.json`، ثم يتم إدخالها
  لقطة القياس عن بعد والتحقق من بايتات الحمولة وإيصالات القطع وتقارير الموفر وما إلى ذلك
  بالإضافة إلى لوحة النتائج مثل أجنحة Rust/JS/Swift.
- **الطلب المسبق:** قم بتثبيت `maturin develop --release` (أو قم بتثبيت العجلة)، إلى `_crypto`
  تم فتح الرابط `sorafs_multi_fetch_local` قبل pytest; تسخير السيارات سكايبايت,
  عندما تكون ملزمة غير مستقرة.
- **حارس الأداء:** تبلغ الميزانية ≥ 2 ثانية في مجموعة Rust؛ pytest логиruет число
  وحدات البايت المجمعة وموفرو الرعاية الموجزة لإصدار العناصر.

يجب أن يؤدي تحرير البوابة إلى إلغاء ملخص الإخراج من خلال تسخير (Rust، Python، JS، Swift)، لذلك
يمكنك أرشفة إيصالات الحمولة والمقاييس بشكل فردي قبل التسليم
بناء. قم بتثبيت `ci/sdk_sorafs_orchestrator.sh` لاستخدام جميع مجموعات التكافؤ
(الصدأ، روابط بايثون، JS، سويفت) لسبب واحد؛ يجب أن تقوم عناصر CI باستخدام مقتطفات من السجل
من هذا المساعد والمصمم `matrix.md` (بطاقة SDK/الحالة/المدة) لتذكرة الإصدار،
لأنه يمكن للمراجعين مراجعة مصفوفة التكافؤ دون التقدم المحلي.