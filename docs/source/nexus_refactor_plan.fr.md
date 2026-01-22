---
lang: fr
direction: ltr
source: docs/source/nexus_refactor_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44b7100fddd377c97dfcab678ce425ec35edfa4a1276f9b6a22aa2c64135a94d
source_last_modified: "2025-11-02T04:40:40.017979+00:00"
translation_last_reviewed: 2026-01-01
---

# Plan de refonte du ledger Sora Nexus

Ce document capture la roadmap immediate pour la refonte du Sora Nexus Ledger ("Iroha 3"). Il
reflete la disposition actuelle du depot et les regressions observees dans la comptabilite
genesis/WSV, le consensus Sumeragi, les triggers de smart-contract, les requetes snapshot, les
bindings host pointer-ABI et les codecs Norito. L'objectif est de converger vers une architecture
coherente et testable sans tenter de livrer toutes les corrections dans un patch monolithique.

## 0. Principes directeurs
- Preserver un comportement deterministe sur du hardware heterogene; utiliser l'acceleration
  uniquement via des feature flags opt-in avec des fallbacks identiques.
- Norito est la couche de serialization. Tout changement d'etat/schema doit inclure des tests
  round-trip encode/decode Norito et des mises a jour de fixtures.
- La configuration passe par `iroha_config` (user -> actual -> defaults). Supprimer les toggles
  d'environnement ad-hoc des chemins de production.
- La politique ABI reste en V1 et non negociable. Les hosts doivent rejeter les types de
  pointeur/syscalls inconnus de facon deterministe.
- `cargo test --workspace` et les tests golden (`ivm`, `norito`, `integration_tests`) restent le
  gate de base pour chaque jalon.

## 1. Snapshot de topologie du depot
- `crates/iroha_core`: acteurs Sumeragi, WSV, chargeur genesis, pipelines (query, overlay, zk lanes),
  glue host pour smart-contracts.
- `crates/iroha_data_model`: schema autoritatif pour les donnees on-chain et les requetes.
- `crates/iroha`: API client utilisee par CLI, tests, SDK.
- `crates/iroha_cli`: CLI operateur, reflete actuellement de nombreuses APIs dans `iroha`.
- `crates/ivm`: VM bytecode Kotodama, points d'entree d'integration host pointer-ABI.
- `crates/norito`: codec de serialization avec adaptateurs JSON et backends AoS/NCB.
- `integration_tests`: assertions cross-component couvrant genesis/bootstrap, Sumeragi, triggers,
  pagination, etc.
