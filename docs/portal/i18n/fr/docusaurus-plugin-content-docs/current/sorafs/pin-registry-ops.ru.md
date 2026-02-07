---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pin-registry-ops
titre : Registre des broches d'exploitation
sidebar_label : Registre des broches d'exploitation
description : Surveillance et triage du registre des broches SoraFS et mesures des réplications SLA.
---

:::note Канонический источник
Consultez `docs/source/sorafs/runbooks/pin_registry_ops.md`. Vous pouvez consulter les versions de synchronisation si vous souhaitez obtenir la documentation de Sphinx qui ne vous concerne pas.
:::

## Обзор

Ce runbook est conçu pour surveiller et activer le triage du registre des broches SoraFS et également la gestion de votre service (SLA) pour la réplication. Les mesures postérieures à `iroha_torii` et exportées à partir de Prometheus vers l'espace de noms `torii_sorafs_*`. Torii gère le registre pendant 30 secondes sur le téléphone, à l'emplacement des tableaux de bord qui sont actuellement disponibles pour les opérateurs non-autorisés запрашивает les points de terminaison `/v1/sorafs/pin/*`. Importez le tableau de bord intégré (`docs/source/grafana_sorafs_pin_registry.json`) vers la disposition de votre ordinateur Grafana, qui correspond à votre situation actuelle.

## Mesure de la mesure

| Métrique | Étiquettes | Description |
| ------ | ------ | -------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | L'inventaire se manifeste en chaîne pour la gestion de votre cycle. |
| `torii_sorafs_registry_aliases_total` | — | Les manifestes d'alias actifs sont enregistrés dans le registre. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog pour les réplications, les segments selon l'état. |
| `torii_sorafs_replication_backlog_total` | — | Jauge adaptée, commande `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Учет SLA : `met` demande de paiement, mise à jour dans le contrat, `missed` pour la mise à jour + l'installation, `pending` отражает незавершенные заказы. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Агрегированная латентность завершения (эпохи между выпуском и завершением). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Окна запаса для незавершенных заказов (date limite minус эпоха выдачи). |

Si les jauges sont utilisées pour extraire un instantané, les tableaux de bord doivent également être utilisés avec le boîtier `1m` ou activés.

## Tableau de bord Grafana

Le tableau de bord JSON contient un panneau qui répertorie les opérateurs de processus. Il n'y a aucune raison pour le travail du propriétaire, si vous envisagez de créer des graphismes sobres.

1. **Жизненный цикл manifeste** – `torii_sorafs_registry_manifests_total` (группировка по `status`).
2. **Alias ​​du catalogue** – `torii_sorafs_registry_aliases_total`.
3. **Очередь заказов по статусу** – `torii_sorafs_registry_orders_total` (groupement pour `status`).
4. **Arrêt par rapport aux commandes historiques** – voir `torii_sorafs_replication_backlog_total` et `torii_sorafs_registry_orders_total{status="expired"}` pour la livraison.
5. **Коэффициент успеха SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Déclaration par rapport à la date limite** – sélectionnez `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` et `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilisez la transformation Grafana pour effectuer la préparation `min_over_time`, mais vous n'avez absolument rien à faire avant запаса, par exemple:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Пропущенные заказы (tarif 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Alertes de sécurité temporaires

- **Успех SLA < 0,95 dans 15 minutes**
  - Port: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Création : Pейджинг SRE ; начать la réplication du retard de triage.
- **arriéré en attente d'environ 10**
  - Par: `torii_sorafs_replication_backlog_total > 10` сохраняется 10 min
  - Création : Vérifiez les fournisseurs de téléchargement et planifiez les coûts Torii.
- **Истекшие заказы> 0**
  - Port: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Projet : Vérifiez les manifestes de gouvernance, afin de mettre à jour les fournisseurs de désabonnement.
- **p95 завершения > средний запас до date limite**
  - Port: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: Убедиться, что fournisseurs завершают до date limite ; рассмотреть перераспределение.

### Exemple de publication Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SLA репликации SoraFS ниже целевого"
          description: "Коэффициент успеха SLA оставался ниже 95% в течение 15 минут."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog репликации SoraFS выше порога"
          description: "Ожидающие заказы репликации превысили настроенный бюджет backlog."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Заказы репликации SoraFS истекли"
          description: "По крайней мере один заказ репликации истек за последние пять минут."
