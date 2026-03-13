---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c9d46094c15b4fb17ab305ebd1912d75668de2b2271b39ea3da8353168bbccc
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pin-registry-ops
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Reflète `docs/source/sorafs/runbooks/pin_registry_ops.md`. Gardez les deux versions synchronisées jusqu'au retrait de la documentation Sphinx héritée.
:::

## Vue d'ensemble

Ce runbook décrit comment surveiller et trier le Pin Registry SoraFS et ses accords de niveau de service (SLA) de réplication. Les métriques proviennent de `iroha_torii` et sont exportées via Prometheus sous le namespace `torii_sorafs_*`. Torii échantillonne l'état du registry toutes les 30 secondes en arrière-plan, donc les dashboards restent à jour même quand aucun opérateur n'interroge les endpoints `/v2/sorafs/pin/*`. Importez le dashboard curaté (`docs/source/grafana_sorafs_pin_registry.json`) pour un layout Grafana prêt à l'emploi qui correspond directement aux sections ci-dessous.

## Référence des métriques

| Métrique | Labels | Description |
| ------- | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventaire des manifests on-chain par état de cycle de vie. |
| `torii_sorafs_registry_aliases_total` | — | Nombre d'aliases de manifest actifs enregistrés dans le registry. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog des ordres de réplication segmenté par statut. |
| `torii_sorafs_replication_backlog_total` | — | Gauge de commodité reflétant les ordres `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Comptabilité SLA : `met` compte les ordres terminés dans les délais, `missed` agrège les completions tardives + expirations, `pending` reflète les ordres en attente. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latence de complétion agrégée (époques entre émission et complétion). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Fenêtres de marge des ordres en attente (deadline moins époque d'émission). |

Tous les gauges se réinitialisent à chaque extraction de snapshot, donc les dashboards doivent échantillonner à une cadence `1m` ou plus rapide.

## Dashboard Grafana

Le JSON du dashboard contient sept panneaux couvrant les workflows opérateurs. Les requêtes sont listées ci-dessous pour référence rapide si vous préférez construire des graphiques sur mesure.

1. **Cycle de vie des manifests** – `torii_sorafs_registry_manifests_total` (groupé par `status`).
2. **Tendance du catalogue d'alias** – `torii_sorafs_registry_aliases_total`.
3. **File d'ordres par statut** – `torii_sorafs_registry_orders_total` (groupé par `status`).
4. **Backlog vs ordres expirés** – combine `torii_sorafs_replication_backlog_total` et `torii_sorafs_registry_orders_total{status="expired"}` pour mettre en évidence la saturation.
5. **Ratio de réussite SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latence vs marge de deadline** – superpose `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` et `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilisez les transformations Grafana pour ajouter des vues `min_over_time` lorsque vous avez besoin du plancher absolu de marge, par exemple :

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordres manqués (taux 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Seuils d'alerte

- **Succès SLA < 0.95 pendant 15 min**
  - Seuil : `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Action : Page SRE ; démarrer le triage du backlog de réplication.
- **Backlog en attente au-dessus de 10**
  - Seuil : `torii_sorafs_replication_backlog_total > 10` maintenu pendant 10 min
  - Action : Vérifier la disponibilité des providers et le scheduler de capacité Torii.
- **Ordres expirés > 0**
  - Seuil : `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Action : Inspecter les manifests de gouvernance pour confirmer le churn des providers.
- **p95 de complétion > marge moyenne de deadline**
  - Seuil : `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Action : Vérifier que les providers valident avant les deadlines ; envisager des réassignations.

### Exemples de règles Prometheus

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
          summary: "SLA de réplication SoraFS sous la cible"
          description: "Le ratio de succès SLA est resté sous 95% pendant 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de réplication SoraFS au-dessus du seuil"
          description: "Les ordres de réplication en attente ont dépassé le budget de backlog configuré."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordres de réplication SoraFS expirés"
          description: "Au moins un ordre de réplication a expiré au cours des cinq dernières minutes."
```

## Workflow de triage

1. **Identifier la cause**
   - Si les échecs SLA augmentent alors que le backlog reste faible, concentrer l'analyse sur la performance des providers (échecs PoR, complétions tardives).
   - Si le backlog augmente avec des échecs stables, inspecter l'admission (`/v2/sorafs/pin/*`) pour confirmer des manifests en attente d'approbation du conseil.
2. **Valider l'état des providers**
   - Exécuter `iroha app sorafs providers list` et vérifier que les capacités annoncées correspondent aux exigences de réplication.
   - Vérifier les gauges `torii_sorafs_capacity_*` pour confirmer les GiB provisionnés et le succès PoR.
3. **Réassigner la réplication**
   - Émettre de nouveaux ordres via `sorafs_manifest_stub capacity replication-order` lorsque la marge du backlog (`stat="avg"`) descend sous 5 époques (l'empaquetage manifest/CAR utilise `iroha app sorafs toolkit pack`).
   - Notifier la gouvernance si les aliases n'ont pas de bindings de manifest actifs (baisse inattendue de `torii_sorafs_registry_aliases_total`).
4. **Documenter le résultat**
   - Consigner les notes d'incident dans le journal d'opérations SoraFS avec timestamps et digests de manifest concernés.
   - Mettre à jour ce runbook si de nouveaux modes d'échec ou dashboards sont introduits.

## Plan de déploiement

Suivez cette procédure par étapes lors de l'activation ou du durcissement de la politique de cache d'alias en production :

1. **Préparer la configuration**
   - Mettre à jour `torii.sorafs_alias_cache` dans `iroha_config` (user -> actual) avec les TTL et fenêtres de grâce convenus : `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` et `governance_grace`. Les valeurs par défaut correspondent à la politique de `docs/source/sorafs_alias_policy.md`.
   - Pour les SDKs, diffuser les mêmes valeurs via leurs couches de configuration (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` dans les bindings Rust / NAPI / Python) afin que l'application cliente suive le gateway.
2. **Dry-run en staging**
   - Déployer le changement de configuration sur un cluster de staging qui reflète la topologie de production.
   - Exécuter `cargo xtask sorafs-pin-fixtures` pour confirmer que les fixtures canoniques d'alias décodent et font toujours un round-trip ; toute divergence implique un drift upstream à corriger en premier.
   - Exercer les endpoints `/v2/sorafs/pin/{digest}` et `/v2/sorafs/aliases` avec des preuves synthétiques couvrant fresh, refresh-window, expired et hard-expired. Valider les codes HTTP, les headers (`Sora-Proof-Status`, `Retry-After`, `Warning`) et les champs du corps JSON contre ce runbook.
3. **Activer en production**
   - Déployer la nouvelle configuration pendant la fenêtre de changement standard. L'appliquer d'abord à Torii, puis redémarrer les gateways/services SDK une fois que le nœud confirme la nouvelle politique dans les logs.
   - Importer `docs/source/grafana_sorafs_pin_registry.json` dans Grafana (ou mettre à jour les dashboards existants) et épingler les panneaux de refresh du cache d'alias dans l'espace de travail NOC.
4. **Vérification post-déploiement**
   - Surveiller `torii_sorafs_alias_cache_refresh_total` et `torii_sorafs_alias_cache_age_seconds` pendant 30 minutes. Les pics dans les courbes `error`/`expired` doivent corréler avec les fenêtres de refresh ; une croissance inattendue signifie que les opérateurs doivent inspecter les preuves d'alias et la santé des providers avant de continuer.
   - Confirmer que les logs côté client montrent les mêmes décisions de politique (les SDKs remontent des erreurs quand la preuve est périmée ou expirée). L'absence d'avertissements côté client indique une mauvaise configuration.
5. **Fallback**
   - Si l'émission d'alias prend du retard et que la fenêtre de refresh se déclenche fréquemment, relâcher temporairement la politique en augmentant `refresh_window` et `positive_ttl` dans la config, puis redéployer. Garder `hard_expiry` intact pour que les preuves réellement périmées soient toujours rejetées.
   - Revenir à la configuration précédente en restaurant le snapshot `iroha_config` précédent si la télémétrie continue d'afficher des comptes `error` élevés, puis ouvrir un incident pour tracer les délais de génération d'alias.

## Matériaux liés

- `docs/source/sorafs/pin_registry_plan.md` — roadmap d'implémentation et contexte de gouvernance.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — opérations des workers de stockage, complète ce playbook registry.
