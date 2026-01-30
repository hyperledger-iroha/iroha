---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS Orchestrator GA برابری رپورٹ

Deterministic multi-fetch برابری اب ہر SDK کے حساب سے ٹریک کی جاتی ہے تاکہ ریلیز انجینئرز یہ تصدیق کر سکیں کہ
payload bytes، chunk receipts، provider reports اور scoreboard کے نتائج مختلف implementations کے درمیان ہم آہنگ رہیں۔
ہر harness `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` کے تحت canonical multi-provider bundle استعمال کرتا ہے،
جو SF1 plan، provider metadata، telemetry snapshot اور orchestrator options کو پیک کرتا ہے۔

## Rust بیس لائن

- **کمانڈ:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **اسکوپ:** `MultiPeerFixture` plan کو in-process orchestrator کے ذریعے دو بار چلاتا ہے، assembled payload bytes،
  chunk receipts، provider reports اور scoreboard کے نتائج کی توثیق کرتا ہے۔ انسٹرومنٹیشن peak concurrency اور
  موثر working-set سائز (`max_parallel x max_chunk_length`) کو بھی ٹریک کرتی ہے۔
- **Performance guard:** ہر رن CI ہارڈویئر پر 2 s کے اندر مکمل ہونا چاہیے۔
- **Working set ceiling:** SF1 پروفائل کے ساتھ harness `max_parallel = 3` نافذ کرتا ہے، جس سے ونڈو <= 196608 bytes بنتی ہے۔

نمونہ لاگ آؤٹ پٹ:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK Harness

- **کمانڈ:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **اسکوپ:** اسی fixture کو `iroha_js_host::sorafsMultiFetchLocal` کے ذریعے دوبارہ چلاتا ہے، اور مسلسل رنز کے درمیان
  payloads، receipts، provider reports اور scoreboard snapshots کا موازنہ کرتا ہے۔
- **Performance guard:** ہر اجرا 2 s میں مکمل ہونا چاہیے؛ harness ماپی گئی مدت اور reserved-bytes ceiling
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

مثالی خلاصہ لائن:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK Harness

- **کمانڈ:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **اسکوپ:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں تعریف کردہ parity suite چلاتا ہے،
  Norito bridge (`sorafsLocalFetch`) کے ذریعے SF1 fixture کو دو بار ری پلے کرتا ہے۔ harness payload bytes، chunk receipts،
  provider reports اور scoreboard entries کو اسی deterministic provider metadata اور telemetry snapshots کے ساتھ ویریفائی کرتا ہے
  جو Rust/JS suites میں ہیں۔
- **Bridge bootstrap:** harness ضرورت کے مطابق `dist/NoritoBridge.xcframework.zip` کو unpack کرتا ہے اور `dlopen` کے ذریعے
  macOS slice لوڈ کرتا ہے۔ جب xcframework غائب ہو یا SoraFS bindings موجود نہ ہوں تو یہ
  `cargo build -p connect_norito_bridge --release` پر fallback کرتا ہے اور
  `target/release/libconnect_norito_bridge.dylib` کے ساتھ لنک کرتا ہے، اس لئے CI میں دستی سیٹ اپ درکار نہیں۔
- **Performance guard:** ہر اجرا CI ہارڈویئر پر 2 s میں مکمل ہونا چاہیے؛ harness ماپی گئی مدت اور reserved-bytes ceiling
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

مثالی خلاصہ لائن:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bindings Harness

- **کمانڈ:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **اسکوپ:** اعلی سطحی `iroha_python.sorafs.multi_fetch_local` wrapper اور اس کے typed dataclasses کو استعمال کرتا ہے تاکہ
  canonical fixture اسی API سے گزرے جسے wheel صارفین استعمال کرتے ہیں۔ ٹیسٹ `providers.json` سے provider metadata دوبارہ بناتا ہے،
  telemetry snapshot inject کرتا ہے، اور payload bytes، chunk receipts، provider reports اور scoreboard مواد کو Rust/JS/Swift suites
  کی طرح ویریفائی کرتا ہے۔
- **Pre-req:** `maturin develop --release` چلائیں (یا wheel انسٹال کریں) تاکہ `_crypto` `sorafs_multi_fetch_local` binding ظاہر کرے؛
  اگر binding دستیاب نہ ہو تو harness خودکار طور پر skip ہو جاتا ہے۔
- **Performance guard:** Rust suite جیسا <= 2 s بجٹ؛ pytest assembled byte count اور provider participation summary کو release artefact
  کے لئے لاگ کرتا ہے۔

ریلیز گیٹنگ کو ہر harness (Rust, Python, JS, Swift) کی summary output محفوظ کرنی چاہیے تاکہ محفوظ شدہ رپورٹ build کو promote
کرنے سے پہلے payload receipts اور metrics کو یکساں طور پر compare کر سکے۔ تمام parity suites (Rust, Python bindings, JS, Swift)
کو ایک پاس میں چلانے کے لئے `ci/sdk_sorafs_orchestrator.sh` چلائیں؛ CI artifacts کو اس helper سے لاگ کا اقتباس اور تیار کردہ
`matrix.md` (SDK/status/duration جدول) ریلیز ٹکٹ کے ساتھ جوڑنا چاہیے تاکہ reviewers لوکل پر دوبارہ چلائے بغیر parity matrix کا audit
کر سکیں۔
