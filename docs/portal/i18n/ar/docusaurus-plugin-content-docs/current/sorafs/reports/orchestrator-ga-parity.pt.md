---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatorio de paridade GA do Orchestrator SoraFS

مجموعة من الحتمية متعددة الجلب والمراقبة بواسطة SDK لتأكيد ورثة الإصدار على ذلك
بايتات الحمولة، وإيصالات القطع، وتقارير الموفر، ونتائج لوحة النتائج الدائمة بين جميع
com.explinacoes. يستهلك كل تسخير أو حزمة Canonico متعددة الموفر
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`، الذي يتضمن مخطط SF1، وبيانات تعريف الموفر، ولقطة القياس عن بعد،
opcoes تفعل الأوركسترا.

## الصدأ الأساسي

- **الكوماندوز:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **الخبر:** تنفيذ المخطط `MultiPeerFixture` مرتين في المرة عبر المنسق قيد التشغيل، والتحقق
  بايتات من مجموعات الحمولة، وإيصالات القطع، وتقارير الموفر، ونتائج لوحة النتائج. أداة
  ويرافقه أيضًا التزامن البيكو وفعالية مجموعة العمل (`max_parallel x max_chunk_length`).
- **حارس الأداء:** يجب الانتهاء من كل تنفيذ من خلال 2 بدون أجهزة CI.
- **سقف مجموعة العمل:** مع ملف SF1 أو حزام الأمان المطبق `max_parallel = 3`، نتيجة لذلك
  جانيلا <= 196608 بايت.

مثال على سجل صيدا:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## استخدم JavaScript لـ SDK- **الكوماندوز:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **الشرح:** إعادة إنتاج نفس التركيبة عبر `iroha_js_host::sorafsMultiFetchLocal`، مقارنة الحمولات،
  تقوم الإيصالات وتقارير المزود واللقطات بعمل لوحة النتائج بين التنفيذين المتتاليين.
- **حارس الأداء:** يجب الانتهاء من كل تنفيذ في ثانيتين؛ o تسخير imprime a duracao medida e o
  عدد البايتات المحفوظة بالكامل (`max_parallel = 3`، `peak_reserved_bytes <= 196608`).

مثال على ملخص السيرة الذاتية:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير SDK Swift

- **الكوماندوز:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **الشرح:** تنفيذ مجموعة من الضوابط المحددة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`،
  إعادة إنتاج أو تركيب SF1 مرتين في كل مرة من الجسر Norito (`sorafsLocalFetch`). يا تسخير التحقق من بايت الحمولة،
  إيصالات القطع وتقارير المزود ومدخلات لوحة النتائج التي تستخدم حتمية البيانات الوصفية لموفر الخدمة
  لقطات القياس عن بعد das suites Rust/JS.
- **Bridge bootstrap:** O hardware discompacta `dist/NoritoBridge.xcframework.zip` sob requesta e carrega o strip macOS via
  `dlopen`. عندما يكون xcframework موجودًا أو لا توجد روابط SoraFS، يجب أن يكون هناك تراجع احتياطي
  `cargo build -p connect_norito_bridge --release` ورابط مضاد `target/release/libconnect_norito_bridge.dylib`,
  اتبع دليل الإعداد ومن الضروري استخدام CI.
- **حارس الأداء:** تم إنهاء كل تنفيذ في 2 ثانية بدون أجهزة CI؛ o تسخير imprime a duracao medida e o
  عدد البايتات المحفوظة بالكامل (`max_parallel = 3`، `peak_reserved_bytes <= 196608`).

مثال على ملخص السيرة الذاتية:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## تسخير روابط بايثون- **الكوماندوز:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **الدراسة:** تمرين على غلاف عالي المستوى `iroha_python.sorafs.multi_fetch_local` وفئات البيانات الخاصة بك
  نصائح لتمرير أداة التثبيت Canonico عبر واجهة برمجة التطبيقات (API) التي يستخدمها مستهلكو العجلة. يا اختبار
  إعادة إنشاء بيانات تعريف الموفر من `providers.json`، وإدخال لقطة قياس عن بعد والتحقق منها
  بايتات الحمولة، وإيصالات القطع، وتقارير الموفر، ومحتوى لوحة النتائج المشابهة لمجموعات Rust/JS/Swift.
- **الطلب المسبق:** تنفيذ `maturin develop --release` (أو تثبيت العجلة) لتوضيح `_crypto`
  ملزمة `sorafs_multi_fetch_local` antes de chamar pytest؛ o تسخير التجاهل التلقائي عند الربط
  لا يوجد شيء متاح.
- **حارس الأداء:** Mesmo orcamento <= 2 s da suite Rust; pytest يسجل عدوى البايتات
  الجبال واستئناف مشاركة مقدمي الخدمة لأداة الإصدار.

عليك أن تقوم بتحرير البوابات لتلتقط ملخصًا لإخراج كل تسخير (Rust وPython وJS وSwift) لذلك
يمكن للعلاقة المحفوظة مقارنة إيصالات الحمولة ومقاييس الشكل الموحد قبل الترقية
أم بناء. تنفيذ `ci/sdk_sorafs_orchestrator.sh` لتدوير جميع مجموعات الباريداد (Rust، Python
الارتباطات، JS، Swift) في ممر واحد؛ يجب على أدوات CI أن تقوم بكسر أو حفر سجل هذا المساعد
المزيد من `matrix.md` (تصنيف SDK/الحالة/المدة) إلى تذكرة الإصدار حتى يتمكن المراجعون من الحصول عليها
قم بمراجعة مصفوفة الجدار دون إعادة تنفيذ المجموعة محليًا.