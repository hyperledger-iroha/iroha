---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pin-registry-ops
titre : Registre des opérations du code PIN
sidebar_label : Registre des opérations du code PIN
description : Surveillance et tri du registre Pin de SoraFS et des mesures de réplication SLA.
---

:::note Source canonique
Refleja `docs/source/sorafs/runbooks/pin_registry_ops.md`. Gardez les versions synchronisées jusqu'à ce que la documentation héritée de Sphinx soit retirée.
:::

## CV

Ce runbook documente comment surveiller et trier le registre des broches SoraFS et vos détails de niveau de service (SLA) de réplication. Les valeurs fournies par `iroha_torii` sont exportées via Prometheus vers l'espace de noms `torii_sorafs_*`. Torii affiche l'état du registre dans un intervalle de 30 secondes en seconde plan, de sorte que les tableaux de bord soient maintenus actualisés également lorsque l'opérateur est en train de consulter les points de terminaison `/v1/sorafs/pin/*`. Importez le tableau de bord curado (`docs/source/grafana_sorafs_pin_registry.json`) pour une mise en page de la liste Grafana afin d'utiliser la carte directement dans les sections suivantes.

## Référence de mesures

| Métrique | Étiquettes | Description |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventaire des manifestes en chaîne pour l'état du cycle de vie. |
| `torii_sorafs_registry_aliases_total` | — | Conteo de alias de manifestes actifs enregistrés dans le registre. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Carnet de commandes de réplication segmenté par état. |
| `torii_sorafs_replication_backlog_total` | — | Jauge de commodité qui reflète les ordres `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilité du SLA : `met` compte les ordres terminés avant la date limite, `missed` totalise les ordres tardifs + expirations, `pending` reflète les ordres pendants. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latence agrégée de finalisation (époques entre émission et achèvement). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Ventanas de holgura de órdenes pendientes (date limite menos época de emisión). |

Tous les jauges sont réinitialisées à chaque capture d'instantané, et les tableaux de bord doivent donc afficher la cadence `1m` ou la plus rapide.

## Tableau de bord de Grafana

Le JSON du tableau de bord comprend des panneaux qui couvrent les flux de travail des opérateurs. Les consultations se font en bas pour une référence rapide si vous préférez construire des graphiques à moyen terme.

1. **Ciclo de vida de manifests** – `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendance du catalogue d'alias** – `torii_sorafs_registry_aliases_total`.
3. **Cola de órdenes por estado** – `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs órdenes expiradas** – combinez `torii_sorafs_replication_backlog_total` et `torii_sorafs_registry_orders_total{status="expired"}` pour afficher la saturation.
5. **Rapport d'excellence du SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Latencia vs holgura de date limite** – superpone `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` et `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilisez les transformations de Grafana pour ajouter des vues `min_over_time` lorsque vous avez besoin du plancher absolu de support, par exemple :

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Órdenes fallidas (tasa 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Parapluies d'alerte

- **Éxito del SLA < 0,95 pour 15 min**
  - Ombrale : `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Acción : Paginación a SRE ; lancer le tri du backlog de réplication.
- **Backlog pendant jusqu'à 10**
  - Umbral : `torii_sorafs_replication_backlog_total > 10` soutenu pendant 10 min
  - Action : Réviser la disponibilité des fournisseurs et le planificateur de capacité de Torii.
- **Ordenes expiradas > 0**
  - Ombrale : `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Action : Inspecter les manifestes d'administration pour confirmer le désabonnement des fournisseurs.
- **p95 de completado > holgura promedio de date limite**
  - Ombrale : `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acción : Vérifier que les fournisseurs planifient avant la date limite ; envisager des réaffectations.

### Verre de Prometheus par exemple

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
          summary: "SLA de replicación de SoraFS por debajo del objetivo"
          description: "El ratio de éxito del SLA se mantuvo por debajo de 95% durante 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicación de SoraFS por encima del umbral"
          description: "Las órdenes pendientes excedieron el presupuesto de backlog configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Órdenes de replicación de SoraFS expiradas"
          description: "Al menos una orden de replicación expiró en los últimos cinco minutos."
```

## Flux de tri

1. **Identifier la cause**
   - Si les échecs du SLA subissent le retard dans le maintien du backlog, s'étendent au rendu des fournisseurs (échecs de PoR, terminés tardivement).
   - Si l'arriéré s'accumule avec des manquements établis, inspectez l'admission (`/v1/sorafs/pin/*`) pour confirmer les manifestes en attendant l'approbation du conseil.
2. **État de fournisseur valide**
   - Exécutez `iroha app sorafs providers list` et vérifiez que les capacités annoncées correspondent aux exigences de réplication.
   - Réviser les jauges `torii_sorafs_capacity_*` pour confirmer GiB provisionados y exito de PoR.
3. **Réplication de résignation**
   - Émettre de nouveaux ordres via `sorafs_manifest_stub capacity replication-order` lorsque l'arriéré (`stat="avg"`) est récupéré depuis 5 époques (le paquet de manifeste/CAR utilise `iroha app sorafs toolkit pack`).
   - Notifier l'administration si les alias sont concernés par les liaisons des actifs manifestes (caídas inesperadas de `torii_sorafs_registry_aliases_total`).
4. **Résultat documenté**
   - Enregistrer les notes de l'incident dans le journal des opérations de SoraFS avec les horodatages et les résumés des manifestes affectés.
   - Actualisez ce runbook si de nouveaux modes d'exploitation ou tableaux de bord apparaissent.

## Plan de despliègue

Voici cette procédure pour des étapes visant à autoriser ou à supporter la politique de cache d'alias en production :1. **Préparer la configuration**
   - Actualisation `torii.sorafs_alias_cache` et `iroha_config` (utilisateur -> réel) avec les TTL et les fenêtres de grâce associées : `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` et `governance_grace`. Les défauts coïncident avec la politique en `docs/source/sorafs_alias_policy.md`.
   - Pour les SDK, distribuez les mêmes valeurs à travers vos capacités de configuration (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` dans les liaisons Rust / NAPI / Python) pour que l'application client coïncide avec la passerelle.
2. **Exécution à sec et mise en scène**
   - Développer le changement de configuration dans un cluster de staging qui reflète la topologie de production.
   - Ejecuta `cargo xtask sorafs-pin-fixtures` pour confirmer que les appareils canoniques de l'alias sont décodifiés et vont aller-retour ; tout décalage implique une dérive de l'eau qui doit être résolue en premier.
   - Ejercita los endpoints `/v1/sorafs/pin/{digest}` et `/v1/sorafs/aliases` avec des tests synthétiques qui couvrent les cas frais, actualiser la fenêtre, expirés et dur-expirés. Validez les codes HTTP, les en-têtes (`Sora-Proof-Status`, `Retry-After`, `Warning`) et les champs du corps JSON contre ce runbook.
3. **Habiliter en production**
   - Exécutez la nouvelle configuration dans la fenêtre standard de changement. Appliquez d'abord sur Torii et puis réinitialisez le SDK de passerelles/services une fois que le nœud confirme la nouvelle politique dans les journaux.
   - Importez `docs/source/grafana_sorafs_pin_registry.json` et Grafana (pour actualiser les tableaux de bord existants) et fixez les panneaux d'actualisation du cache d'alias dans l'espace de travail du NOC.
4. **Vérification post-envoi**
   - Monitorea `torii_sorafs_alias_cache_refresh_total` et `torii_sorafs_alias_cache_age_seconds` pendant 30 minutes. Les pics dans les courbes `error`/`expired` doivent être corrélés aux fenêtres de rafraîchissement ; une croissance inattendue implique que les opérateurs doivent inspecter les pseudonymes et la santé des fournisseurs avant de continuer.
   - Confirmez que les journaux du site du client contiennent les mêmes décisions politiques (les SDK indiquent des erreurs lorsque l'essai est périmé ou expiré). L’apparition d’avertissements du client indique une mauvaise configuration.
5. **Retour**
   - Si l'émission d'alias est retracée et la fenêtre de rafraîchissement disparaît avec la fréquence, relâchez temporairement la politique augmentant `refresh_window` et `positive_ttl` dans la configuration, et vuelve à desplegar. Mantén `hard_expiry` intact pour que les essais soient vraiment rassis et rechazándose.
   - Révisez la configuration précédente en restaurant l'instantané antérieur à `iroha_config` si la télémétrie montre les contes `error` élevés, puis ouvrez un incident pour tracer des retours dans la génération d'alias.

## Matériaux liés

- `docs/source/sorafs/pin_registry_plan.md` — feuille de route de mise en œuvre et contexte de gouvernance.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — Opérations du travailleur de stockage, complémentaire à ce manuel de registre.