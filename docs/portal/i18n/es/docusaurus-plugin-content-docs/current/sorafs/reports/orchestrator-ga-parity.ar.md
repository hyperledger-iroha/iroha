---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير تكافؤ GA لمنسق SoraFS

يتم الآن تتبع تكافؤ multi-fetch الحتمي لكل SDK كي يتمكن مهندسو الإطلاق من تأكيد أن
Carga útil, fragmento, proveedor y marcador.
التطبيقات. كل arnés يستهلك الحزمة القياسية متعددة المزوّدين تحت
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, y تجمع خطة SF1 y بيانات
proveedor الوصفية ولقطة telemetría y orquestador.

## خط الأساس في Óxido

- **الأمر:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** يشغّل خطة `MultiPeerFixture` مرتين عبر orquestador داخل العملية، مع التحقق
  Hay una carga útil, un fragmento, un proveedor y un marcador. كما
  تتعقب أدوات القياس ذروة التوازي وحجم work-set الفعال (`max_parallel x max_chunk_length`).
- **حاجز الأداء:** يجب أن يكتمل كل تشغيل خلال 2 s على عتاد CI.
- **سقف مجموعة العمل:** مع ملف تعريف SF1 يفرض arnés `max_parallel = 3`, مما ينتج
  نافذة <= 196608 بايت.

Más información sobre:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Aprovechar el SDK de JavaScript

- **الأمر:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **النطاق:** يعيد تشغيل نفس accesorio عبر `iroha_js_host::sorafsMultiFetchLocal`, ويقارن
  cargas útiles, recibos, proveedores y marcadores.
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s؛ يطبع arnés المدة المقاسة وسقف
  البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

سطر ملخص مثال:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés para Swift SDK- **الأمر:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق:** يشغّل مجموعة التكافؤ المعرّفة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`،
  El accesorio SF1 se encuentra en el lugar Norito (`sorafsLocalFetch`). arnés
  من بايتات carga útil, fragmento, proveedor y marcador
  Proveedor de servicios de telemetría y telemetría mediante Rust/JS.
- **تهيئة الجسر:** يفك arnés ضغط `dist/NoritoBridge.xcframework.zip` عند الطلب ويحمل
  Utilice macOS para `dlopen`. عند غياب xcframework أو افتقاره لروابط SoraFS, يتراجع إلى
  `cargo build -p connect_norito_bridge --release` Más información
  `target/release/libconnect_norito_bridge.dylib`, لذا لا يلزم إعداد يدوي في CI.
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s على عتاد CI؛ ويطبع arnés المدة
  المقاسة وسقف البايتات المحجوزة (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

سطر ملخص مثال:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés de Python- **الأمر:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **النطاق:** يمارس contenedor عالي المستوى `iroha_python.sorafs.multi_fetch_local` y clases de datos
  المقيّدة بالأنواع كي تمر accesorio القياسية عبر نفس الواجهة التي يستخدمها مستهلكو
  rueda. يعيد الاختبار بناء بيانات proveedor الوصفية من `providers.json`, y يحقن
  Telemetría, carga útil, fragmentos, proveedor y proveedor
  marcador مثل حزم Rust/JS/Swift.
- **متطلب مسبق:** شغّل `maturin develop --release` (Rueda) كي يعرض `_crypto` ربط
  `sorafs_multi_fetch_local` mediante pytest يتخطى arnés تلقائيا عندما لا يكون
  الربط متاحا.
- **حاجز الأداء:** نفس حد <= 2 s مثل حزمة Rust؛ يسجل pytest عدد البايتات المجمعة
  وملخص مشاركة proveedores لقطعة الإصدار.

Puede usar un arnés (Rust, Python, JS y Swift)
Carga útil de la carga útil y carga útil de la batería
construir. شغّل `ci/sdk_sorafs_orchestrator.sh` لتنفيذ كل مجموعات التكافؤ (Rust y Python
enlaces وJS وSwift) في تمريرة واحدة؛ ينبغي أن تُرفق مخرجات CI مقتطف السجل من هذا
La configuración del sistema `matrix.md` (SDK/SDK/الحالة/المدة) está disponible
حتى يتمكن المراجعون تدقيق مصفوفة التكافؤ دون إعادة تشغيل المجموعة محليا.