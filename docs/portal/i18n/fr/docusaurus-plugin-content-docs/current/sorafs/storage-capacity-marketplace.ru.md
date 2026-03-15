---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : place de marché de capacité de stockage
titre : Маркетплейс емкости хранения SoraFS
sidebar_label : marchés des marchandises
description : Plan SF-2c pour les marchés publics, les ordres de réplication, les systèmes de télémétrie et les crochets de gouvernance.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/storage_capacity_marketplace.md`. Assurez-vous de copier les données synchronisées lorsque vous activez la documentation.
:::

# Pièces de rechange du marché SoraFS (noir SF-2c)

La feuille de route SF-2c est une place de marché ouverte et une liste de fournisseurs
déclarer les frais d'engagement, payer les commandes de réplication et payer les frais
пропорционально предоставленной доступности. Ce document présente les livrables
pour vous permettre de réaliser et de réaliser des randonnées exploitables.

## Celi

- Фиксировать обязательства fournisseurs по емкости (общие байты, лимиты по lane, срок действия)
  Dans une forme éprouvée, propre à la gouvernance, le transport SoraNet et Torii.
- Permet de récupérer les broches de certains fournisseurs en ce qui concerne les dépenses, les mises et la politique,
  сохраняя детерминированное поведение.
- Définir les délais de livraison (réplications actuelles, disponibilité, preuves de fiabilité) et
  экспортировать телеметрию для распределения frais.
- Préparer le processus de révocation et de litige, que les fournisseurs non désirés peuvent choisir
  наказаны или удалены.

## Concombres domestiques

| Consommateur | Description | Первичный livrable |
|---------|-------------|-----------|
| `CapacityDeclarationV1` | Charge utile Norito, fournisseur d'ID détaillé, chunker de profil avancé, GiB d'entreprise, limites de voie, conseils sur les prix, engagement de mise et engagement de mise en jeu. | Schéma + validateur dans `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Les instructions relatives à la gouvernance, à la création de manifestes CID ou à certains fournisseurs, ainsi que les normes et mesures SLA. | Schéma Norito, utilisé pour le contrat intelligent Torii + API. |
| `CapacityLedger` | Registre en chaîne/hors chaîne, déclarations d'activités supplémentaires, commandes de réplication, mesures de livraison et frais de configuration. | Un module de contrat intelligent ou un service de stub hors chaîne sert à déterminer un instantané. |
| `MarketplacePolicy` | La gouvernance politique, l'enjeu minime, la vérification de l'audit et la crédibilité des stratagèmes. | Structure de configuration dans `sorafs_manifest` + gouvernance du document. |

### Реализованные схемы (status)

## Démarrage du robot

### 1. Слой схем и реестра| Задача | Propriétaire(s) | Première |
|------|----------|-------|
| Supprimez `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Équipe Stockage / Gouvernance | Utilisez Norito ; включить версионирование и ссылки на capacités. |
| Réalisez le module analyseur + validateur dans `sorafs_manifest`. | Équipe de stockage | Обеспечить монотонные IDs, ограничения емкости, требования по enjeu. |
| Ajoutez le chunker de restauration de métadonnées à l'aide de `min_capacity_gib` pour votre profil. | GT Outillage | Le client peut effectuer des tâches minimes sur le matériel selon son profil. |
| Ajoutez le document `MarketplacePolicy`, qui décrit les garde-corps d'admission et les graphiques. | Conseil de gouvernance | Publier dans la documentation sur les paramètres par défaut de la stratégie. |

#### Определения схем (реализованы)

- `CapacityDeclarationV1` est un outil de gestion des composants pour le fournisseur de câbles, avec des poignées canoniques, un chunker, des fonctionnalités et des capuchons optionnels. voie, conseils sur les prix, ainsi que sur la validité et les métadonnées. La validation prend en compte une mise indéterminée, des poignées canoniques, des alias dédoublonnés, des majuscules sur la voie dans les précédents niveaux totaux et monotones GiB.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` связывает manifestes с назначениями, выпущенными gouvernance, с целями избыточности, порогами SLA и гарантиями на affectation ; Les validateurs s'occupent du chunker de poignées canoniques, des fournisseurs uniques et de l'organisation de la date limite, comme Torii ou du registre. commande.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` prend en compte l'époque des instantanés (précision par rapport à l'utilisation de GiB, les réplications de temps, les probabilités de disponibilité/PoR), ainsi que les frais de répartition. Les preuves de puissance sont appliquées dans la déclaration actuelle, avec un pourcentage - au préalable 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Les assistants d'observation (`CapacityMetadataEntry`, `PricingScheduleV1`, voies de validation/affectation/SLA) permettent de déterminer la clé de vérification et le rapport sur le bloc-notes, le dossier Vous pouvez gérer les outils CI et en aval.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` permet de publier un instantané en chaîne à partir de `/v1/sorafs/capacity/state`, de consulter les fournisseurs et de consulter le registre des frais pour déterminer Norito. JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Les validations d'achat fournissent des poignées canoniques, des doubles, des finitions sur la voie, des gardes pour la réplication et les vérifications. Les télémètres à diapason, qui régressent à l'intérieur de CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Outils d'opérateur : `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convertit les spécifications techniques en charges utiles canoniques Norito, les blobs base64 et les résumés JSON, les opérateurs peuvent créer des appareils. `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` et les appareils d'ordre de réplication avec validation locale.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Appareils de référence trouvés dans `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) et généré à partir de `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.### 2. Plan de contrôle d'intégration

