---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de refactorisation de lien
titre : Plan de refactorisation du grand livre Sora Nexus
description : Miroir de `docs/source/nexus_refactor_plan.md`, détaillant le travail de nettoyage par phases pour la base de code Iroha 3.
---

:::note Source canonique
Cette page reflète `docs/source/nexus_refactor_plan.md`. Gardez les deux exemplaires alignés jusqu'à ce que l'édition multilingue arrive dans le portail.
:::

# Plan de refactorisation du grand livre Sora Nexus

Ce document capture la feuille de route immédiate du refactor du Sora Nexus Ledger ("Iroha 3"). Il reflète la topologie actuelle du dépôt et les régressions observées dans la compatibilité Genesis/WSV, le consensus Sumeragi, les déclencheurs de contrats intelligents, les requêtes de snapshots, les liaisons de pointeur hôte-ABI et les codecs Norito. L'objectif est de converger vers une architecture cohérente et testable sans tenter de livrer toutes les corrections dans un patch monolithique.## 0. Principes directeurs
- Préserver un comportement déterministe sur du matériel hétérogène ; utilisez l'accélération uniquement via des feature flags opt-in avec des fallbacks identiques.
- Norito est la couche de sérialisation. Tout changement d'état/schéma doit inclure les tests de aller-retour Norito encode/decode et des mises à jour de luminaires.
- La configuration transite par `iroha_config` (utilisateur -> actuel -> valeurs par défaut). Supprimer les bascules d'environnement ad hoc des chemins de production.
- La politique ABI reste V1 et non négociable. Les hôtes doivent rejeter deterministiquement les pointeurs types/syscalls inconnus.
- `cargo test --workspace` et les golden tests (`ivm`, `norito`, `integration_tests`) restent le gate de base pour chaque jalon.## 1. Snapshot de la topologie du dépôt
- `crates/iroha_core` : acteurs Sumeragi, WSV, Loader Genesis, pipelines (query, overlay, zk lanes), Glue Host des smart contracts.
- `crates/iroha_data_model` : schéma autoritaire pour les données et demandes en chaîne.
- `crates/iroha` : Client API utilisé par CLI, tests, SDK.
- `crates/iroha_cli` : opérateur CLI, reflète actuellement de nombreuses API dans `iroha`.
- `crates/ivm` : VM de bytecode Kotodama, points d'entrée d'intégration pointeur-ABI du host.
- `crates/norito` : codec de sérialisation avec adaptateurs JSON et backends AoS/NCB.
- `integration_tests` : assertions cross-composant incluant genesis/bootstrap, Sumeragi, triggers, pagination, etc.
- Les documents décrivent déjà les objectifs du Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mais l'implémentation est fragmentée et partiellement obsolète par rapport au code.

## 2. Piliers de refactor et jalons### Phase A - Fondations et observabilité
1. **Télémétrie WSV + Instantanés**
   - Etablir une API canonique de snapshots dans `state` (trait `WorldStateSnapshot`) utilisée par requêtes, Sumeragi et CLI.
   - Utiliser `scripts/iroha_state_dump.sh` pour produire des instantanés déterministes via `iroha state dump --format norito`.
