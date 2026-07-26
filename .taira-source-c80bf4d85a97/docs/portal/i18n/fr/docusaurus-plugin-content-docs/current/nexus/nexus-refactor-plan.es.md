---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de refactorisation de lien
titre : Plan de refactorisation du grand livre Sora Nexus
description : en particulier de `docs/source/nexus_refactor_plan.md`, qui détaille le travail de nettoyage par phases pour la base de code de Iroha 3.
---

:::note Fuente canonica
Cette page reflète `docs/source/nexus_refactor_plan.md`. Manten ambas copias alineadas hasta que la edicion multilingue lgue al portal.
:::

# Plan de refactorisation du grand livre Sora Nexus

Ce document capture immédiatement la feuille de route pour le refactor du Sora Nexus Ledger ("Iroha 3"). Réfléchissez à la disposition actuelle du référentiel et aux modifications observées dans la comptabilité de Genesis/WSV, au consensus Sumeragi, aux déclencheurs de contrats intelligents, aux consultations d'instantanés, aux liaisons de pointeur hôte-ABI et aux codecs Norito. L’objectif converge vers une architecture cohérente et plausible sans avoir l’intention d’atterrir toutes les corrections dans une seule pièce monolithique.## 0. Principes directeurs
- Préserver le comportement déterminé en matière de matériel hétérogène ; utiliser l'accélération uniquement avec les drapeaux de fonctionnalité opt-in et les solutions de secours identiques.
- Norito est la capacité de sérialisation. Tout changement d'état/squema doit inclure des essais d'encodage/décodage aller-retour Norito et des mises à jour des appareils.
- La configuration s'effectue par `iroha_config` (utilisateur -> actuel -> valeurs par défaut). Éliminer les bascules ad hoc de l'arrivée sur les chemins de production.
- La politique ABI est en V1 et n'est pas négociable. Les hôtes doivent rechazar types de pointeurs/appels système découverts de forme déterministe.
- `cargo test --workspace` et les tests d'or (`ivm`, `norito`, `integration_tests`) si vous avez la base de l'ordinateur pour chaque hito.## 1. Instantané de la topologie du référentiel
- `crates/iroha_core` : acteurs Sumeragi, WSV, chargeur de genèse, pipelines (requête, superposition, voies zk), colle de l'hôte des contrats intelligents.
- `crates/iroha_data_model` : esquema autoritativo para datos y queries on-chain.
- `crates/iroha` : API client utilisée par CLI, tests, SDK.
- `crates/iroha_cli` : CLI des opérateurs, qui reflètent actuellement de nombreuses API et `iroha`.
- `crates/ivm` : VM du bytecode Kotodama, points d'entrée du pointeur d'intégration-ABI de l'hôte.
- `crates/norito` : codec de sérialisation avec adaptateurs JSON et backends AoS/NCB.
- `integration_tests` : assertions multi-composants qui impliquent la genèse/bootstrap, Sumeragi, les déclencheurs, la pagination, etc.
- Les documents contiennent les métadonnées du grand livre Sora Nexus (`nexus.md`, `new_pipeline.md`, `ivm.md`), mais la mise en œuvre est fragmentée et partiellement obsolète par rapport au code.

## 2. Pilares de refactor et hitos### Phase A - Fondations et observabilité
1. **Télémétrie WSV + instantanés**
   - Créer une API canonique d'instantanés en `state` (trait `WorldStateSnapshot`) utilisée pour les requêtes, Sumeragi et CLI.
   - Utiliser `scripts/iroha_state_dump.sh` pour produire des instantanés déterministes via `iroha state dump --format norito`.
