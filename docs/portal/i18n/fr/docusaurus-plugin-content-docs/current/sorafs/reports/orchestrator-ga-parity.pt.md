---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de parité GA pour Orchestrator SoraFS

La parité déterministe de l'heure de multi-fetch et surveillée par le SDK pour que les développeurs de versions confirment que
octets de charge utile, reçus de blocs, rapports du fournisseur et résultats du tableau de bord permanents alignés entre
mises en œuvre. Chaque harnais consomme un bundle canonique multi-fournisseurs
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, qui empacota le plan SF1, métadonnées du fournisseur, instantané de télémétrie et
les opcoes font orchestrateur.

## Rouille de base

- **Commande :** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo :** Exécution du plan `MultiPeerFixture` depuis l'orchestrateur en cours, vérification
  octets de charge utile montées, reçus de blocs, rapports du fournisseur et résultats du tableau de bord. Un instrumentacao
  Il accompagne également la correspondance entre le pic et le même effet sur l'ensemble de travail (`max_parallel x max_chunk_length`).
- **Performance guard :** Chaque exécution doit être conclue avec 2 sans matériel de CI.
- **Ensemble de travail plafond :** Avec le profil SF1 ou l'application du harnais `max_parallel = 3`, ce qui en fait un
  Janela <= 196608 octets.

Exemple de déclaration de journal :

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Exploiter le SDK JavaScript

- **Commande :** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo :** Reproduction du luminaire mesmo via `iroha_js_host::sorafsMultiFetchLocal`, comparaison des charges utiles,
  les reçus, les rapports des fournisseurs et les instantanés font le tableau de bord entre les exécutions consécutives.
- **Performance guard :** Chaque exécution doit être finalisée en 2 s ; o harnais imprimer un duracao medida e o
  nombre d'octets réservés (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemple de ligne de CV :

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Exploiter le SDK Swift

- **Commande :** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Escopo :** Exécute la suite de parité définie dans `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  reproduzindo o luminaire SF1 duas vezes pelo bridge Norito (`sorafsLocalFetch`). Pour exploiter la vérification des octets de charge utile,
  reçus en bloc, rapports du fournisseur et entrées dans le tableau de bord en utilisant les métadonnées déterministes du fournisseur Mesma
  instantanés de télémétrie des suites Rust/JS.
- **Bridge bootstrap :** Le harnais descompacta `dist/NoritoBridge.xcframework.zip` doit être demandé et chargé pour trancher macOS via
  `dlopen`. Lorsque xcframework est ausente ou n'a pas de liaisons SoraFS, une solution de secours pour
  `cargo build -p connect_norito_bridge --release` et lien contre `target/release/libconnect_norito_bridge.dylib`,
  Voici le manuel de configuration et nécessaire pour CI.
- **Performance guard :** Chaque exécution doit être terminée en 2 sans matériel de CI ; o harnais imprimer un duracao medida e o
  nombre d'octets réservés (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Exemple de ligne de CV :

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Exploiter les liaisons Python- **Commande :** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo :** Exercice ou wrapper de haut niveau `iroha_python.sorafs.multi_fetch_local` et vos classes de données
  astuces pour que le luminaire canonique passe par la même API que les consommateurs de roues utilisent. Ô teste
  reconstituer les métadonnées d'un fournisseur à partir de `providers.json`, injecter un instantané de télémétrie et vérifier
  octets de charge utile, reçus de blocs, rapports du fournisseur et contenu du tableau de bord similaire aux suites Rust/JS/Swift.
- **Pré-requis :** Exécutez `maturin develop --release` (ou installez la roue) pour que `_crypto` expose l'installation
  liaison `sorafs_multi_fetch_local` avant de chamar pytest ; o exploiter est automatiquement ignoré quando o liaison
  nao estiver disponivel.
- **Performance guard:** Mesmo orcamento <= 2 s par suite Rust; pytest enregistre le virus des octets
  montés et le curriculum vitae de participation des fournisseurs pour l'art de la publication.

Le release gate doit capturer la sortie récapitulative de chaque harnais (Rust, Python, JS, Swift) pour que
rapport arquivado permettant de comparer les reçus de charge utile et les mesures de forme uniformes avant le promoteur
euh construire. Exécuter `ci/sdk_sorafs_orchestrator.sh` pour démarrer toutes les suites de paridade (Rust, Python
liaisons, JS, Swift) dans une seule passe ; artefatos de CI devem anexar o trecho de log desse helper
mais le `matrix.md` est disponible (tableau SDK/statut/durée) avec le ticket de sortie pour que les réviseurs puissent le faire
auditer la matrice de parité sans réexécuter une suite localement.