```

## Triage des flux de travail

1. **Prendre la décision**
   - Si un SLA est lancé, un backlog est en cours, il s'agit de fournisseurs de services (avec PoR, поздние завершения).
   - Si l'arriéré est rattrapé par des demandes stables, vérifiez l'admission (`/v1/sorafs/pin/*`), afin de modifier les manifestes et d'annuler le règlement.
2. **Provérifier le statut des fournisseurs**
   - Appuyez sur `iroha app sorafs providers list` et vérifiez que les opérations de réplication sont effectuées.
   - Vérifiez les jauges `torii_sorafs_capacity_*`, pour mettre à jour le GiB provisionné et votre PoR.
3. **Prendre la réplique**
   - Vous avez de nouvelles commandes pour `sorafs_manifest_stub capacity replication-order`, alors que l'arriéré (`stat="avg"`) s'ouvre depuis 5 époques (manifeste/utilisation de la voiture) `iroha app sorafs toolkit pack`).
   - Vérifiez la gouvernance, si les alias n'apportent pas de manifestes de liaison actifs (pas de lien `torii_sorafs_registry_aliases_total`).
4. **Ajouter le résultat**
   - Enregistrez les incidents dans le journal d'exploitation SoraFS avec l'horodatage et les manifestes de résumé.
   - Consultez ce runbook, si vous souhaitez accéder à de nouvelles méthodes ou à des tableaux de bord.

## Plan de rénovation

Suivez ce processus de création ou d'utilisation du cache d'alias politique dans le cadre du projet :1. **Подготовить конфигурацию**
   - Enregistrez `torii.sorafs_alias_cache` dans `iroha_config` (utilisateur -> réel) avec les paramètres TTL et les graphiques : `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. L'annonce d'une politique de soutien politique dans `docs/source/sorafs_alias_policy.md`.
   - Pour que les SDK prennent en charge les zones de configuration (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` dans les liaisons Rust / NAPI / Python), le client d'application est connecté à la passerelle.
2. **Exécution à sec et mise en scène**
   - Configurez la configuration de la classe de mise en scène pour modifier la topologie du produit.
   - Sélectionnez `cargo xtask sorafs-pin-fixtures` pour déterminer les appareils alias canoniques afin de les décoder et de procéder à un aller-retour ; Il est nécessaire d'éviter la dérive en amont, ce qui doit permettre d'exploiter le cours d'eau.
   - Programmez les points de terminaison `/v1/sorafs/pin/{digest}` et `/v1/sorafs/aliases` avec une preuve synthétique, envoyant des fichiers frais, une fenêtre d'actualisation, expirés et ayant expiré durement. Vérifiez les codes HTTP, les en-têtes (`Sora-Proof-Status`, `Retry-After`, `Warning`) et votre JSON comme ce runbook.
3. **Включить в продакшене**
   - Vérifiez la nouvelle configuration selon les spécifications standard. En utilisant Torii, vous pouvez installer les services gateways/SDK après avoir modifié les nouvelles politiques de votre journal.
   - Importez `docs/source/grafana_sorafs_pin_registry.json` dans Grafana (ou installez les tableaux de bord de suivi) et installez les panneaux d'actualisation du cache d'alias dans le cadre du NOC.
4. **Proverka после развертывания**
   - Surveillez `torii_sorafs_alias_cache_refresh_total` et `torii_sorafs_alias_cache_age_seconds` pendant 30 minutes. Les touches `error`/`expired` doivent être corrélées à l'actualisation de l'écran ; Il n'y a pas de liste d'opérateurs qui doivent prouver les alias de preuve et les fournisseurs de services avant la production.
   - Assurez-vous que les logs clients vous permettent de résoudre les problèmes politiques (les SDK peuvent vous aider à utiliser la preuve ou à créer un fichier). La configuration préalable du client ne nécessite aucune configuration.
5. **Retour**
   - Si vous ouvrez l'alias et actualisez correctement la zone de travail, vous devez généralement définir la politique, en utilisant `refresh_window` et `positive_ttl` dans la configuration. выполните повторный деплой. N'utilisez pas le `hard_expiry` pour que la preuve soit ouverte.
   - Vérifiez la configuration précédente, en prenant l'instantané `iroha_config`, si le télémètre vous permet de sélectionner une configuration récente. `error`, veuillez ouvrir l'incident pour la transmission de l'alias de génération.

## Matériaux suisses

- `docs/source/sorafs/pin_registry_plan.md` — Réalisation de la carte et mise en œuvre du contexte.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — opérateur de stockage, ajout de ce registre de playbook.