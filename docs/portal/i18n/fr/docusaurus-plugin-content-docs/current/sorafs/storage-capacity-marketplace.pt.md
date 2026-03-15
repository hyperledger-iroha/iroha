---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : place de marché de capacité de stockage
titre : Marché de capacité d'armement SoraFS
sidebar_label : Marché de capacité
description : Plan SF-2c pour marché de capacité, commandes de réplication, télémétrie et crochets de gouvernance.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/storage_capacity_marketplace.md`. Mantenha ambos os local alinhados enquanto a documentacao herdada permanecer ativa.
:::

# Marché de capacité d'armement SoraFS (Rascunho SF-2c)

L'article SF-2c de la feuille de route introduit un marché régi par tous les fournisseurs de
Armazenamento déclare la capacité compromise, reçoit des ordres de réplication et
ganham fees proporcionais a disponibilidade entregue. Ce document délimité
os entregaveis exigidos para a primeira release e os share em trilhas acionaveis.

## Objets

- Exprimer les compromis de capacité (octets totaux, limites par voie, expiration)
  au format vérifié pour la consommation par la gouvernance, le transport SoraNet et Torii.
- Localiser les broches entre les fournisseurs en accord avec la capacité déclarée, la mise et
  restrictions de politique avec un comportement déterministe.
- Medir entrega de armazenamento (succès de réplication, disponibilité, preuves de
  integridade) et exporter des télémétries pour la distribution des frais.
- Prover processus de revogacao et litige pour que les fournisseurs desonestos sejam
  pénalisés ou supprimés.

## Concepts de propriété

| Conceito | Description | Entregavel initiale |
|---------|-----------|---------------|
| `CapacityDeclarationV1` | Payload Norito décrit l'ID du fournisseur, prend en charge le profil de chunker, les compromis GiB, les limites de voie, les conseils de tarification, les compromis de jalonnement et d'expiration. | Esquema + validateur dans `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instructions émises pour la gouvernance qui attribue un CID au manifeste d'un ou de plusieurs fournisseurs, y compris le niveau de redondance et les mesures SLA. | Esquema Norito partagé avec Torii + API de contrat intelligent. |
| `CapacityLedger` | Registre en chaîne/hors chaîne qui déclare des capacités actives, des ordonnances de réplication, des mesures de performance et l'accumulation de frais. | Module de contrat intelligent ou stub de service hors chaîne avec instantané déterminé. |
| `MarketplacePolicy` | La politique de gouvernance définit les enjeux minimes, les exigences de l'auditoire et les courbes de pénalité. | Structure de configuration dans `sorafs_manifest` + document de gouvernance. |

### Esquemas implémentés (statut)

## Déménagement du travail

### 1. Caméra de schéma et de registre| Taréfa | Responsavel(est) | Notes |
|------|--------|-------|
| Définissez `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Équipe Stockage / Gouvernance | Utiliser Norito ; inclure la version sémantique et les références de capacité. |
| Implémenter des modules d'analyseur + validateur dans `sorafs_manifest`. | Équipe de stockage | Import ID monotoniques, limites de capacité, exigences de mise. |
| Estender les métadonnées du registre chunker avec `min_capacity_gib` par profil. | GT Outillage | Ajuda clients a important requisitos minimos de hardware por perfil. |
| Redigir documento `MarketplacePolicy` capturando garde-corps d'admission et calendrier des pénalités. | Conseil de gouvernance | Publier des documents avec les défauts de la politique. |

#### Définitions du schéma (implémentées)- `CapacityDeclarationV1` capture les compromis de capacité perdus par le fournisseur, y compris les poignées canoniques du chunker, les références de capacité, les plafonds optionnels par voie, les conseils de tarification, les lignes de validation et les métadonnées. Une validation de garantie de mise nao zéro, gère les canoniques, les alias dédupliqués, les majuscules par voie à l'intérieur du total déclaré et la comptabilité de GiB en croissant monotonique. [crates/sorafs_manifest/src/capacity.rs:28]
- `ReplicationOrderV1` vincula manifeste des attributs émis pour la gouvernance avec des métadonnées de redondance, des limites de SLA et des garanties pour l'attribut ; Les validateurs impoem gèrent les canoniques, les fournisseurs uniques et les restrictions de délai avant l'Torii ou le registre entrant dans la commande. [crates/sorafs_manifest/src/capacity.rs:301]
- `CapacityTelemetryV1` exprime des instantanés par époque (GiB déclarés par rapport aux utilisateurs, contrôleurs de réplication, pourcentage de disponibilité/PoR) qui alimentent la distribution des frais. Les limites de contrôle sont utilisées à l'intérieur des déclarations et en pourcentage à l'intérieur de 0 à 100 %. [crates/sorafs_manifest/src/capacity.rs:476]
- Compartiments d'aides (`CapacityMetadataEntry`, `PricingScheduleV1`, validateurs de voie/attribution/SLA) pour la validation déterministe des touches et le rapport d'erreur réutilisé par CI et l'outillage en aval. [crates/sorafs_manifest/src/capacity.rs:230]
- `PinProviderRegistry` expose maintenant l'instantané en chaîne via `/v1/sorafs/capacity/state`, en combinant les déclarations des fournisseurs et les entrées du grand livre des frais par le biais de Norito JSON déterminé. [crates/iroha_torii/src/sorafs/registry.rs:17] [crates/iroha_torii/src/sorafs/api.rs:64]
- Une couverture de validation exercée sur l'application des poignées canoniques, la détection des duplications, les limites de voie, les gardes d'attribution de réplication et les contrôles de portée de télémétrie pour que la régression apparaisse immédiatement dans CI. [crates/sorafs_manifest/src/capacity.rs:792]
- Outillage pour les opérateurs : `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convertit les spécifications en charges utiles Norito canoniques, blobs base64 et résumés JSON pour que les opérateurs préparent les appareils de `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` et les commandes de réplication avec validation locale. [crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1] Les luminaires de référence sont présents dans `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) et à Sao Gerados via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Intégration du plan de contrôle| Taréfa | Responsavel(est) | Notes |
|------|--------|-------|
| Gestionnaires supplémentaires Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` avec charges utiles Norito JSON. | Équipe Torii | Espelhar logique du validateur ; réutiliser les assistants Norito JSON. |
| Propager les instantanés `CapacityDeclarationV1` pour les métadonnées du tableau de bord de l'orchestrateur et les plans de récupération de la passerelle. | GT Outillage / Equipe Orchestrateur | Estender `provider_metadata` avec des références de capacité pour que l'évaluation des limites de répit multi-sources par voie. |
| Alimenter les commandes de réplication sur les clients de l'orchestrateur/passerelle pour orienter les attributs et les conseils de basculement. | Équipe Réseautage TL / Gateway | Le constructeur du tableau de bord consome ordens assinadas pela gouvernance. |
| CLI d'outillage : estender `sorafs_cli` avec `capacity declare`, `capacity telemetry`, `capacity orders import`. | GT Outillage | Fornecer JSON déterministe + dit du tableau de bord. |

