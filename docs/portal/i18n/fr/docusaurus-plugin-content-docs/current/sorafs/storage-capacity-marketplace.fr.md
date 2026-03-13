---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : place de marché de capacité de stockage
titre : Marché de capacité de stockage SoraFS
sidebar_label : Marketplace de capacité
description : Plan SF-2c pour le marché de capacité, les ordres de réplication, la télémétrie et les hooks de gouvernance.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/storage_capacity_marketplace.md`. Gardez les deux emplacements alignés tant que la documentation héritée reste active.
:::

# Marché de capacité de stockage SoraFS (Brouillon SF-2c)

L'item SF-2c de la feuille de route introduit un marché gouverné où les fournisseurs de
stockagent déclarer une capacité engagée, recevoir des ordres de réplication et
perçoivent des frais proportionnels à la disponibilité livrée. Ce cadre documentaire
les livrables requis pour la première release et les déclines en pistes actionnables.

## Objectifs

- Exprimer les engagements de capacité (bytes totaux, limites par voie, expiration)
  sous une forme vérifiable consommable par la gouvernance, le transport SoraNet et Torii.
- Allouer les broches entre fournisseurs selon la capacité déclarée, le mise et les
  contraintes de politique tout en maintenant un comportement déterministe.
- Mesurer la livraison de stockage (succès de réplication, uptime, preuves d'intégrité)
  et exportateur de la télémétrie pour la distribution des redevances.
- Fournir des processus de révocation et de litige afin que les prestataires malhonnêtes
  soient pénalisés ou retirés.

## Concepts de domaine

| Concepts | Descriptif | Habitable initiale |
|---------|-------------|-----------------|
| `CapacityDeclarationV1` | Payload Norito décrivant l'ID du fournisseur, le support du profil chunker, les GiB engagés, les limites par voie, les indications de tarification, l'engagement de staking et l'expiration. | Schéma + validateur dans `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instruction émise par la gouvernance assignant un CID de manifeste à un ou plusieurs fournisseurs, incluant le niveau de redondance et les métriques SLA. | Schéma Norito partagé avec Torii + API de smart contract. |
| `CapacityLedger` | Registre on-chain/off-chain qui convient aux déclarations de capacité active, aux ordres de réplication, aux métriques de performance et à l'accumulation des frais. | Module de contrat intelligent ou stub de service off-chain avec snapshot déterministe. |
| `MarketplacePolicy` | Politique de gouvernance définissant l'enjeu minimum, les exigences d'audit et les courbes de pénalité. | Structure de config dans `sorafs_manifest` + document de gouvernance. |

### Schémas implémentés (statut)

## Découpage du travail

### 1. Couche schéma et registre| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| définir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Équipe Stockage / Gouvernance | Utiliser Norito ; include versionnement sémantique et références de capacités. |
| Implémenter les modules analyseur + validateur dans `sorafs_manifest`. | Équipe de stockage | Imposer des identifiants monotones, des limites de capacité, des exigences d'enjeu. |
| Étendre les métadonnées du registre chunker avec `min_capacity_gib` par profil. | GT Outillage | Aidez les clients à imposer des exigences minimales de matériel par profil. |
| Rédiger le document `MarketplacePolicy` capturant les garde-fous d'admission et le calendrier de pénalités. | Conseil de gouvernance | Publier dans les documents avec les défauts de politique. |

