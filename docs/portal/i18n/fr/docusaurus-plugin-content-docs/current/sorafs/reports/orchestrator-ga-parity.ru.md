---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ouvrir la session GA pour SoraFS Orchestrator

La détermination du paramètre multi-fetch doit être effectuée à partir du SDK, par exemple
les ingénieurs de publication ont été chargés de la charge utile en octets, des reçus de fragments, du fournisseur
les rapports et le tableau de bord des résultats permettent de réaliser des rapports.
Ce harnais utilise un ensemble multi-fournisseurs canonique de
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, pour le plan SF1,
métadonnées du fournisseur, instantané de télémétrie et orchestrateur d'options.

## Base de référence de rouille

- **Commande :** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Portée :** Le plan `MultiPeerFixture` concerne l'orchestrateur en cours,
  vérification des octets de charge utile, des reçus de blocs, des rapports du fournisseur et des résultats
  tableau de bord. L'instrument permet d'obtenir une configuration et un effet efficaces
  размер ensemble de travail (`max_parallel × max_chunk_length`).
- **Performance guard :** Ce processus prend plus de 2 s sur le matériel CI.
- **Ensemble de travail plafond :** Pour le profil SF1, le harnais est `max_parallel = 3`,
  давая окно ≤ 196608 octets.

Exemple de sortie de journal :

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harnais du SDK JavaScript

- **Commande :** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Portée :** Programmez votre luminaire à partir de `iroha_js_host::sorafsMultiFetchLocal`,
  charge utile, reçus, rapports des fournisseurs et instantanés du tableau de bord
  последовательными запусками.
- **Garde de performance :** Каждый запуск должен завершиться за 2 s ; harnais
  измеренную длительность и потолок octets réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemple de ligne de résumé :

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais du SDK Swift

- **Commande :** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Portée :** Ajouter la suite de parité à partir de `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  Le luminaire SF1 doit être équipé du pont Norito (`sorafsLocalFetch`). Harnais testé
  octets de charge utile, reçus de blocs, rapports du fournisseur et tableau de bord des entrées, utilisation de tout cela
  Déterminez les métadonnées du fournisseur et les instantanés de télémétrie, ainsi que les suites Rust/JS.
- **Bridge bootstrap :** Le harnais associe `dist/NoritoBridge.xcframework.zip` au travail et
  Téléchargez la tranche macOS à partir de `dlopen`. Lorsque xcframework fonctionne ou ne prend pas en charge les liaisons SoraFS,
  Vous pouvez utiliser le repli sur `cargo build -p connect_norito_bridge --release` et la connexion avec
  `target/release/libconnect_norito_bridge.dylib`, sans frais de base chez CI.
- **Performance guard :** Le délai d'attente est de 2 s pour le matériel CI ; harnais
  измеренную длительность и потолок octets réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemple de ligne de résumé :

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais de liaisons Python- **Commande :** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Portée :** Test du wrapper de haut niveau `iroha_python.sorafs.multi_fetch_local` et de votre type
  dataclasses, les appareils canoniques créés par votre API, que vous pouvez utiliser
  roue des consommateurs. Testez les métadonnées du fournisseur depuis `providers.json`, en cliquant sur
  instantané de télémétrie et vérification des octets de charge utile, des reçus de blocs, des rapports du fournisseur et
  Il existe un tableau de bord similaire à celui des suites Rust/JS/Swift.
- **Pré-requis :** Choisissez `maturin develop --release` (ou installez la roue), puis `_crypto`.
  J'ai ouvert la liaison `sorafs_multi_fetch_local` avant pytest ; harnais авто-скипает,
  когда contraignant недоступен.
- **Performance guard :** Тот же бюджет ≤ 2 s, что и у Rust suite ; pytest logifie les choses
  собранных octets et résumé des fournisseurs pour la publication de l'artefact.

Release Gating doit être utilisé pour le résumé de la sortie du faisceau (Rust, Python, JS, Swift), ici
L'archivage permet de récupérer les reçus de charge utile et les mesures élaborées avant la production
construire. Utilisez `ci/sdk_sorafs_orchestrator.sh` pour utiliser vos suites de parité
(Rust, liaisons Python, JS, Swift) pour cette méthode ; Les artefacts CI doivent être extraits du journal
из этого helper и сгенерированный `matrix.md` (таблица SDK/statut/duration) k release ticket,
Les évaluateurs peuvent auditer la matrice de parité avant le programme local.