2. **Déterminisme Genesis/Bootstrap**
   - Refactoriser l'ingestion genesis pour passer par un pipeline unique base sur Norito (`iroha_core::genesis`).
   - Ajouter une couverture d'intégration/régression qui rejoue genesis plus le premier bloc et vérifie les racines WSV identiques entre arm64/x86_64 (suivi dans `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Tests de fixité cross-crate**
   - Etendre `integration_tests/tests/genesis_json.rs` pour valider les invariants WSV, pipeline et ABI dans un seul harnais.
   - Introduire un scaffold `cargo xtask check-shape` qui panique sur schema drift (suivi dans le backlog DevEx tooling ; voir l'action item dans `scripts/xtask/README.md`).### Phase B - WSV et surface de requêtes
1. **Transactions de stockage d'état**
   - Collapser `state/storage_transactions.rs` en un adaptateur transactionnel qui applique l'ordre de commit et la détection de conflits.
   - Des tests unitaires vérifient désormais que les modifications Asset/World/Triggers Font Rollback en cas d'échec.
2. **Refactor du modèle de requêtes**
   - Déplacer la logique de pagination/curseur dans des composants reutilisables sous `crates/iroha_core/src/query/`. Aligner les représentations Norito dans `iroha_data_model`.
   - Ajouter des requêtes d'instantanés pour les déclencheurs, les actifs et les rôles avec un ordre déterministe (suivi via `crates/iroha_core/tests/snapshot_iterable.rs` pour la couverture actuelle).
3. **Consistance des instantanés**
   - S'assurer que la CLI `iroha ledger query` utilise le même chemin de snapshot que Sumeragi/fetchers.
   - Les tests de régression de snapshots CLI se trouvent sous `tests/cli/state_snapshot.rs` (feature-gated pour les exécutions prêtées).### Phase C - Pipeline Sumeragi
1. **Topologie et gestion des époques**
   - Extraire `EpochRosterProvider` en trait avec des implémentations basées sur des snapshots de mise WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` fournit un constructeur simple et convivial pour des mocks dans des bancs/tests.
2. **Simplification du flux de consensus**
   - Réorganisateur `crates/iroha_core/src/sumeragi/*` en modules : `pacemaker`, `aggregation`, `availability`, `witness` avec des types partages sous `consensus`.
   - Remplacer le message passant ad-hoc par des enveloppes Norito typees et introduire des tests de propriété de view-change (suivi dans le backlog messagerie Sumeragi).
3. **Voie d'intégration/preuve**
   - Aligner les épreuves de voie avec les engagements DA et garantir une gating RBC uniforme.
   - Le test d'intégration de bout en bout `integration_tests/tests/extra_functional/seven_peer_consistency.rs` vérifie maintenant le chemin avec RBC actif.### Phase D - Contrats intelligents et pointeur d'hôtes-ABI
1. **Audit de la frontière hôte**
   - Consolider les contrôles de type pointeur (`ivm::pointer_abi`) et les adaptateurs de hôte (`iroha_core::smartcontracts::ivm::host`).
   - Les attentes de pointer table et les liaisons de host manifest sont couvertes par `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs`, qui exercent les mappages TLV golden.
2. **Sandbox d'exécution des déclencheurs**
   - Refactoriser les triggers pour passer par un `TriggerExecutor` commun qui applique gaz, validation de pointeurs et journalisation d'événements.
   - Ajout des tests de régression pour les triggers call/time couvrant les chemins d'echec (suivi via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alignement CLI et client**
   - S'assurer que les opérations CLI (`audit`, `gov`, `sumeragi`, `ivm`) reposent sur les fonctions client partagees `iroha` pour éviter la dérive.
   - Les tests de snapshots JSON du CLI vivent dans `tests/cli/json_snapshot.rs`; gardez-les a jour pour que la sortie core continue de correspondre à la référence JSON canonique.### Phase E - Durcissement du codec Norito
1. **Registre de schémas**
   - Créer un registre de schémas Norito sous `crates/norito/src/schema/` pour sourcer les encodages canoniques des types core.
   - Ajout de doc tests vérifiant l'encodage de payloads d'exemple (`norito::schema::SamplePayload`).
2. **Rafraîchissement des luminaires dorés**
   - Mettre à jour les golden luminaires `crates/norito/tests/*` pour correspondre au nouveau schéma WSV une fois le livre refactor.
   - `scripts/norito_regen.sh` régénère les golden JSON Norito de manière déterministe via le helper `norito_regen_goldens`.
3. **Intégration IVM/Norito**
   - Valider la sérialisation des manifestes Kotodama de bout en bout via Norito, en garantissant un pointeur de métadonnées ABI cohérent.
   - `crates/ivm/tests/manifest_roundtrip.rs` maintient la parite Norito encode/decode pour les manifestes.## 3. Préoccupations transversales
- **Stratégie de tests** : Chaque phase promeut tests unitaires -> tests de crate -> tests d'intégration. Les tests en echec capturent les régressions actuelles; les nouveaux tests évitent leur retour.
- **Documentation** : Après chaque phase, mettre à jour `status.md` et reporter les éléments ouverts dans `roadmap.md` tout en supprimant les taches terminées.
- **Benchmarks de performance** : Maintenir les benchs existants dans `iroha_core`, `ivm` et `norito` ; ajouter des mesures de base post-refactor pour valider l'absence de régressions.
- **Feature flags** : Conserver des bascules au niveau crate uniquement pour les backends qui nécessitent des chaînes d'outils externes (`cuda`, `zk-verify-batch`). Les chemins SIMD CPU sont toujours construits et sélectionnés à l'exécution; fournir des solutions de repli scalaires déterministes pour le matériel non supporté.## 4. Actions immédiates
- Echafaudage de la Phase A (snapshot trait + câblage télémétrie) - voir les taches actionnables dans les mises à jour du roadmap.
- L'audit récent des défauts pour `sumeragi`, `state` et `ivm` a révélé les points suivants :
  - `sumeragi` : des allocations de code mort protégeant la diffusion des preuves de changement de vue, l'état de replay VRF et l'export de télémétrie EMA. Ils restent portes jusqu'à ce que la simplification du flux de consensus de la Phase C et les livrables d'intégration voie/preuve soient livrés.
  - `state` : le nettoyage de `Cell` et le routage télémétrique passent sur la piste télémétrique WSV de la Phase A, tandis que les notes SoA/parallel-apply basculent dans le backlog d'optimisation de pipeline de la Phase C.
  - `ivm` : l'exposition du toggle CUDA, la validation des enveloppes et la couverture Halo2/Metal se mappent au travail host-boundary de la Phase D plus le thème transversal d'accélération GPU ; les noyaux restent dans le backlog GPU dédié jusqu'à maturité.
- Préparer un RFC cross-team reprenant ce plan pour signature avant de livrer des changements de code invasifs.## 5. Questions ouvertes
- RBC doit-il rester optionnel après P1, ou est-il obligatoire pour les voies du grand livre Nexus ? Décision des parties requise.
- Doit-on imposer des groupes de composabilité DS en P1 ou les laisser desactives jusqu'à maturité des épreuves de voie ?
- Quelle est la localisation canonique des paramètres ML-DSA-87 ? Candidat : ​​nouvelle caisse `crates/fastpq_isi` (création en attente).

---

_Dernière mise à jour : 2025-09-12_