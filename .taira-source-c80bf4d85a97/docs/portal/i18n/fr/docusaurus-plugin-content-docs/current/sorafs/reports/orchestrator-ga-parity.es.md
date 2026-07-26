---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de parité GA de l'Orchestrator SoraFS

La parité déterministe du multi-fetch est désormais rastrea par le SDK pour que les
les ingénieurs de publication peuvent confirmer que les octets de charge utile, les reçus de fragments,
rapports du fournisseur et résultats du tableau de bord permanents peuvent être alignés entre
mises en œuvre. Chaque harnais consomme le bundle multi-fournisseurs canonico bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, qui empaqueta le plan SF1,
métadonnées du fournisseur, instantané de télémétrie et options de l'orchestrateur.

## Base de référence de rouille

- **Commande :** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Portée :** Exécuter le plan `MultiPeerFixture` deux fois via l'orchestrateur en cours,
  vérifier les octets de charge utile ensamblados, les reçus de blocs, les rapports du fournisseur et
  résultats du tableau de bord. La instrumentación tambien rastrea concurrencia pico
  et le tamano efectivo del working-set (`max_parallel x max_chunk_length`).
- **Performance guard :** Chaque éjection doit être complète en 2 secondes sur le matériel CI.
- **Ensemble de travail plafond :** Avec le profil SF1 et l'application du harnais `max_parallel = 3`,
  donc une fenêtre <= 196608 octets.

Exemple de sortie de journal :

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harnais du SDK JavaScript

- **Commande :** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Portée :** Reproduire le luminaire el mismo via `iroha_js_host::sorafsMultiFetchLocal`,
  comparer les charges utiles, les reçus, les rapports des fournisseurs et les instantanés du tableau de bord entre
  ejecuciones consecutivas.
- **Performance guard:** Chaque éjection doit être finalisée en 2 s ; el harnais imprimer la
  durée moyenne et technologie des octets réservés (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemple de ligne de résumé :

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais du SDK Swift

- **Commande :** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Portée :** Exécuter la suite de parité définie en `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduciendo el luminaire SF1 dos veces a traves del bridge Norito (`sorafsLocalFetch`). Le harnais
  vérifier les octets de charge utile, les reçus de blocs, les rapports du fournisseur et les entrées du tableau de bord en utilisant le
  Il existe également des instantanés de métadonnées déterministes et de télémétrie du fournisseur des suites Rust/JS.
- **Bridge bootstrap :** Le harnais décomprime `dist/NoritoBridge.xcframework.zip` bas demandé et chargé
  el tranche macOS via `dlopen`. Si xcframework échoue ou n'a pas de liaisons SoraFS, vous devez utiliser une solution de secours
  `cargo build -p connect_norito_bridge --release` et lien contre
  `target/release/libconnect_norito_bridge.dylib`, sans manuel de configuration en CI.
- **Performance guard:** Chaque éjection doit être terminée en 2 secondes sur le matériel CI ; el harnais imprimer la
  durée moyenne et technologie des octets réservés (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemple de ligne de résumé :

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais de liaisons Python- **Commande :** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope :** Exécuter le wrapper de haut niveau `iroha_python.sorafs.multi_fetch_local` et ses classes de données
  astuces pour que le luminaire canonique fluya par la même API qui consomme les roues. Le test
  reconstruire les métadonnées du fournisseur à partir de `providers.json`, injecter l'instantané de télémétrie et vérifier
  octets de charge utile, reçus de blocs, rapports du fournisseur et contenu du tableau de bord identique à celui-ci
  suites Rust/JS/Swift.
- **Pré-requis :** Exécutez `maturin develop --release` (ou installez la roue) pour que `_crypto` étende la
  liaison `sorafs_multi_fetch_local` avant d'invoquer pytest ; el harnais se auto-salta cuando el
  la liaison n'est pas disponible.
- **Performance guard:** El mismo presupuesto <= 2 s que la suite Rust; pytest enregistre le contenu de
  octets ensamblados et le résumé de la participation des fournisseurs pour l'artefact de publication.

Le release gating doit capturer la sortie récapitulative de chaque harnais (Rust, Python, JS, Swift) pour cela
le rapport archivé peut comparer les reçus de charge utile et les mesures de forme uniforme avant de
promoteur un build. Exécution `ci/sdk_sorafs_orchestrator.sh` pour corriger chaque suite de parité
(Rust, liaisons Python, JS, Swift) dans une seule étape ; les artefacts de CI doivent être adjoints au
extrait du journal de cette aide mais le `matrix.md` généré (tableau de SDK/état/durée) du ticket
release pour que les évaluateurs vérifient la matrice de parité sans réexécuter la suite localement.