| Задача | Propriétaire(s) | Première |
|------|----------|-------|
| Ajoutez les charges utiles Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` avec les charges utiles JSON Norito. | Équipe Torii | Validation de la logique ; Utilisez les assistants JSON Norito. |
| Enregistrez les instantanés `CapacityDeclarationV1` dans l'orchestrateur du tableau de bord des métadonnées et la passerelle de récupération des plans. | GT Outillage / Equipe Orchestrateur | Déterminez les capacités `provider_metadata` en fonction de la capacité de notation de plusieurs systèmes en fonction des limites de la voie. |
| Améliorez les commandes de réplication dans l'orchestrateur/la passerelle des clients pour la mise en œuvre des affectations et des conseils de basculement. | Équipe Réseautage TL / Gateway | Le constructeur de tableau de bord peut prendre en charge les ordres de réplication de gouvernance. |
| Outils CLI : utilisez les commandes `sorafs_cli` et `capacity declare`, `capacity telemetry`, `capacity orders import`. | GT Outillage | Préparer le tableau de bord des sorties JSON +. |

### 3. Marché politique et gouvernance

| Задача | Propriétaire(s) | Première |
|------|----------|-------|
| Утвердить `MarketplacePolicy` (mise en jeu minimale, multiplicateurs de fréquences, периодичность аудита). | Conseil de gouvernance | Опубликовать в docs, зафиксировать историю ревизий. |
| Créez des crochets de gouvernance, pour que le Parlement puisse approuver, renouveler et révoquer les déclarations. | Conseil de gouvernance / équipe Smart Contract | Utiliser les événements Norito + les manifestes d'ingestion. |
| Réalisez les frais graphiques (frais de réduction, réduction des obligations), en vous connectant au SLA par téléphone. | Conseil de Gouvernance / Trésorerie | Согласовать с sorties règlement `DealEngine`. |
| Documenter le processus de litige et l'évolution du dossier. | Documents / Gouvernance | Сослаться на dispute runbook + helpers CLI. |

### 4. Frais de comptage et de paiement

| Задача | Propriétaire(s) | Première |
|------|----------|-------|
| Configurez la mesure de l'ingestion dans Torii pour le `CapacityTelemetryV1`. | Équipe Torii | Validez le GiB-heure, par exemple PoR, la disponibilité. |
| Afficher le comptage de pipeline `sorafs_node` pour obtenir l'application de la commande + statistique SLA. | Équipe de stockage | Comprend les commandes de réplication et gère le chunker. |
| Règlement du pipeline : conversion de données télémétriques + réplication en paiements, nomenclature en XOR, création de résumés prêts pour la gouvernance et fixation du grand livre de comptes. | Équipe Trésorerie / Stockage | Ajouter à Deal Engine / Exportations du Trésor. |
| Exportation de tableaux de bord/alertes pour la mesure des retards (ingestion du backlog, utilisation de la télémétrie). | Observabilité | Téléchargez le paquet Grafana pour votre SF-6/SF-7. |- Torii doit publier `/v1/sorafs/capacity/telemetry` et `/v1/sorafs/capacity/state` (JSON + Norito), les opérateurs peuvent utiliser les instantanés de télémétrie pour эпохам, а инспекторы - получать канонический ledger для аудита или упаковки доказательств.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- L'intégration `PinProviderRegistry` garantit que les commandes de réplication sont envoyées vers le point final ; helpers CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) permet de valider/mettre en place des télémètres lors des exécutions d'automatisation pour déterminer le hachage et définir l'alias.
- Les instantanés de mesure forment des fiches `CapacityTelemetrySnapshot`, sont ajoutés à l'instantané `metering` et les exportations Prometheus correspondent à l'importation. Carte Grafana dans `docs/source/grafana_sorafs_metering.json`, qui commande la facturation en fournissant des GiB-heure, programme les frais nano-SORA et prend en charge les SLA en réalité.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Lorsque le lissage de mesure et l'instantané incluent `smoothed_gib_hours` et `smoothed_por_success_bps`, les opérateurs peuvent utiliser les tendances EMA avec le système. En fait, la gouvernance utilisée pour les paiements.【crates/sorafs_node/src/metering.rs:401】

### 5. Résolution du litige et révocation

| Задача | Propriétaire(s) | Première |
|------|----------|-------|
| Определить payload `CapacityDisputeV1` (заявитель, preuve, целевой fournisseur). | Conseil de gouvernance | Norito schéma + validateur. |
| Поддержка CLI для подачи litiges и ответов (avec pièces jointes preuves). | GT Outillage | Обеспечить детерминированный hachage paquet preuve. |
| Effectuer des vérifications automatiques en cas de litige SLA (escalade automatique en cas de litige). | Observabilité | Alertes poroges et crochets de gouvernance. |
| Documentez la révocation du playbook (délai de grâce, données épinglées). | Équipe Documents/Stockage | Téléchargez le document de stratégie et le runbook de l'opérateur. |

## Test de test et CI

- Tests uniques pour tous les nouveaux programmes de validation (`sorafs_manifest`).
- Tests d'intégration qui simulent : déclaration → ordre de réplication → mesure → paiement.
- Flux de travail CI pour la régénération des échantillons de déclaration/télémétrie des composants et des preuves de synchronisation (en utilisant `ci/check_sorafs_fixtures.sh`).
- Tests de charge pour l'API de registre (avec 10 000 fournisseurs, 100 000 commandes).

## Télémétrie et frontières

- Panneaux du bord de mer :
  - Déclarer ou utiliser le fournisseur.
  - Ordres de réplication en attente et средняя задержка назначения.
  - Соответствие SLA (uptime %, частота успеха PoR).
  - Накопление fees и штрафы по эпохам.
- Alertes :
  - Fournisseur ниже miniмальной заявленной емкости.
  - L'ordre de réplication est plus important que le SLA.
  - Pipeline de comptage.

## Documentation matérielle

- L'opérateur doit déclarer les marchandises, fournir des services et surveiller l'utilisation.
- Руководство по gouvernance для утверждения деклараций, выдачи ordonnances, обработки litiges.
- Référence API pour les points de terminaison et l'ordre de réplication des formats.
- FAQ du marché pour les robots.## Liste des prix pour GA

La feuille de route **SF-2c** bloque le déploiement de la production pour la fourniture de documents concrets
по учету, обработке litiges и онбордингу. Utilisez des objets d'art nouveaux, qui correspondent à des critères
приемки в синхronе с реализацией.

### Votre compte et votre connexion XOR
- Exportez les fichiers de stockage d'instantanés et exportez le grand livre XOR pour toute la période, puis tapez :
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  L'aide s'applique aux nouveaux codoms pour les règlements non nécessaires/pré-plaqués ou les stratagèmes et
  выдаст текстовый файл Prometheus résumé.
- Alerte `SoraFSCapacityReconciliationMismatch` (à `dashboards/alerts/sorafs_capacity_rules.yml`)
  срабатывает, когда réconciliation метрики сообщают о расхождениях; tableaux de bord лежат в
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archiver le résumé JSON et les hachages dans `docs/examples/sorafs_capacity_marketplace_validation/`
  вместе с paquets de gouvernance.

### Доказательства différend et slashing
- Подавайте litiges через `sorafs_manifest_stub capacity dispute` (tests :
  `cargo test -p sorafs_car --test capacity_cli`), les charges utiles sont installées sur les canons.
- Ouvrir le `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et le
  штрафов (`record_capacity_telemetry_penalises_persistent_under_delivery`), que vous devez télécharger
  детерминированное воспроизведение litiges et barres obliques.
