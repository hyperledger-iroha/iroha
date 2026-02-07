---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de refactorisation de lien
titre : Plan de refatoracao do ledger Sora Nexus
description : Espelho de `docs/source/nexus_refactor_plan.md`, détaille le travail de nettoyage par étapes pour la base de code de Iroha 3.
---

:::note Fonte canonica
Cette page reflète `docs/source/nexus_refactor_plan.md`. Mantenha as duas copias alinhadas ate que a edicao multilingue chegue ao portal.
:::

# Plan de refatorage du grand livre Sora Nexus

Ce document capture la feuille de route immédiate pour le refactoring du grand livre Sora Nexus ("Iroha 3"). Il reflète la mise en page actuelle du référentiel et les régressions observées dans la comptabilité Genesis/WSV, selon le consensus Sumeragi, les déclencheurs de contrats intelligents, les consultations d'instantanés, les liaisons de pointeur hôte-ABI et les codecs Norito. L'objectif et la convergence pour une architecture cohérente et un test en tentant d'entrer tout en corrigeant un patch monolithique unique.## 0. Principes directeurs
- Préserver le comportement déterminé dans le matériel hétérogène ; utiliser l'accélération via les indicateurs de fonctionnalité opt-in avec les solutions de secours identiques.
- Norito et une caméra de série. Tout changement d'état/schéma doit inclure des tests d'encodage/décodage aller-retour Norito et des mises à jour de luminaires.
- Une configuration fluide par `iroha_config` (utilisateur -> réel -> valeurs par défaut). Remover bascule ad hoc de l'environnement des chemins de production.
- Une politique ABI permanente V1 et inégociable. Les hôtes doivent refuser les types de pointeurs/appels système détectés de forme déterministe.
- `cargo test --workspace` et les tests dorés (`ivm`, `norito`, `integration_tests`) continuent comme base de porte pour chaque cadre.## 1. Instantané de la topologie du référentiel
- `crates/iroha_core` : utilisateurs Sumeragi, WSV, chargeur de genèse, pipelines (requête, superposition, voies zk), colle pour héberger les contrats intelligents.
- `crates/iroha_data_model` : schéma autorisé pour les données et les requêtes en chaîne.
- `crates/iroha` : API client utilisée par CLI, tests, SDK.
- `crates/iroha_cli` : CLI des opérateurs, il y a actuellement de nombreuses API dans `iroha`.
- `crates/ivm` : VM de bytecode Kotodama, points d'entrée du pointeur d'intégration-ABI de l'hôte.
- `crates/norito` : codec de sérialisation avec adaptateurs JSON et backends AoS/NCB.
- `integration_tests` : assertions cross-composant cobrindo genesis/bootstrap, Sumeragi, déclencheurs, pagination, etc.
- Les documents décrivent les métadonnées du grand livre Sora Nexus (`nexus.md`, `new_pipeline.md`, `ivm.md`), mais la mise en œuvre est fragmentée et partiellement supprimée dans la relation avec le code.

## 2. Pilares de refatoracao et marcos### Phase A - Fondations et observabilité
1. **Télémétrie WSV + instantanés**
   - Créer une API canonique d'instantanés dans `state` (trait `WorldStateSnapshot`) utilisée pour les requêtes, Sumeragi et CLI.
   - Utiliser `scripts/iroha_state_dump.sh` pour produire des instantanés déterministes via `iroha state dump --format norito`.
