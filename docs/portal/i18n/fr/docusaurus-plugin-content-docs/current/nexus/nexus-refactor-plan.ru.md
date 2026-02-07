---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de refactorisation de lien
titre : Plan de refonte Sora Nexus Ledger
description : Le bouton `docs/source/nexus_refactor_plan.md`, описывающее поэтапную зачистку кодовой базы Iroha 3.
---

:::note Канонический источник
Cette page correspond à `docs/source/nexus_refactor_plan.md`. Il est possible de copier des copies synchronisées si de nombreuses versions ne sont pas disponibles sur le portail.
:::

# Plan de refonte Sora Nexus Ledger

Ce document fournit une feuille de route pour la refactorisation de Sora Nexus Ledger ("Iroha 3"). En dehors de la structure du dépôt et de la régression, qui se trouvent dans votre Genesis/WSV, le consensus Sumeragi déclenche connecteurs intelligents, requêtes d'instantanés, liaisons d'hôte pointeur-ABI et codecs Norito. Il s'agit d'un projet d'architecture architecturale qui ne peut pas utiliser notre propre patch.## 0. Principes naturels
- Сохранять детерминированное поведение на гетерогенном железе ; utilisez uniquement les indicateurs de fonctionnalité d'inscription pour identifier les solutions de secours.
- Norito - selon la série. Les paramètres d'état/schéma doivent inclure les tests d'encodage/décodage aller-retour Norito et les appareils de mise à jour.
- La configuration se fait à partir de `iroha_config` (utilisateur -> réel -> valeurs par défaut). Utilisez les bascules d'environnement ad hoc pour le projet.
- La politique ABI est disponible en V1 et n'est pas disponible. Les hôtes peuvent déterminer les types de pointeurs/appels système les plus récents.
- `cargo test --workspace` et Golden Tests (`ivm`, `norito`, `integration_tests`) sont disponibles pour chaque étape.## 1. Répertoire d'images topographiques
- `crates/iroha_core` : acteurs Sumeragi, WSV, chargeur Genesis, pipelines (requête, superposition, voies zk), colle pour hôte de contrat intelligent.
- `crates/iroha_data_model` : données de schéma automatiques et requêtes en chaîne.
- `crates/iroha` : API client, utilisation CLI, tests, SDK.
- `crates/iroha_cli` : l'opérateur CLI, qui utilise de nombreuses API depuis `iroha`.
- `crates/ivm` : VM bytecode Kotodama, points d'entrée pour l'intégration pointeur-hôte ABI.
- `crates/norito` : codec de sérialisation avec adaptateurs JSON et backends AoS/NCB.
- `integration_tests` : assertions inter-composants, genèse/bootstrap, Sumeragi, déclencheurs, pagination et etc.
- Docs présente le registre Sora Nexus (`nexus.md`, `new_pipeline.md`, `ivm.md`), avec fragmentation et précision. устарела относительно кода.

## 2. Les éléments de refactorisation et les éléments possibles### Phase A - Fondements et observabilité
1. **Télémétrie WSV + instantanés**
   - Téléchargez l'API d'instantané canonique dans `state` (trait `WorldStateSnapshot`), en utilisant des requêtes, Sumeragi et CLI.
   - Utilisez `scripts/iroha_state_dump.sh` pour détecter les instantanés à partir de `iroha state dump --format norito`.
2. **Genèse/Déterminisme Bootstrap**
   - Commencez par ingest genesis, en attendant le pipeline Norito (`iroha_core::genesis`).
   - Ajoutez la fenêtre d'intégration/régression qui vous permet de créer Genesis ainsi que le bloc précédent et de modifier l'identification des racines WSV par arm64/x86_64 (en utilisant Arm64/x86_64). `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Tests de fixité entre caisses**
   - Corrigez `integration_tests/tests/genesis_json.rs` pour valider WSV, pipeline et invariants ABI dans votre harnais.
   - L'échafaudage `cargo xtask check-shape` entraîne une panique en cas de dérive de schéma (en retard dans le backlog des outils DevEx ; comme élément d'action dans `scripts/xtask/README.md`).### Phase B – WSV et surface de requête
1. **Transactions de stockage d'État**
   - Le modèle `state/storage_transactions.rs` de l'adaptateur de transition permet d'effectuer la validation et de détecter les conflits.
   - Les tests unitaires prouvent que les modifications des actifs/du monde/des déclencheurs sont effectuées lors de l'installation.
2. **Refactor du modèle de requête**
   - Logique de pagination/curseur dans les composants d'exploitation du module `crates/iroha_core/src/query/`. Enregistrez les représentations Norito dans `iroha_data_model`.
   - Créez des requêtes d'instantanés pour les déclencheurs, les actifs et les rôles avec les paramètres de sécurité (en utilisant `crates/iroha_core/tests/snapshot_iterable.rs` pour votre recherche).
3. **Cohérence des instantanés**
   - En outre, la CLI `iroha ledger query` utilise le chemin de l'instantané, à savoir Sumeragi/fetchers.
   - Tests de régression d'instantanés CLI effectués dans `tests/cli/state_snapshot.rs` (fonctionnalités pour les programmes moyens).### Phase C - Gazoduc Sumeragi
1. **Topologie et gestion des époques**
   - Utilisez `EpochRosterProvider` dans la fonction de réalisation, qui concerne les instantanés de mise WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` propose un constructeur de projet, utilisé pour les simulations sur bancs/tests.