- Adressez-vous au `docs/source/sorafs/dispute_revocation_runbook.md` pour votre documentation et
  эскалации; привязывайте approbations strike обратно в rapport de validation.

### Fournisseurs de téléphones portables et de téléphones portables
- Régénérer les déclarations/télémétries des artefacts à partir de `sorafs_manifest_stub capacity ...` et
  Programmez les tests CLI avant la fin (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Sélectionnez Torii (`/v1/sorafs/capacity/declare`), puis fixez-le
  `/v1/sorafs/capacity/state` et étiquettes Grafana. Следуйте flow выхода в
  `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archiver les artefacts et les sorties de réconciliation ici
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Avis et suivi

1. Завершить SF-2b (politique d'admission) - marché опирается на проверенных fournisseurs.
2. Réalisez les schémas + registre (ce document) avant l'intégration Torii.
3. Installer la canalisation de mesurage jusqu'à la fermeture.
4. Dernière question : inclure les frais de contrôle de gouvernance et de répartition des frais après la vérification des données de mesure lors de la mise en scène.

Les progrès doivent être complétés par la feuille de route en lien avec ce document. Consultez la feuille de route après cela,
как каждая основная секция (схемы, plan de contrôle, intégration, comptage, conflits d'ordre) достигнет
fonctionnalité statut complet.