---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/nexus/nexus-refactor-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8376f4dd0a7626ff3c374994370071bab4c4073a97171d7152b1a977e760a64a
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-refactor-plan
title: Plan de refactorisation du ledger Sora Nexus
description: Miroir de `docs/source/nexus_refactor_plan.md`, detaillant le travail de nettoyage par phases pour la base de code Iroha 3.
---

:::note Source canonique
Cette page reflete `docs/source/nexus_refactor_plan.md`. Gardez les deux copies alignees jusqu'a ce que l'edition multilingue arrive dans le portal.
:::

# Plan de refactorisation du ledger Sora Nexus

Ce document capture le roadmap immediat du refactor du Sora Nexus Ledger ("Iroha 3"). Il reflete la topologie actuelle du depot et les regressions observees dans la comptabilite genesis/WSV, le consensus Sumeragi, les triggers de smart contracts, les requetes de snapshots, les bindings de host pointer-ABI et les codecs Norito. L'objectif est de converger vers une architecture coherente et testable sans tenter de livrer toutes les corrections dans un patch monolithique.

## 0. Principes directeurs
- Preserver un comportement deterministe sur du materiel heterogene; utiliser l'acceleration uniquement via des feature flags opt-in avec des fallbacks identiques.
- Norito est la couche de serialisation. Tout changement d'etat/schema doit inclure des tests de round-trip Norito encode/decode et des mises a jour de fixtures.
- La configuration transite par `iroha_config` (user -> actual -> defaults). Supprimer les toggles d'environnement ad-hoc des paths de production.
- La politique ABI reste V1 et non negociable. Les hosts doivent rejeter deterministiquement les pointer types/syscalls inconnus.
- `cargo test --workspace` et les golden tests (`ivm`, `norito`, `integration_tests`) restent le gate de base pour chaque jalon.

