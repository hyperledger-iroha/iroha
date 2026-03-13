---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : place de marché de capacité de stockage
titre : Marché de capacité de stockage de SoraFS
sidebar_label : Marché de capacité
description : Plan SF-2c pour le marché de capacité, les ordonnances de réplication, la télémétrie et les crochets de gouvernement.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/storage_capacity_marketplace.md`. Gardez les emplacements alignés au milieu de la documentation héritée si active.
:::

# Marché de capacité de stockage de SoraFS (Borrador SF-2c)

L'élément de la feuille de route SF-2c introduit un marché géré par les fournisseurs de
almacenamiento déclarera capacité compromise, recevoir des ordres de réplication et
ganan frais proportionnels à la disponibilidad entregada. Ce document délimité
les éléments requis pour la première sortie et les divisions en pistes accessibles.

## Objets

- Exprimer des compromis de capacité (octets totaux, limites par voie, expiration)
  en une forme de consommable vérifiable par l'administration, le transport SoraNet et Torii.
- Attribuer des broches entre les fournisseurs selon la capacité déclarée, la mise et les restrictions de
  La politique à tous les égards se maintient dans un comportement déterministe.
- Médir l'entrega de stockage (sortie de réplication, disponibilité, preuves d'intégrité) et
  exporter des télémétries pour la distribution des frais.
- Procéder aux processus de révocation et de litige pour les fournisseurs malhonnêtes de Sean
  pénalisés ou supprimés.

## Concepts de propriété

| Concept | Description | Initiale entregable |
|---------|-------------|-----------|
| `CapacityDeclarationV1` | Charge utile Norito qui décrit l'ID du fournisseur, le support de profil de chunker, les compromis GiB, les limites de voie, les pistes de tarification, le compromis de jalonnement et l'expiration. | Esquema + validateur en `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instruction émise par le gouvernement pour attribuer un CID de manifeste à un ou plusieurs fournisseurs, y compris le niveau de redondance et les mesures SLA. | Esquema Norito partagé avec Torii + API de contrat intelligent. |
| `CapacityLedger` | Registre en chaîne/hors chaîne qui rastrea les déclarations de capacité active, les ordonnances de réplication, les mesures de rendu et l'accumulation des frais. | Module de contrat intelligent ou stub de service hors chaîne avec détermination d'instantané. |
| `MarketplacePolicy` | Politique de gouvernance qui définit les enjeux minimes, les exigences des auditoires et les courbes de pénalisation. | Structure de configuration en `sorafs_manifest` + document de gouvernement. |

### Esquemas Implementados (État)

## Desglose de travail

### 1. Capa de esquema et registre| Tarée | Responsable(s) | Notes |
|------|------|------|
| Définissez `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipo de Storage / Gobernanza | Utiliser Norito ; inclure la version sémantique et les références de capacités. |
| Implémenter les modules d'analyseur + validateur en `sorafs_manifest`. | Équipement de stockage | Imponer des identifiants monotones, des limites de capacité, des exigences d'enjeu. |
| Extension des métadonnées du registre chunker avec `min_capacity_gib` par profil. | GT Outillage | Aidez les clients à exiger un minimum de matériel par profil. |
| Rédiger le document `MarketplacePolicy` avec garde-corps d'admission et calendrier de pénalités. | Conseil de gouvernance | Publier des documents avec les défauts de la politique. |

#### Définitions de l'esquema (implémentées)- `CapacityDeclarationV1` capture les compromis de capacité ferme du fournisseur, y compris les poignées canoniques du chunker, les références de capacités, les plafonds optionnels par voie, les listes de prix, les fenêtres de validation et les métadonnées. La validation est assurée sans zéro, gère les canoniques, les alias dédupliqués, les plafonds par voie à l'intérieur du total déclaré et la stabilité de GiB monotonique.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` enlaza manifestes con cessions émises par la gouvernance qui incluent des objets de redondance, des cadres de SLA et des garanties par cession ; Les validateurs imposent des poignées canoniques, des fournisseurs uniques et des restrictions de délai avant que Torii ou le registre consomme la commande.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` exprime des instantanés par époque (GiB déclarés par rapport aux utilisateurs, contrôleurs de réplication, pourcentages de disponibilité/PoR) qui alimentent la distribution des frais. Les validations maintiennent l'utilisation dans la déclaration et les pourcentages dans 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Les aides comparées (`CapacityMetadataEntry`, `PricingScheduleV1`, validateurs de voie/signature/SLA) ont prouvé la détermination de la validation des clés et signalent les erreurs que CI et les outils en aval peuvent réutiliser. 【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` expose désormais l'instantané en chaîne via `/v2/sorafs/capacity/state`, en combinant les déclarations du fournisseur et les entrées du grand livre des frais avec Norito JSON déterministe.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La couverture de validation exerçant l'application des poignées canoniques, la détection des duplications, les limites par voie, les gardes d'attribution de réplication et les contrôles de portée de télémétrie pour que les régressions apparaissent en CI de immédiat.【crates/sorafs_manifest/src/capacity.rs:792】
- Outils pour les opérateurs : `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convertit les spécifications lisibles en charges utiles Norito canoniques, blobs base64 et résumés JSON pour que les opérateurs préparent les appareils de `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` et les ordres de réplication avec validation local.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Les appareils de référence sont présents en `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) et sont générés via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Intégration du plan de contrôle| Tarée | Responsable(s) | Notes |
|------|------|------|
| Gestionnaires d'agrégation Torii `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` avec charges utiles Norito JSON. | Équipe Torii | Réfléchir à la logique du validateur ; réutiliser les assistants Norito JSON. |
| Propager les instantanés de `CapacityDeclarationV1` dans les métadonnées du tableau de bord de l'explorateur et des avions à récupérer sur la passerelle. | Groupe de travail sur l'outillage / Equipo de Orchestrator | Extender `provider_metadata` avec références de capacité pour que le scoring multi-source respecte les limites de la voie. |
| Inspectez les commandes de réplication chez les clients de l'explorateur/passerelle pour guider les attributions et les conseils de basculement. | Équipe Réseautage TL / Gateway | Le constructeur du tableau de bord consomme des ordres fermes par la gouvernance. |
| CLI d'outillage : extension `sorafs_cli` avec `capacity declare`, `capacity telemetry`, `capacity orders import`. | GT Outillage | Proveer JSON déterministe + sorties du tableau de bord. |

