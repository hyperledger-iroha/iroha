---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Orchestrator GA برابری رپورٹ

Búsqueda múltiple determinista سکیں کہ
bytes de carga útil, recibos de fragmentos, informes de proveedores, marcador, implementaciones de مختلف, کے درمیان ہم آہنگ رہیں۔
Arnés `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` کے تحت paquete multiproveedor canónico استعمال کرتا ہے،
جو plan SF1, metadatos del proveedor, instantánea de telemetría y opciones del orquestador کو پیک کرتا ہے۔

## Rust بیس لائن

- **کمانڈ:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **اسکوپ:** `MultiPeerFixture` plan کو orquestador en proceso کے ذریعے دو بار چلاتا ہے، bytes de carga útil ensamblados،
  recibos fragmentados, informes de proveedores اور marcador کے نتائج کی توثیق کرتا ہے۔ انسٹرومنٹیشن pico de concurrencia اور
  Conjunto de trabajo de موثر سائز (`max_parallel x max_chunk_length`) کو بھی ٹریک کرتی ہے۔
- **Guardia de rendimiento:** ہر رن CI ہارڈویئر پر 2 s کے اندر مکمل ہونا چاہیے۔
- **Límite del conjunto de trabajo:** SF1 پروفائل کے ساتھ arnés `max_parallel = 3` نافذ کرتا ہے، جس سے ونڈو <= 196608 bytes بنتی ہے۔

نمونہ لاگ آؤٹ پٹ:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Arnés del SDK de JavaScript

- **کمانڈ:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **اسکوپ:** اسی accesorio کو `iroha_js_host::sorafsMultiFetchLocal` کے ذریعے دوبارہ چلاتا ہے، اور مسلسل رنز کے درمیان
  cargas útiles, recibos, informes de proveedores e instantáneas del marcador کا موازنہ کرتا ہے۔
- **Guardia de rendimiento:** ہر اجرا 2 s میں مکمل ہونا چاہیے؛ aprovechar el límite máximo de bytes reservados
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔مثالی خلاصہ لائن:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés Swift SDK

- **کمانڈ:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **اسکوپ:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں تعریف کردہ suite de paridad چلاتا ہے،
  Puente Norito (`sorafsLocalFetch`) کے ذریعے Accesorio SF1 کو دو بار ری پلے کرتا ہے۔ aprovechar los bytes de carga útil, los recibos en trozos,
  informes de proveedores اور entradas del marcador کو اسی metadatos deterministas del proveedor اور instantáneas de telemetría کے ساتھ ویریفائی کرتا ہے
  جو Rust/JS suites میں ہیں۔
- **Bridge bootstrap:** arnés ضرورت کے مطابق `dist/NoritoBridge.xcframework.zip` کو descomprimir کرتا ہے اور `dlopen` کے ذریعے
  Rebanada de macOS لوڈ کرتا ہے۔ جب xcframework غائب ہو یا SoraFS enlaces موجود نہ ہوں تو یہ
  `cargo build -p connect_norito_bridge --release` پر respaldo کرتا ہے اور
  `target/release/libconnect_norito_bridge.dylib` کے ساتھ لنک کرتا ہے، اس لئے CI میں دستی سیٹ اپ درکار نہیں۔
- **Guardia de rendimiento:** ہر اجرا CI ہارڈویئر پر 2 s میں مکمل ہونا چاہیے؛ aprovechar el límite máximo de bytes reservados
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

مثالی خلاصہ لائن:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Arnés de fijaciones de Python- **کمانڈ:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **اسکوپ:** اعلی سطحی `iroha_python.sorafs.multi_fetch_local` contenedor اور اس کے clases de datos mecanografiadas کو استعمال کرتا ہے تاکہ
  accesorio canónico اسی API سے گزرے جسے rueda صارفین استعمال کرتے ہیں۔ ٹیسٹ `providers.json` سے metadatos del proveedor دوبارہ بناتا ہے،
  inyección de instantáneas de telemetría کرتا ہے، اور bytes de carga útil, recibos de fragmentos, informes del proveedor y marcador de Rust/JS/Swift suites
  کی طرح ویریفائی کرتا ہے۔
- **Requisito previo:** `maturin develop --release` چلائیں (یا rueda انسٹال کریں) تاکہ `_crypto` `sorafs_multi_fetch_local` vinculante ظاہر کرے؛
  اگر vinculante دستیاب نہ ہو تو arnés خودکار طور پر skip ہو جاتا ہے۔
- **Guardia de rendimiento:** Rust suite جیسا <= 2 s بجٹ؛ Recuento de bytes ensamblados de pytest y resumen de participación del proveedor y artefacto de lanzamiento
  کے لئے لاگ کرتا ہے۔

ریلیز گیٹنگ کو ہر arnés (Rust, Python, JS, Swift) کی resumen de salida محفوظ کرنی چاہیے تاکہ محفوظ شدہ رپورٹ build کو promover
کرنے سے پہلے recibos de carga útil اور métricas کو یکساں طور پر comparar کر سکے۔ Ampliar conjuntos de paridad (Rust, enlaces de Python, JS, Swift)
کو ایک پاس میں چلانے کے لئے `ci/sdk_sorafs_orchestrator.sh` چلائیں؛ Artefactos de CI کو اس ayudante سے لاگ کا اقتباس اور تیار کردہ
`matrix.md` (SDK/estado/duración جدول) ریلیز ٹکٹ کے ساتھ جوڑنا چاہیے تاکہ revisores لوکل پر دوبارہ چلائے Auditoría de matriz de paridad
کر سکیں۔