---
lang: fr
direction: ltr
source: README.md
status: complete
translator: manual
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:00:00+00:00"
translation_last_reviewed: 2026-02-07
---

# Hyperledger Iroha

[![Licence](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha est une plateforme blockchain déterministe pour des déploiements permissionnés et de consortium. Elle fournit la gestion des comptes et des actifs, des permissions on-chain et des smart contracts via l’Iroha Virtual Machine (IVM).

> L’état du workspace et les changements récents sont suivis dans [`status.md`](./status.md).

## Lignes de release

Ce dépôt publie deux lignes de déploiement à partir de la même base de code :

- **Iroha 2** : réseaux permissionnés/de consortium auto-hébergés.
- **Iroha 3 (SORA Nexus)** : ligne orientée Nexus reposant sur les mêmes crates cœur.

Les deux lignes partagent les mêmes composants principaux, notamment la sérialisation Norito, le consensus Sumeragi et la chaîne d’outils Kotodama -> IVM.

## Structure du dépôt

- [`crates/`](./crates) : crates Rust principales (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `ivm`, `norito`, etc.).
- [`integration_tests/`](./integration_tests) : tests d’intégration et de réseau inter-composants.
- [`IrohaSwift/`](./IrohaSwift) : package SDK Swift.
- [`java/iroha_android/`](./java/iroha_android) : package SDK Android.
- [`docs/`](./docs) : documentation utilisateur, opérateur et développeur.

## Démarrage rapide

### Prérequis

- [Rust stable](https://www.rust-lang.org/tools/install)
- Optionnel : Docker + Docker Compose pour des exécutions multi-peers en local

### Build et tests (workspace)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Notes :

- Un build workspace complet peut prendre environ 20 minutes.
- Les tests workspace complets peuvent prendre plusieurs heures.
- Le workspace cible `std` (les builds WASM/no-std ne sont pas pris en charge).

### Commandes de test ciblées

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### Commandes de test SDK

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## Lancer un réseau local

Démarrez le réseau Docker Compose fourni :

```bash
docker compose -f defaults/docker-compose.yml up
```

Utilisez la CLI avec la configuration client par défaut :

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Pour les étapes de déploiement natif du daemon, voir [`crates/irohad/README.md`](./crates/irohad/README.md).

## API et observabilité

Torii expose des API Norito et JSON. Endpoints opérateur courants :

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Référence complète des endpoints :

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Crates principales

- [`crates/iroha`](./crates/iroha) : bibliothèque cliente.
- [`crates/irohad`](./crates/irohad) : binaires daemon peer.
- [`crates/iroha_cli`](./crates/iroha_cli) : CLI de référence.
- [`crates/iroha_core`](./crates/iroha_core) : moteur d’exécution et cœur du ledger.
- [`crates/iroha_config`](./crates/iroha_config) : modèle de configuration typé.
- [`crates/iroha_data_model`](./crates/iroha_data_model) : modèle de données canonique.
- [`crates/iroha_crypto`](./crates/iroha_crypto) : primitives cryptographiques.
- [`crates/norito`](./crates/norito) : codec de sérialisation déterministe.
- [`crates/ivm`](./crates/ivm) : Iroha Virtual Machine.
- [`crates/iroha_kagami`](./crates/iroha_kagami) : outillage clés/genesis/config.

## Carte de la documentation

- Index principal : [`docs/README.md`](./docs/README.md)
- Genesis : [`docs/genesis.md`](./docs/genesis.md)
- Consensus (Sumeragi) : [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Pipeline de transactions : [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- Internals P2P : [`docs/source/p2p.md`](./docs/source/p2p.md)
- Syscalls IVM : [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Grammaire Kotodama : [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Format wire Norito : [`norito.md`](./norito.md)
- Suivi de l’avancement : [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Traductions

Vue d’ensemble en japonais : [`README.ja.md`](./README.ja.md)

Autres vues d’ensemble :
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Workflow de traduction : [`docs/i18n/README.md`](./docs/i18n/README.md)

## Contribution et aide

- Guide de contribution : [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Canaux communauté/support : [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Licence

Iroha est distribué sous licence Apache-2.0. Voir [`LICENSE`](./LICENSE).

La documentation est distribuée sous licence CC-BY-4.0 : http://creativecommons.org/licenses/by/4.0/
