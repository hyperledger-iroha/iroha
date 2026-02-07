---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pin-registry-ops
titre : Registre des Opérations du Pin
sidebar_label : Registre des opérations du Pin
description : Surveillez et triez le Pin Registry SoraFS et les métriques SLA de réplication.
---

:::note Source canonique
Reflète `docs/source/sorafs/runbooks/pin_registry_ops.md`. Gardez les deux versions synchronisées jusqu'au retrait de la documentation Sphinx héritée.
:::

## Vue d'ensemble

Ce runbook décrit comment surveiller et trier le Pin Registry SoraFS et ses accords de niveau de service (SLA) de réplication. Les métriques proviennent de `iroha_torii` et sont exportées via Prometheus sous le namespace `torii_sorafs_*`. Torii échantillonne l'état du registre toutes les 30 secondes en arrière-plan, donc les tableaux de bord restent à jour même quand aucun opérateur n'interroge les endpoints `/v1/sorafs/pin/*`. Importez le tableau de bord curaté (`docs/source/grafana_sorafs_pin_registry.json`) pour une mise en page Grafana prête à l'emploi qui correspond directement aux sections ci-dessous.

## Référence des métriques

| Métrique | Étiquettes | Descriptif |
| ------- | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventaire des manifestes en chaîne par état de cycle de vie. |
| `torii_sorafs_registry_aliases_total` | — | Nombre d'alias de manifestes actifs enregistrés dans le registre. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog des ordres de réplication segmenté par statut. |
| `torii_sorafs_replication_backlog_total` | — | Jauge de commodité reflétant les commandes `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Comptabilité SLA : `met` compte les commandes terminées dans les délais, `missed` agrège les achèvements tardifs + expirations, `pending` reflète les commandes en attente. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latence de achèvement agrégée (époques entre émission et achèvement). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Marge des ordres en attente (deadline moins Fenêtres époque d'émission). |

Tous les jauges se réinitialisent à chaque extraction de snapshot, donc les tableaux de bord doivent échantillonner à une cadence `1m` ou plus rapide.

## Tableau de bord Grafana

Le JSON du tableau de bord contient sept panneaux couvrant les workflows opérateurs. Les requêtes sont répertoriées ci-dessous pour référence rapide si vous préférez construire des graphiques sur mesure.

1. **Cycle de vie des manifestes** – `torii_sorafs_registry_manifests_total` (groupé par `status`).
2. **Tendance du catalogue d'alias** – `torii_sorafs_registry_aliases_total`.
3. **File d'ordres par statut** – `torii_sorafs_registry_orders_total` (groupé par `status`).
4. **Backlog vs commandes expirés** – combiner `torii_sorafs_replication_backlog_total` et `torii_sorafs_registry_orders_total{status="expired"}` pour mettre en évidence la saturation.
5. **Ratio de réussite SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Latence vs marge de date limite** – superposez `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` et `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilisez les transformations Grafana pour ajouter des vues `min_over_time` lorsque vous avez besoin du plancher absolu de marge, par exemple :

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordres manqués (taux 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Seuils d'alerte

- **Succès SLA < 0,95 pendant 15 min**
  - Seuil : `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Action : Page SRE ; démarrez le triage du backlog de réplication.
- **Backlog en attente au-dessus de 10**
  - Seuil : `torii_sorafs_replication_backlog_total > 10` pendentif maintenu 10 min
  - Action : Vérifier la disponibilité des fournisseurs et le planificateur de capacité Torii.
- **Ordres expirés > 0**
  - Seuil : `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Action : Inspecter les manifestes de gouvernance pour confirmer le churn des fournisseurs.
- **p95 de achèvement > marge moyenne de délai**
  - Seuil : `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Action : Vérifier que les fournisseurs sont valides avant les délais ; envisagé des réaffectations.

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

1. **Identifiant de la cause**
   - Si les échecs SLA augmentent alors que le backlog reste faible, concentrez-vous sur l'analyse sur la performance des fournisseurs (échecs PoR, achèvements tardifs).
   - Si le backlog augmente avec des échecs stables, inspecteur l'admission (`/v1/sorafs/pin/*`) pour confirmer des manifestes en attente d'approbation du conseil.
2. **Valider l'état des fournisseurs**
   - Exécuter `iroha app sorafs providers list` et vérifier que les capacités annoncées correspondent aux exigences de réplication.
   - Vérifier les jauges `torii_sorafs_capacity_*` pour confirmer les GiB provisionnés et le succès PoR.
3. **Réaffecter la réplication**
   - Émettre de nouveaux ordres via `sorafs_manifest_stub capacity replication-order` lorsque la marge du backlog (`stat="avg"`) descend sous 5 époques (l'empaquetage manifest/CAR utilise `iroha app sorafs toolkit pack`).
   - Notifier la gouvernance si les alias n'ont pas de liaisons de manifestes actifs (baisse inattendue de `torii_sorafs_registry_aliases_total`).
4. **Documenter le résultat**
   - Consigner les notes d'incident dans le journal d'opérations SoraFS avec timestamps et résumés de manifeste concerné.
   - Mettre à jour ce runbook si de nouveaux modes d'échec ou tableaux de bord sont introduits.

## Plan de déploiement

Suivez cette procédure par étapes lors de l'activation ou du renforcement de la politique de cache d'alias en production :1. **Préparer la configuration**
   - Mettre à jour `torii.sorafs_alias_cache` dans `iroha_config` (user -> actual) avec les TTL et fenêtres de grâce convenues : `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` et `governance_grace`. Les valeurs par défaut correspondent à la politique de `docs/source/sorafs_alias_policy.md`.
   - Pour les SDK, diffuser les mêmes valeurs via leurs couches de configuration (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` dans les liaisons Rust / NAPI / Python) afin que l'application client suive la passerelle.
2. **Exécution à sec et mise en scène**
   - Déployer le changement de configuration sur un cluster de staging qui reflète la topologie de production.
   - Exécuter `cargo xtask sorafs-pin-fixtures` pour confirmer que les luminaires canoniques d'alias décodent et font toujours un aller-retour ; toute divergence implique une dérive en amont à corriger en premier.
   - Exercer les endpoints `/v1/sorafs/pin/{digest}` et `/v1/sorafs/aliases` avec des preuves synthétiques couvrant Fresh, Refresh-window, Expiré et Hard-expiré. Validez les codes HTTP, les headers (`Sora-Proof-Status`, `Retry-After`, `Warning`) et les champs du corps JSON contre ce runbook.
3. **Activer en production**
   - Déployer la nouvelle configuration pendant la fenêtre de changement standard. L'appliquer d'abord à Torii, puis redémarrer les passerelles/services SDK une fois que le nœud confirme la nouvelle politique dans les logs.
   - Importer `docs/source/grafana_sorafs_pin_registry.json` dans Grafana (ou mettre à jour les tableaux de bord existants) et épingler les panneaux de rafraîchissement du cache d'alias dans l'espace de travail NOC.
4. **Vérification post-déploiement**
   - Surveilleur `torii_sorafs_alias_cache_refresh_total` et pendentif `torii_sorafs_alias_cache_age_seconds` 30 minutes. Les images dans les courbes `error`/`expired` doivent corréler avec les fenêtres de rafraîchissement ; une croissance inattendue signifie que les opérateurs doivent inspecter les preuves d'alias et la santé des prestataires avant de continuer.
   - Confirmer que les logs côté client montrent les mêmes décisions de politique (les SDK remontent des erreurs lorsque la preuve est périmée ou expirée). L'absence d'avertissements côté client indique une mauvaise configuration.
5. **Retour**
   - Si l'émission d'alias prend du retard et que la fenêtre de rafraîchissement se déclenche fréquemment, relâcher temporairement la politique en utilisant `refresh_window` et `positive_ttl` dans la config, puis redéployer. Garder `hard_expiry` intact pour que les preuves réellement périmées soient toujours rejetées.
   - Revenir à la configuration précédente en restaurant le snapshot `iroha_config` précédent si la télémétrie continue d'afficher des comptes `error` élevée, puis ouvrir un incident pour tracer les délais de génération d'alias.

## Matériaux liés

- `docs/source/sorafs/pin_registry_plan.md` — feuille de route d'implémentation et contexte de gouvernance.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — opérations des travailleurs de stockage, complète ce registre playbook.