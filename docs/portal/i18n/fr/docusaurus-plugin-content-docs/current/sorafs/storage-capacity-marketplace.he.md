---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08459a61bed9c7e2b544490318d05f7bf7da814d776e31a7ca4529b5f690d18f
source_last_modified: "2025-11-14T04:43:22.362428+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/storage_capacity_marketplace.md`. Gardez les deux emplacements alignés tant que la documentation héritée reste active.
:::

# Marketplace de capacité de stockage SoraFS (Brouillon SF-2c)

L'item SF-2c de la roadmap introduit un marketplace gouverné où les providers de
stockage déclarent une capacité engagée, reçoivent des ordres de réplication et
perçoivent des fees proportionnels à la disponibilité livrée. Ce document cadre
les livrables requis pour la première release et les décline en pistes actionnables.

## Objectifs

- Exprimer les engagements de capacité (bytes totaux, limites par lane, expiration)
  sous une forme vérifiable consommable par la gouvernance, le transport SoraNet et Torii.
- Allouer les pins entre providers selon la capacité déclarée, le stake et les
  contraintes de politique tout en maintenant un comportement déterministe.
- Mesurer la livraison de stockage (succès de réplication, uptime, preuves d'intégrité)
  et exporter la télémétrie pour la distribution des fees.
- Fournir des processus de révocation et de dispute afin que les providers malhonnêtes
  soient pénalisés ou retirés.

## Concepts de domaine

| Concept | Description | Livrable initial |
|---------|-------------|-----------------|
| `CapacityDeclarationV1` | Payload Norito décrivant l'ID du provider, le support du profil chunker, les GiB engagés, les limites par lane, des hints de pricing, l'engagement de staking et l'expiration. | Schéma + validateur dans `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instruction émise par la gouvernance assignant un CID de manifest à un ou plusieurs providers, incluant le niveau de redondance et les métriques SLA. | Schéma Norito partagé avec Torii + API de smart contract. |
| `CapacityLedger` | Registry on-chain/off-chain qui suit les déclarations de capacité actives, les ordres de réplication, les métriques de performance et l'accumulation des fees. | Module de smart contract ou stub de service off-chain avec snapshot déterministe. |
| `MarketplacePolicy` | Politique de gouvernance définissant le stake minimum, les exigences d'audit et les courbes de pénalité. | Struct de config dans `sorafs_manifest` + document de gouvernance. |

### Schémas implémentés (statut)

## Découpage du travail

### 1. Couche schéma et registry

| Tâche | Owner(s) | Notes |
|------|----------|-------|
| Définir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Storage Team / Governance | Utiliser Norito ; inclure versionnement sémantique et références de capacités. |
| Implémenter les modules parser + validateur dans `sorafs_manifest`. | Storage Team | Imposer IDs monotones, limites de capacité, exigences de stake. |
| Étendre la metadata du chunker registry avec `min_capacity_gib` par profil. | Tooling WG | Aide les clients à imposer des exigences minimales de hardware par profil. |
| Rédiger le document `MarketplacePolicy` capturant les guardrails d'admission et le calendrier de pénalités. | Governance Council | Publier dans les docs avec les defaults de politique. |

#### Définitions de schéma (implémentées)

- `CapacityDeclarationV1` capture des engagements de capacité signés par provider, incluant handles chunker canoniques, références de capacité, caps optionnels par lane, hints de pricing, fenêtres de validité et metadata. La validation assure un stake non nul, des handles canoniques, des aliases dédupliqués, des caps par lane dans le total déclaré et une comptabilité GiB monotone.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` associe des manifests à des assignments émis par la gouvernance avec objectifs de redondance, seuils SLA et garanties par assignment ; les validateurs imposent des handles canoniques, des providers uniques et des contraintes de deadline avant ingestion par Torii ou le registry.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` exprime des snapshots par époque (GiB déclarés vs utilisés, compteurs de réplication, pourcentages d'uptime/PoR) qui alimentent la distribution des fees. Les bornes maintiennent l'utilisation dans les déclarations et les pourcentages dans 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Les helpers partagés (`CapacityMetadataEntry`, `PricingScheduleV1`, validateurs lane/assignment/SLA) fournissent une validation déterministe des clés et un reporting d'erreur réutilisable par CI et le tooling downstream.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` expose désormais le snapshot on-chain via `/v1/sorafs/capacity/state`, en combinant déclarations de providers et entrées du fee ledger derrière un Norito JSON déterministe.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La couverture de validation exerce l'application des handles canoniques, la détection de doublons, les bornes par lane, les guards d'assignation de réplication et les checks de plage de télémétrie pour que les régressions apparaissent immédiatement en CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Tooling opérateur : `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convertit des specs lisibles en payloads Norito canoniques, blobs base64 et résumés JSON afin que les opérateurs puissent préparer des fixtures `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` et des ordres de réplication avec validation locale.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Les fixtures de référence vivent dans `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) et sont générées via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Intégration du plan de contrôle

| Tâche | Owner(s) | Notes |
|------|----------|-------|
| Ajouter des handlers Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` avec payloads Norito JSON. | Torii Team | Répliquer la logique de validation ; réutiliser les helpers Norito JSON. |
| Propager les snapshots `CapacityDeclarationV1` vers la metadata du scoreboard d'orchestrator et les plans de fetch gateway. | Tooling WG / Orchestrator team | Étendre `provider_metadata` avec références de capacité pour que le scoring multi-source respecte les limites par lane. |
| Alimenter les ordres de réplication dans les clients orchestrator/gateway pour piloter les assignments et hints de failover. | Networking TL / Gateway team | Le builder de scoreboard consomme des ordres signés par la gouvernance. |
| Tooling CLI : étendre `sorafs_cli` avec `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | Fournir JSON déterministe + sorties scoreboard. |

### 3. Politique marketplace & gouvernance

| Tâche | Owner(s) | Notes |
|------|----------|-------|
| Ratifier `MarketplacePolicy` (stake minimum, multiplicateurs de pénalité, cadence d'audit). | Governance Council | Publier dans les docs, capturer l'historique des révisions. |
| Ajouter des hooks de gouvernance pour que le Parlement approuve, renouvelle et révoque les déclarations. | Governance Council / Smart Contract team | Utiliser les événements Norito + ingestion de manifests. |
| Implémenter le calendrier de pénalités (réduction de fees, slashing de bond) lié aux violations SLA télémétrées. | Governance Council / Treasury | S'aligner avec les outputs de settlement du `DealEngine`. |
| Documenter le processus de dispute et la matrice d'escalade. | Docs / Governance | Lier au runbook de dispute + helpers CLI. |

### 4. Metering & distribution des fees

| Tâche | Owner(s) | Notes |
|------|----------|-------|
| Étendre l'ingestion de metering Torii pour accepter `CapacityTelemetryV1`. | Torii Team | Valider GiB-hour, succès PoR, uptime. |
| Mettre à jour le pipeline de metering `sorafs_node` pour reporter utilisation par ordre + stats SLA. | Storage Team | Aligner avec les ordres de réplication et handles chunker. |
| Pipeline de settlement : convertir télémétrie + réplication en payouts dénommés en XOR, produire des résumés prêts pour gouvernance, et enregistrer l'état du ledger. | Treasury / Storage Team | Connecter à Deal Engine / Treasury exports. |
| Exporter dashboards/alertes pour la santé du metering (backlog d'ingestion, télémétrie stale). | Observability | Étendre le pack Grafana référencé par SF-6/SF-7. |

- Torii expose désormais `/v1/sorafs/capacity/telemetry` et `/v1/sorafs/capacity/state` (JSON + Norito) afin que les opérateurs soumettent des snapshots de télémétrie par époque et que les inspecteurs récupèrent le ledger canonique pour audit ou packaging de preuves.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- L'intégration `PinProviderRegistry` garantit que les ordres de réplication sont accessibles via le même endpoint ; les helpers CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) valident et publient désormais la télémétrie depuis des automatisations avec hashing déterministe et résolution d'alias.
- Les snapshots de metering produisent des entrées `CapacityTelemetrySnapshot` fixées au snapshot `metering`, et les exports Prometheus alimentent le board Grafana prêt à importer dans `docs/source/grafana_sorafs_metering.json` afin que les équipes de facturation suivent l'accumulation de GiB-hour, les fees nano-SORA projetés et la conformité SLA en temps réel.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Lorsque le smoothing de metering est activé, le snapshot inclut `smoothed_gib_hours` et `smoothed_por_success_bps` pour comparer les valeurs EMA aux compteurs bruts utilisés par la gouvernance pour les payouts.【crates/sorafs_node/src/metering.rs:401】

### 5. Gestion des disputes et révocations

| Tâche | Owner(s) | Notes |
|------|----------|-------|
| Définir le payload `CapacityDisputeV1` (plaignant, preuve, provider ciblé). | Governance Council | Schéma Norito + validateur. |
| Support CLI pour déposer des disputes et répondre (avec pièces de preuve). | Tooling WG | Assurer un hashing déterministe du bundle de preuves. |
| Ajouter des checks automatiques pour violations SLA répétées (auto-escalade en dispute). | Observability | Seuils d'alertes et hooks de gouvernance. |
| Documenter le playbook de révocation (période de grâce, évacuation des données pin). | Docs / Storage Team | Lier au doc de politique et au runbook opérateur. |

## Exigences de tests & CI

- Tests unitaires pour tous les nouveaux validateurs de schéma (`sorafs_manifest`).
- Tests d'intégration simulant : déclaration → ordre de réplication → metering → payout.
- Workflow CI pour régénérer des déclarations/telemetrie de capacité et assurer la synchronisation des signatures (étendre `ci/check_sorafs_fixtures.sh`).
- Tests de charge pour l'API registry (simuler 10k providers, 100k orders).

## Télémétrie & dashboards

- Panneaux de dashboard :
  - Capacité déclarée vs utilisée par provider.
  - Backlog des ordres de réplication et délai moyen d'assignation.
  - Conformité SLA (uptime %, taux de succès PoR).
  - Accumulation des fees et pénalités par époque.
- Alertes :
  - Provider sous la capacité minimale engagée.
  - Ordre de réplication bloqué > SLA.
  - Échecs du pipeline de metering.

## Livrables de documentation

- Guide opérateur pour déclarer la capacité, renouveler les engagements et monitorer l'utilisation.
- Guide de gouvernance pour approuver les déclarations, émettre des ordres et gérer les disputes.
- Référence API pour les endpoints de capacité et le format d'ordre de réplication.
- FAQ marketplace pour developers.

## Checklist de readiness GA

L'item de roadmap **SF-2c** conditionne le rollout production à des preuves concrètes
sur la comptabilité, la gestion des disputes et l'onboarding. Utilisez les artefacts
ci-dessous pour garder les critères d'acceptation alignés avec l'implémentation.

### Comptabilité nocturne et réconciliation XOR
- Exporter le snapshot d'état de capacité et l'export du ledger XOR pour la même fenêtre, puis lancer :
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Le helper sort avec un code non nul en cas de settlements/penalties manquants ou excessifs et émet un résumé Prometheus au format textfile.
- L'alerte `SoraFSCapacityReconciliationMismatch` (dans `dashboards/alerts/sorafs_capacity_rules.yml`) se déclenche lorsque les métriques de réconciliation signalent des écarts ; dashboards dans `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archiver le résumé JSON et les hashes sous `docs/examples/sorafs_capacity_marketplace_validation/` avec les paquets de gouvernance.

### Preuve de dispute & slashing
- Déposer des disputes via `sorafs_manifest_stub capacity dispute` (tests :
  `cargo test -p sorafs_car --test capacity_cli`) pour garder des payloads canoniques.
- Lancer `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et les suites de pénalité (`record_capacity_telemetry_penalises_persistent_under_delivery`) pour prouver que disputes et slashes rejouent de manière déterministe.
- Suivre `docs/source/sorafs/dispute_revocation_runbook.md` pour la capture de preuves et l'escalade ; lier les approbations de strike au rapport de validation.

### Onboarding des providers et smoke tests de sortie
- Regénérer les artefacts de déclaration/télémétrie avec `sorafs_manifest_stub capacity ...` et rejouer les tests CLI avant soumission (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Soumettre via Torii (`/v1/sorafs/capacity/declare`) puis capturer `/v1/sorafs/capacity/state` et des captures Grafana. Suivre le flux de sortie dans `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archiver les artefacts signés et les outputs de réconciliation dans
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dépendances & séquencement

1. Terminer SF-2b (politique d'admission) — le marketplace dépend de providers validés.
2. Implémenter la couche schéma + registry (ce doc) avant l'intégration Torii.
3. Finaliser le pipeline de metering avant d'activer les payouts.
4. Étape finale : activer la distribution de fees pilotée par gouvernance une fois les données de metering vérifiées en staging.

Le progrès doit être suivi dans la roadmap avec des références à ce document. Mettre à jour la roadmap une fois que chaque section majeure (schéma, plan de contrôle, intégration, metering, gestion des disputes) atteint le statut feature complete.
