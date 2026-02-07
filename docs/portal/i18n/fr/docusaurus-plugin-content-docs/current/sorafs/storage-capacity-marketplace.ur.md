---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : place de marché de capacité de stockage
titre : SoraFS اسٹوریج کیپیسٹی مارکیٹ پلیس
sidebar_label : کیپیسٹی مارکیٹ پلیس
description : Il s'agit d'ordres de réplication et de commandes SF-2c.
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/storage_capacity_marketplace.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن فعال ہے دونوں لوکیشنز کو ہم آہنگ رکھیں۔
:::

# SoraFS Batterie de secours (SF-2c)

Le SF-2c est en mesure de répondre aux besoins des fournisseurs en matière de capacité engagée. Les commandes de réplication et les frais de livraison ainsi que la disponibilité et les frais supplémentaires. Il existe de nombreux livrables et des pistes exploitables. تقسیم کرتی ہے۔

## مقاصد

- fournisseurs d'engagements de capacité (octets, voies, limites, expiration) et gouvernance du transport SoraNet Torii استعمال کر سکیں۔
- capacité déclarée, enjeu et contraintes politiques, broches et fournisseurs, comportement déterministe et comportement déterministe
- livraison du stockage (succès de la réplication, disponibilité, preuves d'intégrité) et distribution des frais et exportation de télémétrie
- révocation et litige concernant les fournisseurs malhonnêtes et pénalisant ou supprimant les fournisseurs malhonnêtes

## ڈومین کانسیپٹس

| Concepts | Descriptif | Livrable initial |
|---------|-------------|-----------|
| `CapacityDeclarationV1` | Charge utile Norito, ID du fournisseur, prise en charge du profil de chunker, GiB engagé, limites spécifiques aux voies, conseils de tarification, engagement de jalonnement et expiration pour les paiements en ligne. | `sorafs_manifest::capacity` schéma + validateur۔ |
| `ReplicationOrder` | gouvernance et instructions et manifeste CID et les fournisseurs et assigner des niveaux de redondance et des métriques SLA | Torii est un schéma Norito + API de contrat intelligent |
| `CapacityLedger` | registre en chaîne/hors chaîne, déclarations de capacité active, ordres de réplication, mesures de performance et accumulation de frais, etc. | module de contrat intelligent, stub de service hors chaîne et instantané déterministe |
| `MarketplacePolicy` | politique de gouvernance, participation minimale, exigences d'audit et courbes de pénalités | `sorafs_manifest` structure de configuration + document de gouvernance |

### Schémas implémentés (Statut)

## ورک بریک ڈاؤن

### 1. Couche de schéma et de registre| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1` en cours | Équipe Stockage / Gouvernance | Norito Mise à jour gestion des versions sémantiques et références aux capacités |
| `sorafs_manifest` analyseur + modules de validation pour les utilisateurs | Équipe de stockage | ID monotones, limites de capacité, exigences de mise |
| métadonnées du registre chunker pour le profil et `min_capacity_gib` pour le profil | GT Outillage | clients et profil requis et configuration matérielle minimale requise pour les clients |
| `MarketplacePolicy` Garde-fous et garde-fous d'admission et barème des pénalités | Conseil de gouvernance | les documents publient les paramètres par défaut de la politique et les paramètres par défaut |

#### Définitions de schéma (implémentées)- `CapacityDeclarationV1` du fournisseur avec des engagements de capacité signés et des captures d'écran avec des poignées de chunker canoniques, des références de capacités, des plafonds de voie facultatifs, des conseils de tarification, des fenêtres de validité et des métadonnées. validation یہ یقینی بناتا ہے کہ mise غیر صفر ہو، gère les alias canoniques dédupliqués ہوں، les capuchons de voie اعلان کردہ total کے اندر ہوں اور Comptabilité GiB monotone ہو۔【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` manifeste des missions émises par la gouvernance, des garanties par mission et des objectifs de redondance, des seuils SLA et des garanties par mission. validateurs, poignées de chunker canoniques, fournisseurs uniques et contraintes de délai.
- Instantanés d'époque `CapacityTelemetryV1` (compteurs de réplication GiB déclarés et utilisés, pourcentages de disponibilité/PoR) et distribution des frais et flux de flux. les limites vérifient les déclarations et les pourcentages jusqu'à 0-100% par rapport aux déclarations et aux pourcentages 【crates/sorafs_manifest/src/capacity.rs:476】
- Aides partagées (`CapacityMetadataEntry`, `PricingScheduleV1`, validateurs de voie/affectation/SLA) validation de clé déterministe et rapport d'erreurs pour la réutilisation des outils CI et la réutilisation des outils en aval. ہیں۔【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` pour un instantané en chaîne et `/v1/sorafs/capacity/state` pour exposer les déclarations du fournisseur et les écritures du grand livre des frais et déterministe Norito JSON pour جوڑ کر۔【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Couverture de validation application canonique des poignées, détection des doublons, limites par voie, gardes d'affectation de réplication et vérifications de la plage de télémétrie et exercices de régressions et de régressions CI pour la mise en œuvre de la CI (crates/sorafs_manifest/src/capacity.rs:792)
- Outils d'opérateur : spécifications lisibles par l'homme `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` et charges utiles canoniques Norito, blobs base64 et résumés JSON pour les opérateurs `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` pour les appareils d'ordre de réplication et la validation locale pour l'étape de préparation 【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Appareils de référence `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) Il s'agit d'un `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` qui génère des problèmes

### 2. Intégration du plan de contrôle| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| Gestionnaires `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` Torii et charges utiles JSON Norito pour les utilisateurs | Équipe Torii | logique du validateur et miroir Norito Les assistants JSON réutilisent les fichiers |
| `CapacityDeclarationV1` instantanés et métadonnées du tableau de bord de l'orchestrateur et plans de récupération de la passerelle qui se propagent | GT Outillage / Equipe Orchestrateur | `provider_metadata` Références de capacité pour les limites des voies de notation multi-sources |
| ordres de réplication et clients orchestrateur/passerelle, flux, affectations et conseils de basculement | Équipe Réseautage TL / Gateway | Ordres de réplication signés par la gouvernance du générateur de tableau de bord |
| Outils CLI : `sorafs_cli` ou `capacity declare`, `capacity telemetry`, `capacity orders import` ou `capacity orders import`. | GT Outillage | JSON déterministe + sorties du tableau de bord |

### 3. Politique et gouvernance du marché

| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| `MarketplacePolicy` (mise minimale, multiplicateurs de pénalité, cadence d'audit) | Conseil de gouvernance | les documents publient et capturent l'historique des révisions. |
| crochets de gouvernance شامل کریں تاکہ Déclarations du Parlement کو approuver، renouveler اور révoquer کر سکے۔ | Conseil de gouvernance / équipe Smart Contract | Événements Norito + ingestion de manifeste استعمال کریں۔ |
| barème des pénalités (réduction des frais, réduction des obligations) et violations télémesurées des SLA | Conseil de Gouvernance / Trésorerie | `DealEngine` sorties de règlement et aligner les sorties |
| processus de règlement des différends et matrice d'escalade | Documents / Gouvernance | runbook de litige + assistants CLI |

### 4. Comptage et répartition des frais

| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| Torii mesure de l'ingestion et `CapacityTelemetryV1` pour le comptage des données | Équipe Torii | GiB-heure, réussite du PoR, validation de la disponibilité |
| Pipeline de comptage `sorafs_node` pour utilisation par commande + statistiques SLA | Équipe de stockage | les ordres de réplication et les poignées du chunker sont alignés |
| Pipeline de règlement : télémétrie + données de réplication et paiements libellés en XOR et résumés prêts pour la gouvernance par rapport à l'état du grand livre | Équipe Trésorerie / Stockage | Deal Engine / Exportations du Trésor en fil de fer |
| état de mesure (arriéré d'ingestion, télémétrie obsolète) et exportation de tableaux de bord/alertes | Observabilité | Pack SF-6/SF-7 avec support Grafana et extension de support |- Torii et `/v1/sorafs/capacity/telemetry` et `/v1/sorafs/capacity/state` (JSON + Norito) exposent les instantanés de télémétrie de l'époque des opérateurs et des inspecteurs audit et emballage des preuves dans le grand livre canonique حاصل کر سکیں۔【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- Intégration `PinProviderRegistry` pour les commandes de réplication et les points de terminaison. Aides CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) pour le hachage déterministe et la résolution d'alias pour les exécutions d'automatisation et la validation/publication de télémétrie.
- Instantanés de mesure des entrées `CapacityTelemetrySnapshot` entre les instantanés `metering` et épinglés dans Prometheus exports `docs/source/grafana_sorafs_metering.json` Carte Grafana prête à l'importation et flux de données pour les équipes de facturation Accumulation de GiB-heures, frais nano-SORA projetés et conformité SLA en temps réel سکیں۔【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Le lissage de la mesure est activé et l'instantané est disponible pour les valeurs `smoothed_gib_hours` et `smoothed_por_success_bps` pour les opérateurs, les valeurs tendance EMA et les compteurs bruts. Il existe des paiements de gouvernance et des paiements de gouvernance pour les paiements en ligne 【crates/sorafs_node/src/metering.rs:401】

### 5. Gestion des litiges et des révocations

| Tâche | Propriétaire(s) | Remarques |
|------|----------|-------|
| Charge utile `CapacityDisputeV1` (plaignant, preuve, fournisseur cible) | Conseil de gouvernance | Schéma Norito + validateur۔ |
| litiges pour le support CLI (pièces jointes de preuves par exemple)۔ | GT Outillage | paquet de preuves pour le hachage déterministe |
| violations répétées du SLA et contrôles automatisés (transfert automatique en litige) | Observabilité | seuils d'alerte et crochets de gouvernance |
| playbook de révocation (délai de grâce, données épinglées et évacuation) | Équipe Documents/Stockage | document de politique et runbook de l'opérateur |

## Exigences de tests et de CI

- Il existe des validateurs de schéma et des tests unitaires (`sorafs_manifest`).
- tests d'intégration et simulation de mots : déclaration → ordre de réplication → comptage → paiement
- Flux de travail CI et exemples de déclarations de capacité/régénération de télémétrie et synchronisation des signatures (`ci/check_sorafs_fixtures.sh` pour étendre la fonction)
- API de registre et tests de charge (10 000 fournisseurs, 100 000 commandes simulent des milliers)

## Télémétrie et tableaux de bord

- Panneaux du tableau de bord :
  - le fournisseur کے حساب سے a déclaré la capacité utilisée۔
  - arriéré des ordres de réplication et délai d'affectation moyen
  - Conformité SLA (% de disponibilité et taux de réussite du PoR)۔
  - فی accumulation de frais d'époque et pénalités۔
- Alertes :
  - fournisseur کم از کم capacité engagée سے نیچے۔
  - l'ordre de réplication SLA est bloqué
  - défaillances du pipeline de comptage۔

## Livrables de documentation- déclaration de capacité, engagements renouvelés, utilisation, guide de l'opérateur
- les déclarations approuvent les ordonnances et les litiges traitent le guide de gouvernance
- points de terminaison de capacité et format d'ordre de réplication et référence API
- FAQ des développeurs et du marché

## Liste de contrôle de préparation à GA

Il s'agit de la comptabilité **SF-2c**, de la gestion des litiges et de l'intégration ainsi que du déploiement de la production et du déploiement de la production. La mise en œuvre des critères d'acceptation et la synchronisation des artefacts sont terminées.

### Comptabilité nocturne et rapprochement XOR
- instantané de l'état de la capacité lors de l'exportation du grand livre XOR et de la fenêtre de configuration :
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  یہ aide manquante/règlements trop payés یا pénalités پر sortie non nulle کرتا ہے اور Prometheus résumé du fichier texte émettre کرتا ہے۔
- Alerte `SoraFSCapacityReconciliationMismatch` (`dashboards/alerts/sorafs_capacity_rules.yml` ici)
  mesures de réconciliation et écarts entre les incendies et les incendies tableaux de bord `dashboards/grafana/sorafs_capacity_penalties.json` ہیں۔
- Résumé JSON des hachages et `docs/examples/sorafs_capacity_marketplace_validation/` des archives de fichiers
  paquets de gouvernance کے ساتھ۔

### Contestation et réduction des preuves
- litiges `sorafs_manifest_stub capacity dispute` کے ذریعے فائل کریں (essais :
  `cargo test -p sorafs_car --test capacity_cli`) Charges utiles canoniques
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et suites de pénalités
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) Il y a des différends et des coupures de rediffusion déterministe
- capture de preuves et escalade pour `docs/source/sorafs/dispute_revocation_runbook.md` فالو کریں؛ approbations de grève et rapport de validation

### Tests de fumée d'intégration et de sortie des fournisseurs
- artefacts de déclaration/télémétrie comme `sorafs_manifest_stub capacity ...` ou régénération et soumission et relecture des tests CLI (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)
- Torii (`/v1/sorafs/capacity/declare`) et soumettre la capture d'écran `/v1/sorafs/capacity/state` et Grafana capturer des captures d'écran. `docs/source/sorafs/capacity_onboarding_runbook.md` Flux de sortie pour le flux de sortie
- artefacts signés et sorties de réconciliation selon `docs/examples/sorafs_capacity_marketplace_validation/` dans les archives

## Dépendances et séquençage

1. SF-2b (politique d'admission) مکمل کریں - fournisseurs approuvés par le marché پر انحصار کرتا ہے۔
2. Intégration Torii avec schéma de base + couche de registre (en français)
3. les paiements permettent de gérer le pipeline de comptage
4. Étapes : mise en scène et vérification des données de comptage et activation de la distribution des frais contrôlée par la gouvernance.

progrès et feuille de route La fonctionnalité de gestion des litiges (schéma, plan de contrôle, intégration, mesure, gestion des litiges) est complète et la feuille de route est terminée.