#### Définitions de schéma (implémentées)- `CapacityDeclarationV1` capture des engagements de capacité signés par fournisseur, incluant les poignées canoniques chunker, les références de capacité, les caps optionnels par voie, les conseils de tarification, les fenêtres de validité et les métadonnées. La validation assure un enjeu non nul, des handles canoniques, des alias dédupliqués, des caps par voie dans le total déclaré et une comptabilité GiB monotone.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` associe des manifestes à des missions émises par la gouvernance avec objectifs de redondance, seuils SLA et garanties par mission ; les validateurs imposent des handles canoniques, des fournisseurs uniques et des contraintes de date limite avant ingestion par Torii ou le registre.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` exprime des snapshots par époque (GiB déclarés vs utilisations, compteurs de réplication, pourcentages d'uptime/PoR) qui alimentent la répartition des frais. Les bornes maintiennent l'utilisation dans les déclarations et les pourcentages dans 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Les helpers partagés (`CapacityMetadataEntry`, `PricingScheduleV1`, validateurs lane/assignment/SLA) fournissent une validation déterministe des clés et un reporting d'erreur réutilisable par CI et le outillage en aval.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` expose désormais le snapshot on-chain via `/v2/sorafs/capacity/state`, en combinant déclarations de fournisseurs et entrées du fee ledger derrière un Norito JSON déterministe.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La couverture de validation exerce l'application des poignées canoniques, la détection de doublons, les bornes par voie, les gardes d'assignation de réplication et les contrôles de plage de télémétrie pour que les régressions apparaissent immédiatement en CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Tooling opérateur : `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convertit des specs lisibles en payloads Norito canoniques, blobs base64 et résumés JSON afin que les opérateurs préparent des luminaires `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` et des ordres de réplication avec validation locale.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Les luminaires de référence vivent dans `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) et sont générés via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Intégration du plan de contrôle| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| Ajouter des handlers Torii `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` avec payloads Norito JSON. | Équipe Torii | Répliquer la logique de validation ; réutiliser les helpers Norito JSON. |
| Propager les snapshots `CapacityDeclarationV1` vers les métadonnées du scoreboard d'orchestrator et les plans de fetch gateway. | GT Outillage / Equipe Orchestrateur | Étendre `provider_metadata` avec références de capacité pour que le scoring multi-source respecte les limites par voie. |
| Alimenter les ordres de réplication dans les clients orchestrateur/passerelle pour piloter les affectations et les astuces de basculement. | Équipe Réseautage TL / Gateway | Le constructeur de tableau de bord consomme des ordres signés par la gouvernance. |
| Tooling CLI : étendre `sorafs_cli` avec `capacity declare`, `capacity telemetry`, `capacity orders import`. | GT Outillage | Fournir JSON déterministe + tableau de bord des sorties. |

### 3. Marché politique & gouvernance

| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| Ratifiant `MarketplacePolicy` (mise minimum, multiplicateurs de pénalité, cadence d'audit). | Conseil de gouvernance | Publier dans les docs, capturer l'historique des révisions. |
| Ajouter des crochets de gouvernance pour que le Parlement approuve, renouvelle et révoque les déclarations. | Conseil de gouvernance / équipe Smart Contract | Utiliser les événements Norito + ingestion de manifestes. |
| Implémenter le calendrier de pénalités (réduction de frais, suppression de caution) lié aux violations SLA télémétrées. | Conseil de Gouvernance / Trésorerie | S'aligner avec les sorties de règlement du `DealEngine`. |
| Documenter le processus de litige et la matrice d'escalade. | Documents / Gouvernance | Lier au runbook de dispute + helpers CLI. |

### 4. Comptage et répartition des frais

| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| Étendre l'ingestion de comptage Torii pour accepter `CapacityTelemetryV1`. | Équipe Torii | Valider GiB-heure, succès PoR, uptime. |
| Mettre à jour le pipeline de comptage `sorafs_node` pour reporter utilisation par ordre + stats SLA. | Équipe de stockage | Aligner avec les ordres de réplication et handles chunker. |
| Pipeline de règlement : convertir télémétrie + réplication en paiements nommés en XOR, produire des CV prêts pour gouvernance, et enregistrer l'état du grand livre. | Équipe Trésorerie / Stockage | Connecter à Deal Engine / Exportations du Trésor. |
| Exportateur de tableaux de bord/alertes pour la santé du comptage (backlog d'ingestion, télémétrie obsolète). | Observabilité | Étendre le pack Grafana référencé par SF-6/SF-7. |- Torii expose désormais `/v2/sorafs/capacity/telemetry` et `/v2/sorafs/capacity/state` (JSON + Norito) afin que les opérateurs soumettent des instantanés de télémétrie par époque et que les inspecteurs récupèrent le grand livre canonique pour audit ou packaging de preuves.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- L'intégration `PinProviderRegistry` garantit que les ordres de réplication sont accessibles via le même point de terminaison ; les helpers CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) valident et publient désormais la télémétrie depuis des automatisations avec hachage déterministe et résolution d'alias.
- Les instantanés de comptage produits des entrées `CapacityTelemetrySnapshot` fixés au instantané `metering`, et les exportations Prometheus alimentent le tableau Grafana prêt à importer dans `docs/source/grafana_sorafs_metering.json` afin que les équipes de facturation suivent l'accumulation de GiB-heure, les frais. nano-SORA projetés et la conformité SLA en temps réel.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Lorsque le lissage de comptage est activé, le snapshot inclut `smoothed_gib_hours` et `smoothed_por_success_bps` pour comparer les valeurs EMA aux compteurs bruts utilisés par la gouvernance pour les paiements.【crates/sorafs_node/src/metering.rs:401】

### 5. Gestion des litiges et révocations

| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| définir le payload `CapacityDisputeV1` (plaignant, preuve, fournisseur ciblé). | Conseil de gouvernance | Schéma Norito + validateur. |
| Support CLI pour déposer des litiges et répondre (avec pièces de preuve). | GT Outillage | Assurer un hachage déterministe du bundle de preuves. |
| Ajouter des contrôles automatiques pour violations répétées du SLA (auto-escalade en litige). | Observabilité | Seuils d'alertes et crochets de gouvernance. |
| Documenter le playbook de révocation (période de grâce, évacuation des données pin). | Équipe Documents/Stockage | Lier au doc ​​de politique et au runbook opérateur. |

## Exigences de tests & CI

- Tests unitaires pour tous les nouveaux validateurs de schéma (`sorafs_manifest`).
- Tests d'intégration simulant : déclaration → ordre de réplication → comptage → paiement.
- Workflow CI pour régénérer des déclarations/télémétrie de capacité et assurer la synchronisation des signatures (étendre `ci/check_sorafs_fixtures.sh`).
- Tests de charge pour l'APIregistre (simulateur 10k fournisseurs, 100k commandes).

## Télémétrie & tableaux de bord

- Panneaux de tableau de bord :
  - Capacité déclarée vs utilisée par le fournisseur.
  - Backlog des ordres de réplication et délai moyen d'assignation.
  - Conformité SLA (uptime %, taux de succès PoR).
  - Accumulation des frais et pénalités par époque.
- Alertes :
  - Fournisseur sous la capacité minimale engagée.
  - Ordre de réplication bloqué > SLA.
  - Échecs du pipeline de comptage.

## Livrables de documentation- Guide opérateur pour déclarer la capacité, renouveler les engagements et surveiller l'utilisation.
- Guide de gouvernance pour approuver les déclarations, émettre des ordres et gérer les litiges.
- Référence API pour les endpoints de capacité et le format d'ordre de réplication.
- Marketplace FAQ pour les développeurs.

## Checklist de préparation GA

L'item de roadmap **SF-2c** conditionne le déploiement production à des preuves concrètes
sur la comptabilité, la gestion des litiges et l'onboarding. Utiliser les artefacts
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
  Le helper sort avec un code non nul en cas de règlements/pénalités manquants ou excessifs et émet un résumé Prometheus au format fichier texte.
- L'alerte `SoraFSCapacityReconciliationMismatch` (dans `dashboards/alerts/sorafs_capacity_rules.yml`) se déclenche lorsque les métriques de réconciliation signalent des écarts ; tableaux de bord dans `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archiver le CV JSON et les hashes sous `docs/examples/sorafs_capacity_marketplace_validation/` avec les paquets de gouvernance.

### Preuve de contestation & slashing
- Déposer des litiges via `sorafs_manifest_stub capacity dispute` (tests :
  `cargo test -p sorafs_car --test capacity_cli`) pour garder des charges utiles canoniques.
- Lancer `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et les suites de pénalité (`record_capacity_telemetry_penalises_persistent_under_delivery`) pour prouver que les litiges et les slashes rejouent de manière déterministe.
- Suivre `docs/source/sorafs/dispute_revocation_runbook.md` pour la capture de preuves et l'escalade ; lier les approbations de grève au rapport de validation.

### Onboarding des prestataires et smoke tests de sortie
- Régénérer les artefacts de déclaration/télémétrie avec `sorafs_manifest_stub capacity ...` et rejouer les tests CLI avant soumission (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Soumettre via Torii (`/v2/sorafs/capacity/declare`) puis capturer `/v2/sorafs/capacity/state` et des captures Grafana. Suivre le flux de sortie dans `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archiver les artefacts signés et les sorties de réconciliation dans
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dépendances & séquencement

1. Terminer SF-2b (politique d'admission) — le marché dépend de fournisseurs validés.
2. Implémenter la couche schéma + registre (ce doc) avant l'intégration Torii.
3. Finaliser le pipeline de comptage avant d'activer les paiements.
4. Étape finale : activer la répartition de frais pilotée par gouvernance une fois les données de comptage vérifiées en staging.

Le progrès doit être suivi dans la feuille de route avec des références à ce document. Mettre à jour la feuille de route une fois que chaque section majeure (schéma, plan de contrôle, intégration, comptage, gestion des litiges) atteint le statut fonctionnalité complète.