---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Orquestrador GA برابری رپورٹ

Multibusca determinística برابری اب ہر SDK کے حساب سے ٹریک کی جاتی ہے تاکہ ریلیز انجینئرز یہ تصدیق کر سکیں کہ
bytes de carga útil, recibos de blocos, relatórios de provedores e placar کے نتائج مختلف implementações کے درمیان ہم آہنگ رہیں۔
ہر aproveitar `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` کے تحت pacote canônico de vários provedores استعمال کرتا ہے،
No plano SF1, nos metadados do provedor, no instantâneo de telemetria e nas opções do orquestrador.

## Ferrugem

- **کمانڈ:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **اسکوپ:** Plano `MultiPeerFixture` کو orquestrador em processo کے ذریعے دو بار چلاتا ہے, bytes de carga útil montados,
  recibos de pedaços, relatórios do provedor e placar کے نتائج کی توثیق کرتا ہے۔ انسٹرومنٹیشن pico de simultaneidade
  Conjunto de trabalho سائز (`max_parallel x max_chunk_length`) کو بھی ٹریک کرتی ہے۔
- **Guarda de desempenho:** ہر رن CI ہارڈویئر پر 2 s کے اندر مکمل ہونا چاہیے۔
- **Teto definido de trabalho:** SF1 پروفائل کے ساتھ arnês `max_parallel = 3` نافذ کرتا ہے، جس سے ونڈو <= 196608 bytes بنتی ہے۔

O que você precisa saber:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Equipamento do SDK JavaScript

- **کمانڈ:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **اسکوپ:** اسی fixture کو `iroha_js_host::sorafsMultiFetchLocal` کے ذریعے دوبارہ چلاتا ہے، اور مسلسل رنز کے درمیان
  cargas úteis, recibos, relatórios de provedores e instantâneos de placar
- **Guarda de desempenho:** ہر اجرا 2 s میں مکمل ہونا چاہیے؛ chicote ماپی گئی مدت اور teto de bytes reservados
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

مثالی خلاصہ لائن:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Equipamento do SDK Swift

- **کمانڈ:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **اسکوپ:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں تعریف کردہ conjunto de paridade چلاتا ہے،
  Ponte Norito (`sorafsLocalFetch`) کے ذریعے Fixação SF1 کو دو بار ری پلے کرتا ہے۔ aproveitar bytes de carga útil, recibos de pedaços,
  relatórios do provedor اور entradas do placar کو اسی metadados determinísticos do provedor اور instantâneos de telemetria کے ساتھ ویریفائی کرتا ہے
  As suítes Rust/JS não são compatíveis
- **Bridge bootstrap:** chicote ضرورت کے مطابق `dist/NoritoBridge.xcframework.zip` کو descompactar کرتا ہے اور `dlopen` کے ذریعے
  fatia do macOS لوڈ کرتا ہے۔ Por meio do xcframework, há ligações SoraFS.
  `cargo build -p connect_norito_bridge --release` é um substituto alternativo
  `target/release/libconnect_norito_bridge.dylib` کے ساتھ لنک کرتا ہے, اس لئے CI میں دستی سیٹ اپ درکار نہیں۔
- **Guarda de desempenho:** ہر اجرا CI ہارڈویئر پر 2 s میں مکمل ہونا چاہیے؛ chicote ماپی گئی مدت اور teto de bytes reservados
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

مثالی خلاصہ لائن:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Chicote de ligações Python- **کمانڈ:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **اسکوپ:** اعلی سطحی `iroha_python.sorafs.multi_fetch_local` wrapper اور اس کے dataclasses digitadas کو استعمال کرتا ہے تاکہ
  acessório canônico اسی API سے گزرے جسے roda صارفین استعمال کرتے ہیں۔ ٹیسٹ `providers.json` سے metadados do provedor دوبارہ بناتا ہے،
  injeção de instantâneo de telemetria کرتا ہے، اور bytes de carga útil, recebimentos de pedaços, relatórios de provedor اور placar مواد کو Rust/JS/Swift suites
  کی طرح ویریفائی کرتا ہے۔
- **Pré-requisito:** `maturin develop --release` چلائیں (یا roda انسٹال کریں) تاکہ `_crypto` `sorafs_multi_fetch_local` ligação ظاہر کرے؛
  اگر vinculativo دستیاب نہ ہو تو arnês خودکار طور پر pular ہو جاتا ہے۔
- **Proteção de desempenho:** Suíte Rust جیسا <= 2 s بجٹ؛ contagem de bytes montada em pytest e resumo de participação do provedor e artefato de liberação
  کے لئے لاگ کرتا ہے۔

ریلیز گیٹنگ کو ہر aproveitar (Rust, Python, JS, Swift) کی saída de resumo محفوظ کرنی چاہیے تاکہ محفوظ شدہ رپورٹ construir کو promover
کرنے سے پہلے recibos de carga útil e métricas کو یکساں طور پر comparar کر سکے۔ Conjuntos de paridade (Rust, ligações Python, JS, Swift)
O que há de errado com o `ci/sdk_sorafs_orchestrator.sh` چلائیں؛ Artefatos de CI são um ajudante que pode ser usado para criar artefatos de CI
`matrix.md` (SDK/status/duração جدول) ریلیز ٹکٹ کے ساتھ جوڑنا چاہیے تاکہ revisores لوکل پر دوبارہ چلائے بغیر matriz de paridade کا auditoria
کر سکیں۔