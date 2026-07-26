---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de parité GA SoraFS Orchestrator

La parité déterministe multi-fetch est désormais suivie par SDK afin que les
les ingénieurs de publication peuvent confirmer que les octets de charge utile, les reçus de blocs,
Les rapports des fournisseurs et les résultats de scoreboard restent alignés entre
mises en œuvre. Chaque harnais consomme le bundle multi-fournisseur canonique dans
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, qui regroupe le plan SF1,
le fournisseur de métadonnées, l'instantané de télémétrie et les options d'orchestrator.

## Base de référence de rouille

- **Commande :** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Scope :** Exécuter le plan `MultiPeerFixture` deux fois via l'orchestrator in-process,
  en vérifiant les octets de payload assemblés, les chunk reçus, les rapports des fournisseurs et
  résultats du tableau de bord. L'instrumentation suit aussi la concurrence de pointe
  et la taille effective du working-set (`max_parallel × max_chunk_length`).
- **Performance guard:** Chaque exécution doit finir en 2 s sur le matériel CI.
- **Ensemble de travail plafond :** Avec le profil SF1, le harnais impose `max_parallel = 3`,
  donnant une fenêtre ≤ 196608 octets.

Exemple de sortie de journal :

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harnais du SDK JavaScript

- **Commande :** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** Rejouer la même luminaire via `iroha_js_host::sorafsMultiFetchLocal`,
  comparer les charges utiles, les reçus, les rapports des fournisseurs et les instantanés du tableau de bord entre
  exécutions consécutives.
- **Performance guard:** Chaque exécution doit finir en 2 s ; le harnais imprime la
  durée enregistrée et le plafond de bytes réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemple de ligne de résumé :

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais du SDK Swift

- **Commande :** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Scope:** Exécute la suite de parité définie dans `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  rejouant la luminaire SF1 deux fois via le pont Norito (`sorafsLocalFetch`). Le harnais vérifie
  les octets de charge utile, les reçus en morceaux, les rapports des fournisseurs et les entrées de tableau de bord en utilisant les
  même fournisseur de métadonnées déterministes et instantanés de télémétrie que les suites Rust/JS.
- **Bridge bootstrap:** Le harnais décompresse `dist/NoritoBridge.xcframework.zip` à la demande et charge
  le slice macOS via `dlopen`. Lorsque l'xcframework est manquant ou n'a pas les liaisons SoraFS, il
  bascule sur `cargo build -p connect_norito_bridge --release` et se trouve à
  `target/release/libconnect_norito_bridge.dylib`, sans manuel de configuration et CI.
- **Performance guard:** Chaque exécution doit finir en 2 s sur le matériel CI ; le harnais imprime la
  durée enregistrée et le plafond de bytes réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemple de ligne de résumé :

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais de liaisons Python- **Commande :** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** Exerce le wrapper haut niveau `iroha_python.sorafs.multi_fetch_local` et ses dataclasses typées
  afin que la fixation canonique passe par la même API que les consommateurs de wheel. Le test de reconstitution
  la metadata supplier depuis `providers.json`, injecte le snapshot de télémétrie et vérifie les octets de payload,
  chunk reçus, supplier reports et contenu du scoreboard comme les suites Rust/JS/Swift.
- **Pre-req:** Exécutez `maturin develop --release` (ou installez la wheel) pour que `_crypto` expose le contraignant
  `sorafs_multi_fetch_local` avant d'appeler pytest ; le harnais saute automatiquement lorsque la liaison est indisponible.
- **Performance guard:** Même budget ≤ 2 s que la suite Rust ; pytest enregistre le nombre d'octets assemblés
  et le résumé de participation des prestataires pour l'artefact de libération.

Le release gating doit capturer la sortie récapitulative de chaque harnais (Rust, Python, JS, Swift) afin que
le rapport archivé peut comparer les recettes de charge utile et les métriques de manière uniforme avant de
promouvoir une construction. Exécutez `ci/sdk_sorafs_orchestrator.sh` pour lancer chaque suite de parité
(Rust, liaisons Python, JS, Swift) en une seule passe ; les artefacts CI doivent joindre l'extrait
log de ce helper plus le `matrix.md` généré (table SDK/status/durée) au ticket de release afin que
les reviewers peuvent auditer la matrice de parité sans relancer la suite localement.