## 1. Snapshot de la topologie du depot
- `crates/iroha_core`: acteurs Sumeragi, WSV, loader genesis, pipelines (query, overlay, zk lanes), glue host des smart contracts.
- `crates/iroha_data_model`: schema autoritatif pour les donnees et requetes on-chain.
- `crates/iroha`: API client utilisee par CLI, tests, SDK.
- `crates/iroha_cli`: CLI operateur, reflete actuellement de nombreuses APIs dans `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, points d'entree d'integration pointer-ABI du host.
- `crates/norito`: codec de serialisation avec adaptateurs JSON et backends AoS/NCB.
- `integration_tests`: assertions cross-component couvrant genesis/bootstrap, Sumeragi, triggers, pagination, etc.
- Les docs decrivent deja les objectifs du Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mais l'implementation est fragmentee et partiellement obsolete par rapport au code.

## 2. Piliers de refactor et jalons

### Phase A - Fondations et observabilite
1. **Telemetrie WSV + Snapshots**
   - Etablir une API canonique de snapshots dans `state` (trait `WorldStateSnapshot`) utilisee par queries, Sumeragi et CLI.
   - Utiliser `scripts/iroha_state_dump.sh` pour produire des snapshots deterministes via `iroha state dump --format norito`.
2. **Determinisme Genesis/Bootstrap**
   - Refactoriser l'ingestion genesis pour passer par un pipeline unique base sur Norito (`iroha_core::genesis`).
   - Ajouter une couverture d'integration/regression qui rejoue genesis plus le premier bloc et verifie des racines WSV identiques entre arm64/x86_64 (suivi dans `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Tests de fixity cross-crate**
   - Etendre `integration_tests/tests/genesis_json.rs` pour valider les invariants WSV, pipeline et ABI dans un seul harness.
   - Introduire un scaffold `cargo xtask check-shape` qui panic sur schema drift (suivi dans le backlog DevEx tooling; voir l'action item dans `scripts/xtask/README.md`).

### Phase B - WSV et surface de requetes
1. **Transactions de state storage**
   - Collapser `state/storage_transactions.rs` en un adaptateur transactionnel qui applique l'ordre de commit et la detection de conflits.
   - Des unit tests verifient desormais que les modifications asset/world/triggers font rollback en cas d'echec.
2. **Refactor du modele de requetes**
   - Deplacer la logique de pagination/cursor dans des composants reutilisables sous `crates/iroha_core/src/query/`. Aligner les representations Norito dans `iroha_data_model`.
   - Ajouter des snapshot queries pour triggers, assets et roles avec un ordre deterministe (suivi via `crates/iroha_core/tests/snapshot_iterable.rs` pour la couverture actuelle).
3. **Consistance des snapshots**
   - S'assurer que le CLI `iroha ledger query` utilise le meme chemin de snapshot que Sumeragi/fetchers.
   - Les tests de regression de snapshots CLI se trouvent sous `tests/cli/state_snapshot.rs` (feature-gated pour les runs lents).

### Phase C - Pipeline Sumeragi
1. **Topologie et gestion des epoques**
   - Extraire `EpochRosterProvider` en trait avec des implementations basees sur des snapshots de stake WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` fournit un constructeur simple, friendly pour mocks dans benches/tests.
2. **Simplification du flux de consensus**
   - Reorganiser `crates/iroha_core/src/sumeragi/*` en modules: `pacemaker`, `aggregation`, `availability`, `witness` avec des types partages sous `consensus`.
   - Remplacer le message passing ad-hoc par des enveloppes Norito typees et introduire des property tests de view-change (suivi dans le backlog messaging Sumeragi).
3. **Integration lane/proof**
   - Aligner les lane proofs avec les engagements DA et garantir une gating RBC uniforme.
   - Le test d'integration end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs` verifie maintenant le chemin avec RBC active.

### Phase D - Smart contracts et hosts pointer-ABI
1. **Audit de la frontiere host**
   - Consolider les checks de pointer-type (`ivm::pointer_abi`) et les adaptateurs de host (`iroha_core::smartcontracts::ivm::host`).
   - Les attentes de pointer table et les bindings de host manifest sont couverts par `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs`, qui exercent les mappings TLV golden.
2. **Sandbox d'execution des triggers**
   - Refactoriser les triggers pour passer par un `TriggerExecutor` commun qui applique gas, validation de pointers et journaling d'evenements.
   - Ajouter des tests de regression pour les triggers call/time couvrant les chemins d'echec (suivi via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alignement CLI & client**
   - S'assurer que les operations CLI (`audit`, `gov`, `sumeragi`, `ivm`) reposent sur les fonctions client partagees `iroha` pour eviter le drift.
   - Les tests de snapshots JSON du CLI vivent dans `tests/cli/json_snapshot.rs`; gardez-les a jour pour que la sortie core continue de correspondre a la reference JSON canonique.

### Phase E - Durcissement du codec Norito
1. **Registry de schemas**
   - Creer un registry de schemas Norito sous `crates/norito/src/schema/` pour sourcer les encodages canoniques des types core.
   - Ajout de doc tests verifiant l'encodage de payloads d'exemple (`norito::schema::SamplePayload`).
2. **Refresh des golden fixtures**
   - Mettre a jour les golden fixtures `crates/norito/tests/*` pour correspondre au nouveau schema WSV une fois le refactor livre.
   - `scripts/norito_regen.sh` regenere les golden JSON Norito de facon deterministe via le helper `norito_regen_goldens`.
3. **Integration IVM/Norito**
   - Valider la serialisation des manifests Kotodama end-to-end via Norito, en garantissant une metadata pointer ABI coherente.
   - `crates/ivm/tests/manifest_roundtrip.rs` maintient la parite Norito encode/decode pour les manifests.

## 3. Preoccupations transversales
- **Strategie de tests**: Chaque phase promeut unit tests -> crate tests -> integration tests. Les tests en echec capturent les regressions actuelles; les nouveaux tests evitent leur retour.
- **Documentation**: Apres chaque phase, mettre a jour `status.md` et reporter les items ouverts dans `roadmap.md` tout en supprimant les taches terminees.
- **Benchmarks de performance**: Maintenir les benches existants dans `iroha_core`, `ivm` et `norito`; ajouter des mesures de base post-refactor pour valider l'absence de regressions.
- **Feature flags**: Conserver des toggles au niveau crate uniquement pour les backends qui requierent des toolchains externes (`cuda`, `zk-verify-batch`). Les chemins SIMD CPU sont toujours construits et selectionnes a l'execution; fournir des fallbacks scalaires deterministes pour le materiel non supporte.

## 4. Actions immediates
- Scaffolding de la Phase A (snapshot trait + wiring telemetrie) - voir les taches actionnables dans les mises a jour du roadmap.
- L'audit recent des defauts pour `sumeragi`, `state` et `ivm` a revele les points suivants:
  - `sumeragi`: des allowances de dead-code protegent le broadcast des preuves de view-change, l'etat de replay VRF et l'export telemetrie EMA. Ils restent gates jusqu'a ce que la simplification du flux de consensus de la Phase C et les livrables d'integration lane/proof soient livrees.
  - `state`: le nettoyage de `Cell` et le routage telemetrie passent sur la piste telemetrie WSV de la Phase A, tandis que les notes SoA/parallel-apply basculent dans le backlog d'optimisation de pipeline de la Phase C.
  - `ivm`: l'exposition du toggle CUDA, la validation des enveloppes et la couverture Halo2/Metal se mappent au travail host-boundary de la Phase D plus le theme transversal d'acceleration GPU; les kernels restent dans le backlog GPU dedie jusqu'a maturite.
- Preparer un RFC cross-team resumant ce plan pour sign-off avant de livrer des changements de code invasifs.

## 5. Questions ouvertes
- RBC doit-il rester optionnel apres P1, ou est-il obligatoire pour les lanes du ledger Nexus? Decision des parties prenantes requise.
- Doit-on imposer des groupes de composabilite DS en P1 ou les laisser desactives jusqu'a maturite des lane proofs?
- Quelle est la localisation canonique des parametres ML-DSA-87? Candidat: nouveau crate `crates/fastpq_isi` (creation en attente).

---

_Derniere mise a jour: 2025-09-12_