### 3. Politique du marché et gouvernance

| Taréfa | Responsavel(est) | Notes |
|------|--------|-------|
| Ratificar `MarketplacePolicy` (mise minimo, multiplicadores de penalidade, cadencia de auditoria). | Conseil de gouvernance | Publier des documents, capturer l'historique des révisions. |
| Ajouter des crochets de gouvernance pour que le Parlement puisse approuver, rénover et révoquer les déclarations. | Conseil de gouvernance / équipe Smart Contract | Utiliser les événements Norito + ingérer les manifestes. |
| Mettre en œuvre un calendrier de pénalités (réduction des frais, réduction des obligations) lié aux violations des télégraphies SLA. | Conseil de Gouvernance / Trésorerie | Alinhar com publie le règlement `DealEngine`. |
| Documenter le processus de litige et la matrice d'escalade. | Documents / Gouvernance | Lien vers le runbook de litige + les assistants de CLI. |

### 4. Comptage et répartition des frais

| Taréfa | Responsavel(est) | Notes |
|------|--------|-------|
| Augmenter l'absorption du dosage par Torii pour obtenir `CapacityTelemetryV1`. | Équipe Torii | Valider GiB-heure, succès PoR, disponibilité. |
| Actualiser le pipeline de comptage du `sorafs_node` pour signaler l'utilisation sur commande + statistiques SLA. | Équipe de stockage | Alinhar com ordens deplicacao e handles de chunker. |
| Pipeline de règlement : convertisseur de télémétrie + données de réplication sur des paiements libellés en XOR, produisant rapidement des CV pour la gouvernance et le registraire de l'état du grand livre. | Équipe Trésorerie / Stockage | Connectez-vous aux exportations Deal Engine / Treasury. |
| Exporter des tableaux de bord/alertes pour le comptage (backlog de ingesta, télémétrie obsolète). | Observabilité | Estender pack de Grafana référencé par SF-6/SF-7. |- Torii expose maintenant `/v1/sorafs/capacity/telemetry` et `/v1/sorafs/capacity/state` (JSON + Norito) pour que les opérateurs souhaitent des instantanés de télémétrie à l'époque et que les inspecteurs récupèrent le grand livre canonique pour l'auditoire ou l'enregistrement de preuves. [crates/iroha_torii/src/sorafs/api.rs:268] [crates/iroha_torii/src/sorafs/api.rs:816]
- Une intégration `PinProviderRegistry` garantit que les commandes de réplication sont acessives pour mon point final ; les assistants de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) viennent de valider et de publier des télémétries à partir des exécutions automatisées avec le hachage déterministe et la résolution des alias.
- Les instantanés du produit de mesure entrants `CapacityTelemetrySnapshot` sont fixés sur l'instantané `metering`, et les exportations Prometheus alimentent la carte Grafana immédiatement pour l'importation dans `docs/source/grafana_sorafs_metering.json` pour que les équipements de surveillance de charge s'accumulent. GiB-heure, frais nano-SORA projetés et conformité SLA en temps réel. [crates/iroha_torii/src/routing.rs:5143] [docs/source/grafana_sorafs_metering.json:1]
- Lorsque le lissage des compteurs est activé, l'instantané inclut `smoothed_gib_hours` et `smoothed_por_success_bps` pour que les opérateurs comparent les valeurs avec l'EMA avec les contadores bruts utilisés pour la gouvernance des paiements. [crates/sorafs_node/src/metering.rs:401]

