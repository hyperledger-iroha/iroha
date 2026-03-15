---
lang: fr
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T15:38:30.656337+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: pin-registry-ops-fr
slug: /sorafs/pin-registry-ops-fr
---

:::note Source canonique
Miroirs `docs/source/sorafs/runbooks/pin_registry_ops.md`. Gardez les deux versions alignées dans toutes les versions.
:::

## Aperçu

Ce runbook explique comment surveiller et trier le registre de broches SoraFS et ses accords de niveau de service (SLA) de réplication. Les métriques proviennent de `iroha_torii` et sont exportées via Prometheus sous l'espace de noms `torii_sorafs_*`. Torii échantillonne l'état du registre toutes les 30 secondes en arrière-plan, de sorte que les tableaux de bord restent à jour même lorsqu'aucun opérateur n'interroge les points de terminaison `/v1/sorafs/pin/*`. Importez le tableau de bord organisé (`docs/source/grafana_sorafs_pin_registry.json`) pour une mise en page Grafana prête à l'emploi qui correspond directement aux sections ci-dessous.

## Référence métrique

| Métrique | Étiquettes | Descriptif |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventaire des manifestes en chaîne par état du cycle de vie. |
| `torii_sorafs_registry_aliases_total` | — | Nombre d'alias de manifeste actifs enregistrés dans le registre. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Carnet de commandes de réplication segmenté par statut. |
| `torii_sorafs_replication_backlog_total` | — | Jauge de commodité reflétant les commandes `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Comptabilité SLA : `met` compte les commandes terminées dans les délais, `missed` regroupe les achèvements en retard + les expirations, `pending` reflète les commandes en cours. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latence d'achèvement agrégée (époques entre l'émission et l'achèvement). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Fenêtres de commande en attente (délai moins époque d'émission). |

Toutes les jauges sont réinitialisées à chaque extraction d'instantané, les tableaux de bord doivent donc échantillonner à la cadence `1m` ou plus rapidement.

## Grafana Tableau de bord

Le tableau de bord JSON est livré avec sept panneaux qui couvrent les flux de travail des opérateurs. Les requêtes sont répertoriées ci-dessous pour une référence rapide si vous préférez créer des graphiques sur mesure.

1. **Cycle de vie du manifeste** – `torii_sorafs_registry_manifests_total` (regroupé par `status`).
2. **Tendance du catalogue d'alias** – `torii_sorafs_registry_aliases_total`.
3. **File d'attente de commande par statut** – `torii_sorafs_registry_orders_total` (regroupée par `status`).
4. **Backlog vs commandes expirées** – combine `torii_sorafs_replication_backlog_total` et `torii_sorafs_registry_orders_total{status="expired"}` à la saturation de la surface.
5. **Taux de réussite des SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latence par rapport aux délais** – superposition `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` et `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilisez les transformations Grafana pour ajouter des vues `min_over_time` lorsque vous avez besoin du plancher de marge absolue, par exemple :

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Commandes manquées (tarif 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Seuils d'alerte- **Succès SLA  0**
  - Seuil : `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Action : Inspectez les manifestes de gouvernance pour confirmer le taux de désabonnement des fournisseurs.
- **Achèvement p95 > délai moyen slack**
  - Seuil : `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Action : Vérifier que les prestataires s'engagent avant les délais ; envisager de procéder à des réaffectations.

### Exemple de règles Prometheus

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
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Flux de travail de tri

1. **Identifier la cause**
   - Si le SLA manque le pic alors que le backlog reste faible, concentrez-vous sur les performances du fournisseur (échecs PoR, achèvements tardifs).
   - Si l'arriéré augmente avec des échecs stables, inspectez l'admission (`/v1/sorafs/pin/*`) pour confirmer les manifestes en attente d'approbation par le conseil.
2. **Valider le statut du fournisseur**
   - Exécutez `iroha app sorafs providers list` et vérifiez que les fonctionnalités annoncées correspondent aux exigences de réplication.
   - Vérifiez les jauges `torii_sorafs_capacity_*` pour confirmer le succès du GiB et du PoR provisionnés.
3. **Réaffecter la réplication**
   - Émettre de nouvelles commandes via `sorafs_manifest_stub capacity replication-order` lorsque le retard du carnet de commandes (`stat="avg"`) tombe en dessous de 5 époques (l'emballage manifeste/CAR utilise `iroha app sorafs toolkit pack`).
   - Avertir la gouvernance si les alias manquent de liaisons de manifeste actives (`torii_sorafs_registry_aliases_total` tombe de manière inattendue).
4. **Résultat du document**
   - Enregistrez les notes d'incident dans le journal des opérations SoraFS avec des horodatages et des résumés de manifeste concernés.
   - Mettez à jour ce runbook si de nouveaux modes de défaillance ou tableaux de bord sont introduits.

## Plan de déploiement

Suivez cette procédure par étapes lors de l'activation ou du renforcement de la stratégie de cache d'alias en production :1. **Préparer la configuration**
   - Mettre à jour `torii.sorafs_alias_cache` dans `iroha_config` (utilisateur → réel) avec les TTL et fenêtres de grâce convenues : `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` et `governance_grace`. Les valeurs par défaut correspondent à la stratégie `docs/source/sorafs_alias_policy.md`.
   - Pour les SDK, distribuez les mêmes valeurs via leurs couches de configuration (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` dans les liaisons Rust/NAPI/Python) afin que l'application du client corresponde à la passerelle.
2. **Essai à sec en préparation**
   - Déployez la modification de configuration sur un cluster intermédiaire qui reflète la topologie de production.
   - Exécutez `cargo xtask sorafs-pin-fixtures` pour confirmer que les appareils d'alias canoniques sont toujours décodés et aller-retour ; tout décalage implique une dérive manifeste en amont qui doit être corrigée en premier.
   - Exercez les points de terminaison `/v1/sorafs/pin/{digest}` et `/v1/sorafs/aliases` avec des preuves synthétiques couvrant les cas récents, à fenêtre d'actualisation, expirés et définitivement expirés. Validez les codes d'état HTTP, les en-têtes (`Sora-Proof-Status`, `Retry-After`, `Warning`) et les champs de corps JSON par rapport à ce runbook.
3. **Activer en production**
   - Déployer la nouvelle configuration via la fenêtre de changement standard. Appliquez-le d'abord à Torii, puis redémarrez les passerelles/services SDK une fois que le nœud confirme la nouvelle stratégie dans les journaux.
   - Importez `docs/source/grafana_sorafs_pin_registry.json` dans Grafana (ou mettez à jour les tableaux de bord existants) et épinglez les panneaux d'actualisation du cache d'alias à l'espace de travail NOC.
4. **Vérification post-déploiement**
   - Surveillez `torii_sorafs_alias_cache_refresh_total` et `torii_sorafs_alias_cache_age_seconds` pendant 30 minutes. Les pics dans les courbes `error`/`expired` devraient être en corrélation avec les fenêtres d'actualisation des politiques ; une croissance inattendue signifie que les opérateurs doivent inspecter les preuves d’alias et l’état des fournisseurs avant de continuer.
   - Confirmez que les journaux côté client affichent les mêmes décisions politiques (les SDK feront apparaître des erreurs lorsque la preuve est périmée ou expirée). L'absence d'avertissements client indique une mauvaise configuration.
5. **Retour**
   - Si l'émission d'alias prend du retard et que la fenêtre d'actualisation se déclenche fréquemment, assouplissez temporairement la politique en augmentant `refresh_window` et `positive_ttl` dans la configuration, puis redéployez. Conservez `hard_expiry` intact afin que les épreuves véritablement périmées soient toujours rejetées.
   - Revenez à la configuration précédente en restaurant l'instantané `iroha_config` précédent si la télémétrie continue d'afficher des nombres `error` élevés, puis ouvrez un incident pour suivre les retards de génération d'alias.

## Documents connexes

- `docs/source/sorafs/pin_registry_plan.md` — feuille de route de mise en œuvre et contexte de gouvernance.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — opérations de stockage, complète ce playbook de registre.