### 3. Politique du marché et de la gouvernance

| Tarée | Responsable(s) | Notes |
|------|------|------|
| Ratificar `MarketplacePolicy` (mise minimo, multiplicadores de penalizacion, cadencia de auditoria). | Conseil de gouvernance | Publier des documents, capturer l'histoire des révisions. |
| Agréger les crochets de gouvernement pour que le Parlement puisse approuver, rénover et révoquer les déclarations. | Conseil de gouvernance / équipe Smart Contract | Utiliser les événements Norito + ingérer les manifestes. |
| Mettre en œuvre l'esquema de penalizaciones (réduction des frais, réduction de la caution) liée aux violations des télégraphies SLA. | Conseil de Gouvernance / Trésorerie | Sorties linéaires avec règlement de `DealEngine`. |
| Documenter le processus de litige et la matrice d'escalade. | Documents / Gouvernance | Vinculaire du runbook de litige + helpers de CLI. |

### 4. Médicaments et répartition des frais

| Tarée | Responsable(s) | Notes |
|------|------|------|
| Développez la dose de dosage en Torii pour accepter `CapacityTelemetryV1`. | Équipe Torii | Valider GiB-heure, quitter PoR, disponibilité. |
| Actualiser le pipeline de comptage de `sorafs_node` pour signaler l'utilisation sur commande + estadisticas SLA. | Équipe de stockage | Linéaire avec ordres de réplication et poignées du chunker. |
| Pipeline de règlement : convertir des données télémétriques + des données de réplication en pages libellées en XOR, produire des listes de résultats pour l'administration et enregistrer l'état du grand livre. | Équipe Trésorerie / Stockage | Connectez-vous aux exportations de Deal Engine / Treasury. |
| Exporter des tableaux de bord/alertes pour la santé du comptage (backlog de ingesta, télémétrie obsolète). | Observabilité | Extension du pack de Grafana référencée par SF-6/SF-7. |- Torii ahora expone `/v2/sorafs/capacity/telemetry` et `/v2/sorafs/capacity/state` (JSON + Norito) pour que les opérateurs souhaitent des instantanés de télémétrie par époque et que les inspecteurs récupèrent le grand livre canonique pour les auditoires ou les empaquetés de preuve.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- L'intégration `PinProviderRegistry` garantit que les commandes de réplication sont accessibles au point de terminaison mismo ; les assistants de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) valident et publient désormais la télémétrie des exécutions automatisées avec hachage déterministe et résolution des alias.
- Les instantanés de mesure produits par les entrées `CapacityTelemetrySnapshot` sont fixés à l'instantané `metering`, et les exportations Prometheus alimentent le tableau Grafana liste pour importer en `docs/source/grafana_sorafs_metering.json` pour que les équipements de fabrication soient surveillés. accumulation de GiB-heure, frais nano-SORA projetés et cumul de SLA en temps réel.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Lorsque le lissage de mesure est activé, l'instantané inclut `smoothed_gib_hours` et `smoothed_por_success_bps` pour que les opérateurs comparent les valeurs avec EMA en face des contadores bruts que la gouvernance utilise pour les pages.【crates/sorafs_node/src/metering.rs:401】

### 5. Manière de litiges et de révocations