2. **Simplification du flux de consensus**
   - Réorganiser `crates/iroha_core/src/sumeragi/*` dans les modules : `pacemaker`, `aggregation`, `availability`, `witness` selon les types de module `consensus`.
   - Permet de transmettre des messages ad hoc dans les enveloppes Norito types et d'effectuer des tests de propriétés de changement de vue (en attente du retard de messagerie Sumeragi).
3. **Intégration de voie/preuve**
   - Enregistrez les preuves de voie avec les engagements DA et suivez le contrôle RBC.
   - Le test d'intégration de bout en bout `integration_tests/tests/extra_functional/seven_peer_consistency.rs` doit être vérifié auprès de RBC.### Phase D – Contrats intelligents et hôtes Pointer-ABI
1. **Audit des limites de l'hôte**
   - Consolidez les broches de type pointeur (`ivm::pointer_abi`) et les adaptateurs hôte (`iroha_core::smartcontracts::ivm::host`).
   - La table de pointeurs et les liaisons de manifeste d'hôte contiennent les tests `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs`, qui vérifient les mappages TLV dorés.
2. **Bac à sable d'exécution de déclenchement**
   - Préparer les déclencheurs, que vous pouvez utiliser avec l'`TriggerExecutor`, pour activer le gaz, la validation du pointeur et la journalisation des événements.
   - Créez des tests de régression pour les déclencheurs d'appel/d'heure pour détecter les chemins d'échec (voir `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alignement CLI et client**
   - Examinez les opérations CLI (`audit`, `gov`, `sumeragi`, `ivm`) utilisées pour les fonctions client. `iroha`, pour évaluer la dérive.
   - Les tests d'instantanés CLI JSON sont disponibles dans `tests/cli/json_snapshot.rs` ; Pensez à ce que vous avez fait avec la commande principale associée à la référence canonique JSON.### Phase E - Durcissement du codec Norito
1. **Registre de schémas**
   - Ajoutez le registre de schéma Norito à la suite de `crates/norito/src/schema/` pour définir les encodages canoniques pour les types de données de base.
   - Réalisation de tests de documentation, vérification d'échantillons de charges utiles d'encodage (`norito::schema::SamplePayload`).
2. **Actualisation des luminaires dorés**
   - Enregistrez les luminaires dorés dans `crates/norito/tests/*`, en utilisant le nouveau schéma WSV après la refonte.
   - `scripts/norito_regen.sh` détermine la régénération Norito JSON Goldens avec l'assistant `norito_regen_goldens`.
3. **Intégration IVM/Norito**
   - La vérification de la sérialisation du manifeste Kotodama de bout en bout par Norito garantit la cohérence des métadonnées ABI du pointeur.
   - `crates/ivm/tests/manifest_roundtrip.rs` utilise Norito pour encoder/décoder les portions des manifestes.## 3. Les femmes sexy
- **Stratégie de test** : Cette étape produit des tests unitaires -> tests de caisse -> tests d'intégration. Les tests de correction de la régression technique ; Les nouveaux tests ne sont pas publiés.
- **Documentation** : Après avoir effectué des opérations sur `status.md`, vous devez effectuer des tâches non sécurisées dans `roadmap.md`. завершенные задачи.
- **Repères de performances** : comparez les bancs de référence dans `iroha_core`, `ivm` et `norito` ; il est nécessaire d'effectuer une révision de base après la refonte, afin de modifier la régression actuelle.
- **Feature Flags** : activez les bascules au niveau de la caisse également pour les backends, en utilisant toutes les chaînes d'outils (`cuda`, `zk-verify-batch`). Le processeur SIMD peut être utilisé et utilisé lors de votre prochaine utilisation ; Il est possible de déterminer les solutions de secours scalaires pour les personnes non désirées.## 4. Belle fête
- Échafaudage de phase A (trait instantané + câblage de télémétrie) - см. tâches réalisables dans la feuille de route обновлениях.
- Les défauts d'audit pertinents pour `sumeragi`, `state` et `ivm` ont été détectés à des moments précis :
  - `sumeragi` : tolérances de code mort pour la preuve de changement de vue en diffusion, l'état de relecture VRF et l'exportation de télémétrie EMA. Dans ce cas, il n'est pas possible de livrer les livrables de la phase C pour améliorer le flux de consensus et l'intégration des voies/preuves.
  - `state` : Le nettoyage et le routage de télémétrie `Cell` précèdent la piste de télémétrie WSV de la phase A, ainsi que l'application SoA/parallèle dans le carnet d'optimisation du pipeline de la phase C.
  - `ivm` : activation de la bascule CUDA, validation de l'enveloppe et cartographie de la couverture Halo2/Metal sur le serveur de la limite de l'hôte de la phase D ainsi que sur l'accélération du GPU ; les noyaux sont en retard dans l'arriéré GPU par rapport aux performances.
- Créez un RFC inter-équipes pour obtenir ce plan de signature avant d'entrer le code d'identification.

## 5. Открытые вопросы
- Est-ce que RBC a établi un service opérationnel après P1, ou est-il responsable des voies du grand livre Nexus ? Требуется решение стейкхолдеров.
- Avez-vous des groupes de composabilité DS dans P1 ou faites-vous des preuves de voie ?
- Qu'est-ce que le canal canonique pour les paramètres ML-DSA-87 ? Candidat : nouvelle caisse `crates/fastpq_isi` (создание ожидается).

---

_Mise à jour : 2025-09-12_