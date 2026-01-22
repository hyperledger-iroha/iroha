---
lang: fr
direction: ltr
source: integrated_test_framework.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff9e1108802fdd57703749069f87270c4195f4037a32aa65c28cde9a67b63e98
source_last_modified: "2025-11-02T04:40:28.026560+00:00"
translation_last_reviewed: 2026-01-21
---

# Cadre de tests d'integration pour Hyperledger Iroha (reseau de 7 noeuds)

## Introduction

Hyperledger Iroha v2 fournit un ensemble riche d'Instructions Speciales Iroha (ISI) pour la gestion des domaines, des actifs, des permissions et des pairs. Ce document specifie un cadre de tests d'integration pour un reseau de 7 pairs afin de valider la correction, le consensus et la coherence inter-pairs dans des conditions normales et avec des pannes (en tolererant jusqu'a 2 pairs defectueux).

Ces directives refletent la recente migration du schema de genesis vers des blocs multi-transactions.

Note sur l'API : dans cette base de code, Torii est une API HTTP/WebSocket (Axum). Les tests doivent utiliser les endpoints HTTP (couramment les ports 8080+). Aucun service RPC supplementaire n'ecoute sur 50051.

## Objectifs

- Orchestration automatisee a 7 pairs : demarrer et gerer les pairs par programme pour un CI rapide (principal), ou via Docker Compose (optionnel).
- Configuration de genesis : demarrer a partir d'un genesis Norito commun avec des comptes/cles deterministes et les permissions requises.
- Exercer les ISI : couvrir systematiquement les ISI via des transactions et verifier l'etat resultant.
- Coherence inter-pairs : interroger plusieurs pairs apres chaque etape afin d'assurer un etat de registre identique.
- Verifications de tolerance aux pannes : avec jusqu'a 2 pairs hors ligne ou partitionnes, les pairs restants continuent ; les pairs revenus se resynchronisent sans divergence.

## Modes d'orchestration

- Harness Rust (recommande) : `crates/iroha_test_network` lance les processus `irohad` localement, alloue des ports, ecrit des configs par execution et un genesis Norito `.nrt`, surveille la disponibilite et les hauteurs de bloc, et fournit des utilitaires d'arret/restart.
- Docker Compose (optionnel) : `crates/iroha_swarm` genere un fichier Compose pour N pairs. A utiliser lorsqu'une orchestration externe ou un reseau conteneurise est souhaite.

## Mise en place du reseau de tests

**Bloc de genesis :**

- Source : `defaults/genesis.json` qui regroupe maintenant les instructions dans un tableau `transactions`. Les tests peuvent ajouter des instructions via `NetworkBuilder::with_genesis_instruction` et demarrer une nouvelle transaction via `.next_genesis_transaction()`. Le bloc resultant est serialize en Norito `.nrt`.
- Topologie : stockee dans la premiere transaction (`transactions[0].topology`) et inclut les 7 pairs (cle publique + adresse), afin que chaque pair connaisse le reseau des le depart.
- Comptes/permissions : preferer les comptes standardises de `crates/iroha_test_samples` (`ALICE_ID`, `BOB_ID`, `SAMPLE_GENESIS_ACCOUNT_KEYPAIR`) avec des grants explicites pour les scenarios de test, par ex. `CanManagePeers`, `CanManageRoles`, `CanMintAssetWithDefinition`.
- Strategie d'injection : avec le harness, le genesis est generalement fourni a un pair (le "genesis submitter") ; les autres pairs se synchronisent via le block sync. Avec Compose, pointer tous les pairs vers le meme chemin de genesis.

**Reseau et ports :**

- API HTTP Torii : `API_ADDRESS` (Axum). Pour Compose, mapper `8080-8086` pour 7 pairs vers l'hote. Le harness alloue automatiquement des ports loopback.
- P2P : l'adresse peer-to-peer interne est configuree dans `trusted_peers` et diffusee via gossip. Le harness configure `trusted_peers` automatiquement a chaque execution.

**Stockage des donnees :**

- Kura : stockage des blocs Iroha v2 (pas besoin de conteneur RocksDB/Postgres). Configurer via `[kura]` (par ex. `store_dir`). Desactiver les snapshots pour les tests via `[snapshot]`.

**Flux du harness :**

1) Construire le reseau : `NetworkBuilder::new().with_peers(7)` ; optionnellement `.with_pipeline_time(...)` et `.with_config_layer(...)` pour les overrides ; choisir le carburant IVM via `IvmFuelConfig`.
2) Demarrer les pairs : `.start()` ou `.start_blocking()` ; ecrit les couches de configuration, definit `trusted_peers`, injecte le genesis pour un pair, et attend la disponibilite.
3) Disponibilite : `Network::ensure_blocks(height)` ou `once_blocks_sync(...)` assure que les hauteurs de bloc non vides atteignent les attentes sur tous les pairs. Alternativement, interroger `Client::get_status()`.