| Tarée | Responsable(s) | Notes |
|------|------|------|
| Définissez la charge utile `CapacityDisputeV1` (demandante, preuve, objet du fournisseur). | Conseil de gouvernance | Esquema Norito + validateur. |
| Support de CLI pour lancer des litiges et répondre (avec accessoires de preuve). | GT Outillage | Assurez-vous que le hachage est déterminant pour le bundle de preuves. |
| Agréger les contrôles automatisés pour les violations répétées du SLA (auto-escalado a disputa). | Observabilité | Parapluies d'alerte et crochets de gouvernement. |
| Documenter le manuel de révocation (période de grâce, évacuation des données Pineados). | Équipe Documents/Stockage | Vinculaire du document politique et du runbook des opérateurs. |

## Conditions requises pour les tests et CI

- Tests unitarios para todos los validadores de esquema nuevos (`sorafs_manifest`).
- Tests d'intégration simulés : déclaration -> ordre de réplication -> comptage -> paiement.
- Workflow de CI pour régénérer les déclarations/télémétries de capacité et assurer que les entreprises sont synchronisées (extender `ci/check_sorafs_fixtures.sh`).
- Tests de charge pour l'API du registre (10 000 fournisseurs simultanés, 100 000 commandes).

## Télémétrie et tableaux de bord

- Panneaux de tableau de bord :
  - Capacité déclarée ou utilisée par le fournisseur.
  - Carnet de commandes de réplication et de demande de cession.
  - Cumul de SLA (uptime %, tasa de exito PoR).
  - Accumulation de frais et pénalités par époque.
- Alertes :
  - Provider por debajo de la capacidad minima compromis.
  - Ordre de réplication atascada > SLA.
  - Fallas en el pipeline de comptage.

## Entregables de documentation- Guide de l'opérateur pour déclarer la capacité, rénover les compromis et surveiller l'utilisation.
- Guide d'administration pour approuver les déclarations, émettre des ordonnances et gérer les litiges.
- Référence d'API pour les points finaux de capacité et le format d'ordre de réplication.
- FAQ du marché pour les développeurs.

## Liste de contrôle de préparation pour GA

L'élément de la feuille de route **SF-2c** bloque le déploiement et la production sur des preuves concrètes
acerca de contabilidad, manejo de disputas y onboarding. Utiliser les artefacts suivants
pour maintenir les critères d’acceptation en synchronisation avec la mise en œuvre.

### Contrôle nocturne et réconciliation XOR
- Exporter l'instantané de l'état de capacité et l'exportation du grand livre XOR pour le même
  ventana, luego ejecuta:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  El helper sale con codigo no cero si hay colonies o penalizaciones faltantes/excesivas y
  émettre un CV de Prometheus au format fichier texte.
- L'alerte `SoraFSCapacityReconciliationMismatch` (en `dashboards/alerts/sorafs_capacity_rules.yml`)
  dispara quando las metricas de réconciliation reportan lacunes; les tableaux de bord viven fr
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archivage du CV JSON et des hachages en `docs/examples/sorafs_capacity_marketplace_validation/`
  avec les paquets de gouvernement.

### Preuves de litige et de coupure
- Présenter des litiges via `sorafs_manifest_stub capacity dispute` (tests :
  `cargo test -p sorafs_car --test capacity_cli`) pour maintenir les charges utiles canoniques.
- Exécution `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et les suites de
  pénalité (`record_capacity_telemetry_penalises_persistent_under_delivery`) pour vérifier que
  les disputes et les escarmouches se reproduisent de manière déterministe.
- Sigue `docs/source/sorafs/dispute_revocation_runbook.md` pour la capture des preuves et l'escalade ;
  enlaza les autorisations de grève dans le rapport de validation.

### Intégration des prestataires et sortie des tests de fumée
- Régénérer les artefacts de déclaration/télémétrie avec `sorafs_manifest_stub capacity ...` et
  lance les tests de la CLI avant la soumission (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Envialos via Torii (`/v2/sorafs/capacity/declare`) et ensuite capturer `/v2/sorafs/capacity/state` mas
  captures d'écran de Grafana. Siguez el flujo de salida en `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archives des artefacts confirmés et des résultats de réconciliation à l'intérieur de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dépendances et sécurité

1. Terminaire SF-2b (politique d'admission) — le marché dépend des fournisseurs validés.
2. Implémenter un schéma + une tête de registre (ce document) avant l'intégration de Torii.
3. Compléter le pipeline de comptage avant d'autoriser les paiements.
4. Paso final: habilitar distribucion de fees controlada por gobernanza una vez que los datos
   de comptage se vérifie en mise en scène.

Le progrès doit être suivi de la feuille de route avec des références à ce document. Actualiser le
feuille de route una vez que cada sección mayor (esquema, plano de control, integración, metering,
manejo de disputas) alcance estado fonctionnalité complète.