2. **Déterminisme de Genesis/Bootstrap**
   - Refactoriser l'acquisition de Genesis pour qu'elle passe par un pipeline unique avec Norito (`iroha_core::genesis`).
   - Ajouter une couverture d'intégration/régression qui reproduit la genèse plus le premier blocage et confirme les racines WSV identiques entre arm64/x86_64 (suivi en `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Tests de fixité cross-crate**
   - Extension `integration_tests/tests/genesis_json.rs` pour valider les invariants de WSV, pipeline et ABI dans un harnais solo.
   - Introduire un échafaudage `cargo xtask check-shape` qui panique avant la dérive du schéma (en suivant le backlog de l'outillage DevEx ; voir l'élément d'action sur `scripts/xtask/README.md`).### Fase B - WSV et surface des requêtes
1. **Transacciones de stockage d'état**
   - Colapsar `state/storage_transactions.rs` dans un adaptateur transactionnel qui imponga l'ordre des commissions et la détection des conflits.
   - Les tests unitaires vérifient maintenant que les modifications des actifs/monde/déclencheurs doivent être annulées avant l'échec.
2. **Refactor du modèle de requête**
   - Déplacer la logique de pagination/curseur vers les composants réutilisables sous `crates/iroha_core/src/query/`. Représentations linéaires Norito et `iroha_data_model`.
   - Agréger les requêtes d'instantanés pour les déclencheurs, les actifs et les rôles avec un ordre déterminé (en suivant via `crates/iroha_core/tests/snapshot_iterable.rs` pour la couverture actuelle).
3. **Consistance des instantanés**
   - Assurez-vous que la CLI `iroha ledger query` utilise la même route de snapshot que Sumeragi/fetchers.
   - Les tests de régression des instantanés en CLI sont effectués sur `tests/cli/state_snapshot.rs` (fonctionnalités pour les exécutions lentes).### Phase C - Pipeline Sumeragi
1. **Topologie et gestion d'époque**
   - Extraer `EpochRosterProvider` pour un trait de mise en œuvre résolu pour les instantanés de mise en WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` propose un constructeur simple et convivial pour les mocks et les bancs/tests.
2. **Simplification du flux de consensus**
   - Réorganiser `crates/iroha_core/src/sumeragi/*` en modules : `pacemaker`, `aggregation`, `availability`, `witness` avec les types partagés sous `consensus`.
   - Remplacer le message passant ad hoc par les enveloppes Norito et introduire les tests de propriété de changement de vue (en suivant le backlog de message Sumeragi).
3. **Voie d'intégration/preuve**
   - Preuves de voies linéaires avec engagements de DA et assurer que RBC gating sea uniforme.
   - Le test d'intégration de bout en bout `integration_tests/tests/extra_functional/seven_peer_consistency.rs` doit maintenant vérifier l'itinéraire avec RBC habilité.### Phase D - Contrats intelligents et hôtes pointeur-ABI
1. **Auditoria de limite del host**
   - Consolider les contrôles de type pointeur (`ivm::pointer_abi`) et les adaptateurs d'hôte (`iroha_core::smartcontracts::ivm::host`).
   - Les attentes de la table de pointeur et des liaisons du manifeste hôte sont contenues dans `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs`, qui exécutent les mappages TLV dorés.
2. **Sandbox d'éjection de déclencheurs**
   - Refactoriser les déclencheurs pour les exécuter via un commun `TriggerExecutor` qui impose le gaz, la validation des pointeurs et la journalisation des événements.
   - Réaliser des tests de régression pour les déclencheurs d'appel/heure en coupant les chemins de chute (suivi via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alignement de CLI et client**
   - Assurez-vous que les opérations CLI (`audit`, `gov`, `sumeragi`, `ivm`) dépendent des fonctions compartimentées du client `iroha` pour éviter toute dérive.
   - Les tests de snapshots JSON de la CLI sont présents sur `tests/cli/json_snapshot.rs` ; veillez à ce que la sortie des commandes coïncide avec la référence JSON canonique.### Fase E - Endurecimiento del codec Norito
1. **Registre des schémas**
   - Créez un registre de schéma Norito sous `crates/norito/src/schema/` pour réduire les encodages canoniques de types de base.
   - Se agregaron doc tests qui vérifient la codification des charges utiles de muestra (`norito::schema::SamplePayload`).
2. **Actualisation des luminaires dorés**
   - Actualiser les éléments dorés de `crates/norito/tests/*` pour qu'ils coïncident avec le nouveau schéma WSV lors de l'arrivée du refactor.
   - `scripts/norito_regen.sh` régénérera le Golden JSON de Norito de forme déterminée via l'assistant `norito_regen_goldens`.
3. **Intégration IVM/Norito**
   - Valider la sérialisation des manifestes Kotodama de bout en bout via Norito, en garantissant que le pointeur de métadonnées ABI est cohérent.
   - `crates/ivm/tests/manifest_roundtrip.rs` maintient la parité Norito encode/décode pour les manifestes.## 3. Thèmes transversaux
- **Stratégie de tests** : Chaque étape de la promotion des tests unitaires -> tests de caisse -> tests d'intégration. Los tests fallidos capturan regresiones actuales ; los nouveaux tests évitent de pouvoir les réparer.
- **Documentation** : Lors de chaque étape, actualisez `status.md` et déplacez les éléments ouverts à `roadmap.md` pendant que vous pouvez effectuer des tâches terminées.
- **Benchmarks de rendu** : Bancs Mantener existants en `iroha_core`, `ivm` et `norito` ; ajouter des médicaments de base après le refactor pour valider qu'il n'y a pas de régressions.
- **Drapeaux de fonctionnalité** : Mantener bascule un niveau de caisse uniquement pour les backends qui nécessitent des chaînes d'outils externes (`cuda`, `zk-verify-batch`). Les chemins SIMD du CPU sont toujours construits et sélectionnés lors de l'exécution ; les solutions de secours du fournisseur augmentent les déterminations pour le matériel non pris en charge.## 4. Actions immédiates
- Échafaudage de la phase A (trait instantané + câblage de télémétrie) - voir les zones exploitables dans l'actualisation de la feuille de route.
- Les auditeurs récents de défauts pour `sumeragi`, `state` et `ivm` révèlent les points suivants :
  - `sumeragi` : allocations de protection contre les codes morts lors de la diffusion des essais de changement de vue, état de relecture VRF et exportation de télémétrie EMA. Ceci est permanent et fermé jusqu'à ce que la simplification du flux de consensus de la Phase C et les entregables d'intégration de voie/preuve aterricen.
  - `state` : la nettoyage de `Cell` et la route de télémétrie passent par la piste de télémétrie WSV de la Phase A, tandis que les notes de SoA/parallel-apply sont intégrées au backlog d'optimisation du pipeline de la Phase C.
  - `ivm` : l'exposition des bascules CUDA, la validation des enveloppes et la couverture Halo2/Metal sont mappées sur le travail de limite d'hôte de la Phase D plus le thème transversal d'accélération GPU ; Les noyaux sont permanents dans le backlog dédié au GPU jusqu'à cette liste.
- Préparer une équipe croisée RFC qui résume ce plan pour la signature avant de modifier les changements de code envahissant.## 5. Questions ouvertes
- Debe RBC seguir siendo facultative mas alla de P1, o est-ce obligatoire pour les voies du grand livre Nexus ? Nécessite une décision des parties prenantes.
- Impulsamos grupos de composabilidad DS en P1 o los mantenemos deshabilitados hasta que maduren las lane proofs ?
- Quelle est l'emplacement canonique pour les paramètres ML-DSA-87 ? Candidat : ​​nouvelle caisse `crates/fastpq_isi` (création pendante).

---

_Mise à jour ultime : 2025-09-12_