---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Rapport de parité GA Orchestrator

La parité déterministe de récupérations multiples est désormais suivie par SDK afin que les ingénieurs de version puissent confirmer que
les octets de charge utile, les reçus de blocs, les rapports des fournisseurs et les résultats du tableau de bord restent alignés
mises en œuvre. Chaque harnais consomme le bundle canonique multi-fournisseurs sous
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, qui regroupe le plan SF1, fournisseur
options de métadonnées, d’instantané de télémétrie et d’orchestrateur.

## Base de référence de rouille

- **Commande :** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Portée :** exécute le plan `MultiPeerFixture` deux fois via l'orchestrateur en cours, en vérifiant
  octets de charge utile assemblés, reçus de fragments, rapports des fournisseurs et résultats du tableau de bord. Instruments
  suit également la simultanéité maximale et la taille effective de l'ensemble de travail (`max_parallel × max_chunk_length`).
- **Performance Guard :** Chaque exécution doit être terminée en 2 s sur le matériel CI.
- **Plafond de travail :** Avec le profil SF1, le harnais applique `max_parallel = 3`, ce qui donne un
  Fenêtre ≤ 196608 octets.

Exemple de sortie de journal :

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harnais du SDK JavaScript

- **Commande :** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Portée :** Rejoue le même appareil via `iroha_js_host::sorafsMultiFetchLocal`, en comparant les charges utiles,
  reçus, rapports des fournisseurs et instantanés du tableau de bord lors d'exécutions consécutives.
- **Garde de performance :** Chaque exécution doit se terminer en 2 s ; le harnais imprime la mesure
  durée et plafond d’octets réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemple de ligne de résumé :

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais du SDK Swift

- **Commande :** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Portée :** Exécute la suite de parité définie dans `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  rejouer le luminaire SF1 deux fois via le pont Norito (`sorafsLocalFetch`). Le harnais vérifie
  octets de charge utile, reçus de blocs, rapports des fournisseurs et entrées du tableau de bord utilisant le même système déterministe
  métadonnées du fournisseur et instantanés de télémétrie comme les suites Rust/JS.
- **Bridge bootstrap :** Le harnais déballe `dist/NoritoBridge.xcframework.zip` à la demande et charge
  la tranche macOS via `dlopen`. Lorsque xcframework est manquant ou ne dispose pas des liaisons SoraFS, il
  revient à `cargo build -p connect_norito_bridge --release` et établit des liens vers
  `target/release/libconnect_norito_bridge.dylib`, aucune configuration manuelle n'est donc requise dans CI.
- **Performance guard :** Chaque exécution doit se terminer en 2 s sur le matériel CI ; le harnais imprime le
  durée mesurée et plafond d’octets réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Exemple de ligne de résumé :

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harnais de liaisons Python- **Commande :** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Portée :** exerce le wrapper `iroha_python.sorafs.multi_fetch_local` de haut niveau et son typé
  classes de données afin que l'appareil canonique passe par la même API que celle que les consommateurs de roues appellent. L'épreuve
  reconstruit les métadonnées du fournisseur à partir de `providers.json`, injecte l'instantané de télémétrie et vérifie
  octets de charge utile, reçus de fragments, rapports des fournisseurs et contenu du tableau de bord, tout comme Rust/JS/Swift
  suites.
- **Pré-requis :** Exécutez `maturin develop --release` (ou installez la roue) pour que `_crypto` expose le
  Liaison `sorafs_multi_fetch_local` avant d'appeler pytest ; le harnais saute automatiquement lorsque la fixation
  n'est pas disponible.
- **Garantie des performances :** Même budget ≤2s que la suite Rust ; pytest enregistre le nombre d'octets assemblés
  et un résumé de la participation du fournisseur pour l'artefact de publication.

Le release gating doit capturer le résultat récapitulatif de chaque harnais (Rust, Python, JS, Swift) afin que le
Le rapport archivé peut différencier uniformément les reçus et les métriques de charge utile avant de promouvoir une build. Courir
`ci/sdk_sorafs_orchestrator.sh` pour exécuter chaque suite de parité (Rust, liaisons Python, JS, Swift) dans
un passage ; Les artefacts CI doivent joindre l'extrait de journal de cet assistant ainsi que le fichier généré
`matrix.md` (tableau SDK/statut/durée) au ticket de version afin que les réviseurs puissent auditer la parité
matrice sans réexécuter la suite localement.