- Les docs decrivent deja les objectifs Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`),
  mais l'implementation est fragmentee et partiellement obsolete par rapport au code.

## 2. Piliers et jalons de refonte

### Phase A - Fondations et observabilite
1. **Telemetrie WSV + snapshots**
   - Etablir une API de snapshot canonique dans `state` (trait `WorldStateSnapshot`) utilisee par
     les requetes, Sumeragi et la CLI.
   - Utiliser `scripts/iroha_state_dump.sh` pour produire des snapshots deterministes via
     `iroha state dump --format norito`.
2. **Determinisme genesis/bootstrap**
   - Refactoriser l'ingestion genesis pour passer par un pipeline unique base sur Norito
     (`iroha_core::genesis`).
   - Ajouter une couverture d'integration/regression qui rejoue genesis plus le premier bloc et
     affirme des racines WSV identiques sur arm64/x86_64 (suivi dans
     `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Tests de fixite cross-crate**
   - Etendre `integration_tests/tests/genesis_json.rs` pour valider les invariants WSV, pipeline et
     ABI dans un seul harness.
   - Introduire un scaffold `cargo xtask check-shape` qui panic en cas de schema drift (suivi dans
     le backlog DevEx tooling; voir l'action item de `scripts/xtask/README.md`).

### Phase B - WSV et surface de requetes
1. **Transactions de state storage**
   - Fusionner `state/storage_transactions.rs` dans un adaptateur transactionnel qui impose l'ordre
     de commit et la detection de conflits.
   - Les tests unitaires verifient maintenant que les modifications assets/world/triggers rollback
     en cas d'echec.
2. **Refonte du modele de requetes**
   - Deplacer la logique pagination/cursor dans des composants reutilisables sous
     `crates/iroha_core/src/query/`. Aligner les representations Norito dans `iroha_data_model`.
   - Ajouter des requetes snapshot pour triggers, assets et roles avec un ordre deterministe (suivi
     via `crates/iroha_core/tests/snapshot_iterable.rs` pour la couverture actuelle).
3. **Consistance des snapshots**
   - S'assurer que la CLI `iroha ledger query` utilise le meme chemin de snapshot que Sumeragi/fetchers.
   - Les tests de regression snapshot CLI vivent sous `tests/cli/state_snapshot.rs` (feature-gated
     pour les runs lents).

### Phase C - Pipeline Sumeragi
1. **Topologie et gestion des epoques**
   - Extraire `EpochRosterProvider` en trait avec des implementations basees sur des snapshots de
     stake WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` fournit un constructeur simple et mock-friendly pour
     benches/tests.
2. **Simplification du flux de consensus**
   - Reorganiser `crates/iroha_core/src/sumeragi/*` en modules: `pacemaker`, `aggregation`,
     `availability`, `witness` avec des types partages sous `consensus`.
   - Remplacer le message passing ad-hoc par des envelopes Norito typees et introduire des property
     tests de view-change (suivi dans le backlog de messagerie Sumeragi).
3. **Integration lanes/proofs**
   - Aligner les preuves de lane avec les engagements DA et assurer un gating RBC uniforme.
   - Le test d'integration end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs`
     verifie maintenant le chemin avec RBC active.

### Phase D - Smart contracts et hosts pointer-ABI
1. **Audit de frontiere host**
   - Consolider les checks de types de pointeur (`ivm::pointer_abi`) et les adaptateurs host
     (`iroha_core::smartcontracts::ivm::host`).
   - Les attentes de table de pointeurs et les bindings de host manifest sont couverts par
     `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs`, qui exercent
     les mappings TLV golden.
2. **Sandbox d'execution des triggers**
   - Refactoriser les triggers pour passer par un `TriggerExecutor` commun qui impose gas,
     validation de pointeurs et journaling d'evenements.
   - Ajouter des tests de regression pour les triggers call/time couvrant les chemins d'echec
     (suivi via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alignement CLI et client**
   - S'assurer que les operations CLI (`audit`, `gov`, `sumeragi`, `ivm`) reposent sur les fonctions
     client partagees `iroha` pour eviter le drift.
   - Les tests de snapshot JSON CLI vivent dans `tests/cli/json_snapshot.rs`; les maintenir a jour
     pour que la sortie des commandes core reste alignee sur la reference JSON canonique.

### Phase E - Durcissement du codec Norito
1. **Schema registry**
   - Creer un registry de schema Norito sous `crates/norito/src/schema/` pour sourcer les encodages
     canoniques des types core.
   - Ajouter des doc tests qui verifient l'encodage de payloads exemple (`norito::schema::SamplePayload`).
2. **Refresh des golden fixtures**
   - Mettre a jour `crates/norito/tests/*` golden fixtures pour correspondre au nouveau schema WSV
     une fois la refonte livree.
   - `scripts/norito_regen.sh` regenere les goldens Norito JSON de facon deterministe via le helper
     `norito_regen_goldens`.
3. **Integration IVM/Norito**
   - Valider la serialization des manifests Kotodama end-to-end via Norito, en s'assurant que la
     metadata pointer ABI est coherente.
   - `crates/ivm/tests/manifest_roundtrip.rs` maintient la parite encode/decode Norito pour les
     manifests.

## 3. Enjeux transverses
- **Strategie de tests**: Chaque phase promeut tests unitaires -> tests de crate -> tests
  d'integration. Les tests en echec capturent les regressions actuelles; les nouveaux tests
  evitent leur retour.
- **Documentation**: Apres chaque phase, mettre a jour `status.md` et faire remonter les items
  ouverts vers `roadmap.md` tout en supprimant les taches terminees.
- **Benchmarks de performance**: Conserver les benches existants dans `iroha_core`, `ivm`, `norito`;
  ajouter des mesures de base post-refonte pour valider l'absence de regression.
- **Feature flags**: Garder des toggles au niveau crate uniquement pour les backends qui
  necessitent des toolchains externes (`cuda`, `zk-verify-batch`). Les chemins SIMD CPU sont
  toujours construits et selectionnes au runtime; fournir des fallbacks scalaires deterministes
  pour le hardware non supporte.

## 4. Actions immediates
- Scaffolding de la Phase A (snapshot trait + wiring telemetrie) - voir les taches actionnables
  dans les updates de roadmap.
- L'audit recent des defauts pour `sumeragi`, `state` et `ivm` a mis en evidence:
  - `sumeragi`: des allowances dead-code protegent la diffusion des proofs view-change, l'etat
    de replay VRF et l'export telemetrie EMA. Ils restent gate jusqu'a ce que la simplification du
    flux de consensus et l'integration lanes/proofs de la Phase C soient livrees.
  - `state`: le nettoyage `Cell` et le routage telemetrie passent dans la piste telemetrie WSV
    de la Phase A, tandis que les notes SoA/parallel-apply vont dans le backlog d'optimisation
    pipeline de la Phase C.
  - `ivm`: l'exposition du toggle CUDA, la validation des envelopes et la couverture Halo2/Metal
    s'alignent sur le travail de frontiere host de la Phase D plus le theme transversal
    d'acceleration GPU; les kernels restent dans le backlog GPU dedie jusqu'a readiness.
- Preparer un RFC cross-team resumant ce plan pour sign-off avant de livrer des changements
  de code invasifs.

## 5. Questions ouvertes
- RBC doit-il rester optionnel apres P1, ou est-il obligatoire pour les lanes du ledger Nexus?
  Decision requise.
- Imposer les groupes de composabilite DS en P1 ou les laisser desactives jusqu'a maturite des
  proofs de lane?
- Quelle est la localisation canonique des parametres ML-DSA-87? Candidat: nouveau crate
  `crates/fastpq_isi` (creation en attente).

---

_Derniere mise a jour: 2025-09-12_