2. **Déterminisme de Genesis/Bootstrap**
   - Refatorar a ingestao de genesis para fluir por un seul pipeline com Norito (`iroha_core::genesis`).
   - Ajout d'une couverture d'intégration/régression qui retraite la genèse plus du premier bloc et confirme les racines WSV identiques entre arm64/x86_64 (accompagné de `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Tests de fixité cross-crate**
   - Extension `integration_tests/tests/genesis_json.rs` pour valider les invariants de WSV, pipeline et ABI dans un harnais unique.
   - Introduisez un échafaudage `cargo xtask check-shape` qui panique dans la dérive du schéma (accompagné de l'absence de retard dans l'outillage DevEx ; voir l'élément d'action dans `scripts/xtask/README.md`).### Fase B - WSV et surface des requêtes
1. **Transacoes de stockage d'État**
   - Colapsar `state/storage_transactions.rs` dans un adaptateur transacional qui impose l'ordre de commettre et la détection de conflits.
   - Les tests unitaires ont récemment vérifié que les modifications des actifs/monde/déclencheurs fazem rollback em falhas.
2. **Refator fait le modèle de requêtes**
   - Déplacer la logique de page/curseur pour les composants réutilisés dans `crates/iroha_core/src/query/`. Alinhar représente Norito et `iroha_data_model`.
   - Ajout de requêtes d'instantanés pour les déclencheurs, les actifs et les rôles avec une ordonnance déterminée (accompagnée via `crates/iroha_core/tests/snapshot_iterable.rs` pour une couverture actuelle).
3. **Consistance des instantanés**
   - Garantir que la CLI `iroha ledger query` utilise le même chemin de snapshot que Sumeragi/fetchers.
   - Tests de régression de l'instantané dans la CLI avec `tests/cli/state_snapshot.rs` (fonctionnalités pour les exécutions lentes).### Phase C - Pipeline Sumeragi
1. **Topologie et gestion d'époque**
   - Extraire `EpochRosterProvider` pour un trait d'implémentation basé sur les instantanés de la mise WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` propose un constructeur simple et un ami pour les simulations de bancs/tests.
2. **Simplification du flux de consensus**
   - Réorganiser `crates/iroha_core/src/sumeragi/*` en modules : `pacemaker`, `aggregation`, `availability`, `witness` avec les types partagés sur `consensus`.
   - Remplacer le message passant ad hoc par les enveloppes Norito et introduire des tests de propriété de changement de vue (accompagnés d'aucun retard de messagerie Sumeragi).
3. **Integração voie/preuve**
   - Alinhar Lane prouve les engagements de DA et garantit que le portail de RBC est uniforme.
   - Le test d'intégration de bout en bout `integration_tests/tests/extra_functional/seven_peer_consistency.rs` vient de vérifier le chemin avec RBC habilité.### Phase D - Contrats intelligents et hôtes pointeur-ABI
1. **Auditoria da Fronteira fait l'hôte**
   - Consolider les vérifications de type pointeur (`ivm::pointer_abi`) et les adaptateurs d'hôte (`iroha_core::smartcontracts::ivm::host`).
   - Comme les attentes de la table de pointeurs et des liaisons du manifeste hôte sont liées à `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs`, qui exercent les mappages TLV Golden.
2. **Sandbox pour l'exécution des déclencheurs**
   - Refatorar triggers para rodar via um `TriggerExecutor` comum qui impoe gas, validacao de pointers and journaling of eventos.
   - Tests de régression supplémentaires pour les déclencheurs des chemins cobrindo d'appel/heure de fausse (accompagnés via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alignement de CLI et client**
   - Garantir que les opérateurs de CLI (`audit`, `gov`, `sumeragi`, `ivm`) utilisent les fonctions partagées par le client `iroha` pour éviter toute dérive.
   - Tests de snapshots JSON en CLI vivem em `tests/cli/json_snapshot.rs` ; mantenha-os actualizados para que a dita dos commandos continue alinhada a référence JSON canonica.### Phase E - Durcissement du codec Norito
1. **Registre des schémas**
   - Créer un registre de schéma Norito à `crates/norito/src/schema/` pour fornecer les encodages canoniques pour les types de base.
   - Adicionados doc tests vérifiant la codification des charges utiles de l'exemple (`norito::schema::SamplePayload`).
2. **Rafraîchir les luminaires dorés**
   - Actualiser les luminaires dorés sur `crates/norito/tests/*` pour coïncider avec le nouveau schéma WSV lorsque vous souhaitez refactoriser.
   - `scripts/norito_regen.sh` régénérera le Golden JSON Norito de forme déterministe via l'assistant `norito_regen_goldens`.
3. **Intégration IVM/Norito**
   - Valider la sérialisation des manifestes Kotodama de bout en bout via Norito, garantissant qu'un pointeur de métadonnées ABI soit cohérent.
   - `crates/ivm/tests/manifest_roundtrip.rs` garde le système Norito encode/décode pour les manifestes.## 3. Préoccupations transversales
- **Estrategia de testes** : Chaque phase favorise les tests unitaires -> tests de caisse -> tests d'intégration. Tests selon lesquels Falham capturam régresse actuellement ; de nouveaux tests empêchent de revenir.
- **Documentation** : Apos cada phase, actualizar `status.md` et levar itens em aberto para `roadmap.md` enquanto Remove tarefas concluidas.
- **Benchmarks de performance** : Manter évalue les performances existantes dans `iroha_core`, `ivm` et `norito` ; ajouter des médecins à la base pos-refactor pour valider que nao ha régresse.
- **Drapeaux de fonctionnalité** : Manter active le niveau de caisse pour les backends qui exigent des chaînes d'outils externes (`cuda`, `zk-verify-batch`). Chemins SIMD du CPU toujours construits et sélectionnés lors de l'exécution ; fornecer fallbacks escalades déterministes pour le matériel non pris en charge.## 4. Arrivée immédiate
- Échafaudage de la phase A (trait instantané + câblage de télémétrie) - voir les tarefas acionaveis nas actualizacoes do roadmap.
- Une salle de comptes récente pour `sumeragi`, `state` et `ivm` révèle les séquences suivantes :
  - `sumeragi` : tolérances de protection contre les codes morts ou de diffusion de preuves de changement de vue, état de relecture VRF et exportation de télémétrie EMA. Eles permanecem gated ate que a simplificacao do fluxo de consenso da Fase C e os entregaveis de integracao lane/proof sejam entregues.
  - `state` : une nettoyage de `Cell` et le rotation de télémétrie entre dans le trilha de télémétrie WSV de la phase A, tandis que les notes de SoA/parallel-apply entrent dans aucun retard d'optimisation du pipeline de la phase C.
  - `ivm` : exposition de la bascule CUDA, validation des enveloppes et couverture Halo2/Metal mapeiam pour le travail de la limite de l'hôte de la phase D mais le thème transversal de l'accélération GPU ; les noyaux ne permanecem aucun retard dédié au GPU mangé estarem prontos.
- Préparer un résumé inter-équipes RFC de ce plan pour la signature avant d'appliquer les modifications envahissantes du code.## 5. Questoes em aberto
- RBC doit-il permanecer facultativement alem de P1, ou l'obligation pour les voies du grand livre Nexus ? Exiger une décision des parties prenantes.
- Impomos grupos de composabilidade DS em P1 ou os mantemos desativados ate que as lane proofs amadurecam ?
- Quel est le canon local pour les paramètres ML-DSA-87 ? Candidat : ​​nouvelle caisse `crates/fastpq_isi` (criacao pendente).

---

_Ultime mise à jour : 2025-09-12_