**Flux Docker Compose (optionnel) :**

1) Generer compose : utiliser `iroha_swarm::Swarm` pour emettre un fichier compose avec N pairs. Mapper les ports API vers l'hote et definir les variables d'environnement (CHAIN, cles, TRUSTED_PEERS, GENESIS).
2) Demarrer : `docker compose up`.
3) Disponibilite : sonder les endpoints HTTP Torii jusqu'a un statut sain et une hauteur de bloc >= 1.

## Implementation du harness de tests (Rust)

**Client & transactions :**

- Utiliser `iroha::client::Client` (HTTP/WebSocket) pour soumettre des transactions et requetes a Torii.
- Construire des transactions a partir de sequences ISI `InstructionBox` ou de bytecode IVM ; signer avec des valeurs deterministes `KeyPair` provenant de `iroha_test_samples`.
- Appels utiles : `submit_blocking`, `submit_all_blocking`, `query(...).execute()/execute_all()`, `get_status()`, et flux de blocs et d'evenements via WebSocket.

**Coherence inter-pairs :**

- Apres chaque operation, interroger chaque pair en cours d'execution (`Network::peers()` -> `peer.client()`) et comparer les resultats (par ex. soldes, definitions, liste des pairs). Cela assure des checks de coherence au-dela d'une verification mono-pair.

**Injection de pannes (sans conteneurs) :**

- Utiliser les utilitaires relay/proxy dans `integration_tests/tests/extra_functional/unstable_network.rs` pour re-ecrire `trusted_peers` en proxies TCP et suspendre selectivement des liens. Cela permet des partitions, des pertes et des reconnexions ciblees.

**Prerequis IVM :**

- Certains tests necessitent des echantillons IVM pre-construits ; s'assurer que `crates/ivm/target/prebuilt/build_config.toml` existe. Quand absent, les tests peuvent etre ignores (les tests d'integration actuels font ce check).

## Plan de scenario a 7 noeuds

1) Demarrer un reseau de 7 pairs (harness ou Compose) et attendre le commit de genesis sur tous les pairs.
2) Executer une suite d'ISI :
   - Enregistrer domaines/comptes/actifs ; accorder/revoquer des permissions ; frapper/bruler/transferer des actifs ; definir/supprimer des cle-valeur ; enregistrer des triggers ; mettre a niveau l'executor.
3) Apres chaque etape logique, effectuer des requetes inter-pairs pour verifier l'etat identique.
4) Arreter 1-2 pairs, continuer a soumettre des transactions avec les 5 pairs restants ; assurer le progres et la coherence entre les pairs en cours d'execution.
5) Redemarrer les pairs arretes et verifier le rattrapage et l'egalite inter-pairs.

## Usage CI

- Build : `cargo build --workspace`
- Pre-construire les echantillons IVM (selon les besoins des tests)
- Test : `cargo test --workspace`
- Lint plus strict (optionnel) : `cargo clippy --workspace --all-targets -- -D warnings`

## Conclusion

Ce cadre s'appuie sur le harness Rust du repo (`iroha_test_network`) et le client HTTP (`iroha::client::Client`) pour valider un reseau Iroha v2 a 7 pairs. Il met l'accent sur la coherence inter-pairs, des scenarios de panne realistes, et une mise en place/teardown reproductible adaptee au CI. Docker Compose via `iroha_swarm` est disponible lorsque la conteneurisation est preferee.

## Explorateur

- Tout explorateur blockchain qui parle Torii HTTP/WebSocket peut se connecter independamment a chaque pair. Chaque pair expose un endpoint Torii (hote:port) adapte aux statuts, blocs, requetes et evenements.
- Avec le harness Rust : apres la construction d'un reseau vous pouvez deriver les URLs pour tous les pairs :

  ```rust
  use iroha_test_network::NetworkBuilder;

  let network = NetworkBuilder::new().with_peers(7).build();
  let urls = network.torii_urls();
  // ex. ["http://127.0.0.1:8080", ..., "http://127.0.0.1:8086"]
  ```

  Des helpers par pair sont aussi disponibles : `peer.api_address()` et `peer.torii_url()`.

- Avec Docker Compose (`iroha_swarm`) : generer un compose 7 pairs et mapper `8080..8086` vers l'hote ; pointer votre explorateur vers chacune de ces adresses. Si votre explorateur supporte plusieurs endpoints, configurez les 7 ; sinon, lancez une instance par pair.