### 5. Traitement des litiges et révocation

| Taréfa | Responsavel(est) | Notes |
|------|--------|-------|
| Définir la charge utile `CapacityDisputeV1` (réclamation, preuve, fournisseur également). | Conseil de gouvernance | Schéma Norito + validateur. |
| Support de CLI pour ouvrir des litiges et répondre (avec anexos de evidencia). | GT Outillage | Garantir le hachage déterministe du bundle de preuves. |
| Adicionar checks automatizados para violacoes SLA repetidas (auto-escalada para disputa). | Observabilité | Limites d'alerte et crochets de gouvernance. |
| Playbook documentaire de revogacao (periodo de graca, evacuacao de dados pinados). | Équipe Documents/Stockage | Lien vers le document politique et le runbook de l'opérateur. |

## Conditions requises pour les testicules et CI

- Testes unitarios para todos os novos validadores de schema (`sorafs_manifest`).
- Tests d'intégration qui simulent : déclaration -> ordre de réplication -> comptage -> paiement.
- Workflow de CI pour régénérer les déclarations/télémétries de capacité et garantir que les assinaturas permanecam sincronizadas (estender `ci/check_sorafs_fixtures.sh`).
- Tests de charge pour une API de registre (10 000 fournisseurs simultanés, 100 000 commandes).

## Télémétrie et tableaux de bord

- Paines de tableau de bord :
  - Capacité déclarée ou utilisée par le fournisseur.
  - Carnet de commandes de réplication et retard moyen d'attribution.
  - Conformité de SLA (uptime %, taxa de successo PoR).
  - Accumulation de frais et pénalités par époque.
- Alertes :
  - Fournisseur abaixo da capacidade minima compromis.
  - Ordre de réplication du travail > SLA.
  - Falhas pas de pipeline de comptage.

## Entregaveis de documentacao- Guide de l'opérateur pour déclarer la capacité, rénover les compromis et surveiller l'utilisation.
- Guide de gouvernance pour approuver les déclarations, émettre des ordonnances et résoudre les litiges.
- Référence d'API pour les points finaux de capacité et le format de commande de réplication.
- FAQ du marché pour les développeurs.

## Checklist de préparation GA

L'élément de la feuille de route **SF-2c** bloque le déploiement en production sur la base de preuves concrètes
sur la stabilité, le traitement des litiges et l'intégration. Utilisez os artefatos abaixo para
respectez les critères de synchronisation avec la mise en œuvre.

### Contabilité nocturne et réconciliation XOR
- Exporter l'instantané de l'état de capacité et l'exportation du grand livre XOR pour une semaine,
  après exécuter:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  O helper sai com codigo nao zero em colonies ou penalidades ausentes/excessivas e émette
  un résumé Prometheus dans un fichier texte.
- L'alerte `SoraFSCapacityReconciliationMismatch` (em `dashboards/alerts/sorafs_capacity_rules.yml`)
  dispara quando metricas de reconciliacao reportam lacunes; tableaux de bord ficam em
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archiver le résumé JSON et hacher le `docs/examples/sorafs_capacity_marketplace_validation/`
  avec des paquets de gouvernance.

### Preuves de litige et de coupure
- Archiver les litiges via `sorafs_manifest_stub capacity dispute` (tests :
  `cargo test -p sorafs_car --test capacity_cli`) pour gérer les charges utiles canoniques.
- Exécuter `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e en tant que suites
  de pénalité (`record_capacity_telemetry_penalises_persistent_under_delivery`) pour prouver
  que contestas e slashes fazem replay deterministico.
- Siga `docs/source/sorafs/dispute_revocation_runbook.md` pour la capture des preuves et l'escalade ;
  Linke aprovacoes de strike no relatorio de validacao.

### Intégration des prestataires et tests de fumée de sortie
- Régénérer les articles de déclaration/télémétrie avec `sorafs_manifest_stub capacity ...` et montés sur os
  tests de CLI avant la soumission (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Sous-méta via Torii (`/v1/sorafs/capacity/declare`) et capture `/v1/sorafs/capacity/state` plus
  les captures d'écran font Grafana. Inscrivez le flux indiqué dans `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archiver les artefatos assassinés et les sorties de réconciliation à l'intérieur de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dépendances et séquencement

1. Finaliser SF-2b (politique d'admission) - le marché dépend des fournisseurs agréés.
2. Implémenter une caméra de schéma + registre (ce document) avant l'intégration Torii.
3. Compléter le pipeline de comptage avant d’autoriser les paiements.
4. Étape finale : permettre la distribution des frais contrôlés par la gouvernance lorsque les données sont prises.
   le comptage est vérifié lors de la mise en scène.

Le progrès doit être rastré sans feuille de route avec références à ce document. Actualiser o
feuille de route avec chaque secao principal (schéma, plan de contrôle, intégration, comptage,
tratamento de disputa) la fonctionnalité de statut est terminée.