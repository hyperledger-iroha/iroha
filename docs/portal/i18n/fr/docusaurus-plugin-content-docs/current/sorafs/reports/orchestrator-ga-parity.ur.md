---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Orchestrator GA disponible

Multi-fetch déterministe pour le SDK et le SDK. سکیں کہ
octets de charge utile, reçus de blocs, rapports du fournisseur, tableau de bord, mises en œuvre et mises en œuvre de données.
Le harnais `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` est un ensemble canonique multi-fournisseurs disponible pour tous.
Avec le plan SF1, les métadonnées du fournisseur, l'instantané de télémétrie et les options de l'orchestrateur

## Rust pour vous

- **کمانڈ :** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **اسکوپ:** Plan `MultiPeerFixture` pour un orchestrateur en cours et un orchestrateur en cours pour créer des octets de charge utile assemblés.
  reçus en morceaux, rapports du fournisseur et tableau de bord انسٹرومنٹیشن pic de concurrence اور
  Le groupe de travail est un ensemble de travail (`max_parallel x max_chunk_length`) qui est en cours de réalisation.
- **Performance guard :** ہر رن CI ہارڈویئر پر 2 s کے اندر مکمل ہونا چاہیے۔
- **Plafond de travail :** SF1 faisceau de câbles `max_parallel = 3` Nombre de fils <= 196608 octets

نمونہ لاگ آؤٹ پٹ:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harnais du SDK JavaScript

- **کمانڈ :** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **اسکوپ:** اسی luminaire کو `iroha_js_host::sorafsMultiFetchLocal` کے ذریعے دوبارہ چلاتا ہے، اور مسلسل رنز کے درمیان
  charges utiles, reçus, rapports du fournisseur et instantanés du tableau de bord.
- **Garde de performance :** ہر اجرا 2 s میں مکمل ہونا چاہیے؛ harnais ماپی گئی مدت اور plafond d'octets réservés
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

مثالی خلاصہ لائن:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais du SDK Swift

- **کمانڈ :** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **اسکوپ:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں تعریف کردہ suite de parité چلاتا ہے،
  Pont Norito (`sorafsLocalFetch`) pour le luminaire SF1 et pour le montage en ligne exploiter les octets de charge utile, les reçus de blocs,
  rapports du fournisseur et entrées du tableau de bord ainsi que métadonnées déterministes du fournisseur et instantanés de télémétrie
  Les suites Rust/JS sont disponibles
- **Bridge bootstrap :** harnais pour `dist/NoritoBridge.xcframework.zip` et décompresser pour `dlopen` pour `dlopen`
  tranche macOS en français Les liaisons xcframework sont liées aux liaisons SoraFS.
  `cargo build -p connect_norito_bridge --release` pour la solution de secours
  `target/release/libconnect_norito_bridge.dylib` ساتھ لنک کرتا ہے، اس لئے CI میں دستی سیٹ اپ درکار نہیں۔
- **Performance guard :** ہر اجرا CI ہارڈویئر پر 2 s میں مکمل ہونا چاہیے؛ harnais ماپی گئی مدت اور plafond d'octets réservés
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

مثالی خلاصہ لائن:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais de liaisons Python- **کمانڈ :** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **اسکوپ:** Il s'agit du wrapper `iroha_python.sorafs.multi_fetch_local` qui contient des classes de données typées et des classes de données typées.
  appareil canonique اسی API سے گزرے جسے wheel صارفین استعمال کرتے ہیں۔ ٹیسٹ `providers.json` pour les métadonnées du fournisseur دوبارہ بناتا ہے،
  l'instantané de télémétrie injecte des octets de charge utile, des reçus de blocs, des rapports du fournisseur, et un tableau de bord, ainsi que des suites Rust/JS/Swift
  کی طرح ویریفائی کرتا ہے۔
- **Pré-requis :** `maturin develop --release` (roue de roue) et `_crypto` `sorafs_multi_fetch_local` fixation de roue
  اگر contraignant دستیاب نہ ہو تو harnais خودکار طور پر sauter ہو جاتا ہے۔
- **Performance guard :** Suite Rust pendant <= 2 s pytest nombre d'octets assemblés et résumé de la participation du fournisseur et artefact de publication
  کے لئے لاگ کرتا ہے۔

Il s'agit d'un harnais (Rust, Python, JS, Swift) et d'un résumé de sortie pour construire et promouvoir
Il s'agit des reçus de charge utile et des métriques pour comparer les résultats. Exemples de suites de parité (Rust, liaisons Python, JS, Swift)
Il s'agit d'un produit `ci/sdk_sorafs_orchestrator.sh`. Les artefacts CI et l'assistant sont également disponibles.
`matrix.md` (SDK/statut/durée) Les réviseurs sont en ligne avec les réviseurs بغیر matrice de parité کا audit
کر سکیں۔