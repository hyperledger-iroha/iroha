---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pin-registry-ops
titre : Les opéras font un registre d'épingles
sidebar_label : Les opérateurs font le registre des épingles
description : Surveiller et faciliter le triage du registre des broches du SoraFS et des mesures du SLA de réplication.
---

:::note Fonte canonica
Espelha `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantenha ambas as versoes sincronizadas mangé aposentadoria da documentacao herdada do Sphinx.
:::

## Visa général

Ce runbook documente comment surveiller et faciliter le triage du registre des broches du SoraFS et ses accords de niveau de service (SLA) de réplication. Les mesures proviennent de `iroha_torii` et sont exportées via Prometheus sur l'espace de noms `torii_sorafs_*`. Torii sauvegarde l'état du registre dans un intervalle de 30 secondes par seconde plan, ce qui permet aux tableaux de bord d'être actualisés en permanence lorsque l'opérateur consulte les points de terminaison `/v1/sorafs/pin/*`. Importez le tableau de bord sélectionné (`docs/source/grafana_sorafs_pin_registry.json`) pour une mise en page de Grafana immédiatement pour l'utiliser directement comme suit.

## Référence des mesures

| Métrique | Étiquettes | Description |
| ------- | ------ | --------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventaire des manifestes en chaîne pour l'état du cycle de vie. |
| `torii_sorafs_registry_aliases_total` | - | Contagem de aliases de manifest ativos registrados no registre. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Carnet de commandes de réplication segmenté par statut. |
| `torii_sorafs_replication_backlog_total` | - | Jauge de commodité que espelha ordens `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilité du SLA : `met` avec les commandes conclues dans le délai imparti, `missed` agrége les conclusions tardives + expirées, `pending` espère les commandes pendantes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latence agrégée de conclusion (époques entre émission et conclusion). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Janelas de folga de ordens pendentes (délai menos epoca de emissao). |

Tous les jauges sont réinitialisées à chaque instantané, puis les tableaux de bord doivent être affichés avec la cadence `1m` ou plus rapidement.

## Le tableau de bord fait Grafana

Le JSON du tableau de bord comprend de nombreux détails qui couvrent les flux de travail des opérateurs. As consultas abaixo servem como referencia rapida caso voce prefira montar graficos sob medida.

1. **Ciclo de vida de manifests** - `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendance du catalogue d'alias** - `torii_sorafs_registry_aliases_total`.
3. **Fila de ordens por status** - `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs commandes expiradas** - combiner `torii_sorafs_replication_backlog_total` et `torii_sorafs_registry_orders_total{status="expired"}` pour afficher la saturation.
5. **Razao de successso de SLA** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Latence vs folga do date limite** - résumé `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` et `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilisez les transformations Grafana pour ajouter des vues `min_over_time` lorsque vous précisez la position absolue de la feuille, par exemple :

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordens perdidas (taxons 1h)** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Limites d'alerte

- **Réussite du SLA < 0,95 pendant 15 min**
  -Limiaire : `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Acao : Acionar SRE ; lancer le triage du backlog de réplication.
- **Backlog en attente d'un maximum de 10**
  - Limite : `torii_sorafs_replication_backlog_total > 10` soutenue pendant 10 min
  - À ce sujet : Vérifiez la disponibilité des fournisseurs et le planificateur de capacité du Torii.
- **Ordens expiradas > 0**
  -Limiaire : `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Acao : Inspecter les manifestes de gouvernance pour confirmer le désabonnement des fournisseurs.
- **p95 de conclusion > feuille média de date limite**
  -Limiaire : `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acao : Vérifier si les fournisseurs sont établis avant les délais ; rétributions considérées.

### Regras de Prometheus de l'exemple

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
          summary: "SLA de replicacao SoraFS abaixo da meta"
          description: "A razao de sucesso do SLA ficou abaixo de 95% por 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicacao SoraFS acima do limiar"
          description: "Ordens de replicacao pendentes excederam o budget configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordens de replicacao SoraFS expiradas"
          description: "Pelo menos uma ordem de replicacao expirou nos ultimos cinco minutos."
```

## Flux de triage

1. **Identifier la cause**
   - Si les échecs du SLA augmentent en ce qui concerne le retard permanent, il ne faut pas utiliser les fournisseurs (falhas de PoR, conclusoes tardias).
   - Pour voir l'arriéré crescer com misses estaveis, inspecione a admissao (`/v1/sorafs/pin/*`) pour confirmer les manifestes aguardando aprovacao do conselho.
2. **Statut valide des fournisseurs de dos**
   - Exécutez `iroha app sorafs providers list` et vérifiez si les capacités annoncées correspondent aux exigences de réplication.
   - Vérifiez les jauges `torii_sorafs_capacity_*` pour confirmer le GiB fourni et le succès de PoR.
3. **Retribuer la réplique**
   - Emita novas ordens via `sorafs_manifest_stub capacity replication-order` lorsque le folga do backlog (`stat="avg"`) a baissé de 5 époques (o empacotamento de manifest/CAR usa `iroha app sorafs toolkit pack`).
   - Notifier la gouvernance des alias pour les liaisons actives du manifeste (quedas inesperadas em `torii_sorafs_registry_aliases_total`).
4. **Résultat documenté**
   - Enregistrez les notes d'incident dans le journal des opérations du SoraFS avec les horodatages et les résumés des manifestes concernés.
   - Actualisez ce runbook avec les nouveaux modes de configuration ou les tableaux de bord introduits.

## Plan de déploiement

Voici cette procédure dans les étapes visant à habiliter ou à supporter la politique de cache d'alias dans la production :1. **Préparer la configuration**
   - Actualisez `torii.sorafs_alias_cache` dans `iroha_config` (utilisateur -> réel) avec les TTL et les lignes de graca associées : `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` et `governance_grace`. Les valeurs par défaut correspondent à la politique `docs/source/sorafs_alias_policy.md`.
   - Pour les SDK, distribuez vos valeurs par vos caméras de configuration (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` dans les liaisons Rust / NAPI / Python) pour que l'application du client corresponde à la passerelle.
2. **Essai de mise en scène**
   - Implanter un changement de configuration dans un cluster de staging qui correspond à la topologie de production.
   - Exécuter `cargo xtask sorafs-pin-fixtures` pour confirmer que les appareils canoniques de l'alias sont également décodifiés et fazem aller-retour ; tout décalage implique une dérive en amont qui doit être résolue en premier.
   - Exerca os endpoints `/v1/sorafs/pin/{digest}` et `/v1/sorafs/aliases` com provas sinteticas cobrindo casos Fresh, Actualiser la fenêtre, expiré et dur-expiré. Validez les codes HTTP, les en-têtes (`Sora-Proof-Status`, `Retry-After`, `Warning`) et les champs du corps JSON contre ce runbook.
3. **Habiliter la production**
   - Implanter une nouvelle configuration sur Janela Padrao de Mudancas. Appliquez d'abord sur Torii et après avoir réinitialisé le SDK des passerelles/services en aidant le nœud à confirmer la nouvelle politique de nos journaux.
   - Importez `docs/source/grafana_sorafs_pin_registry.json` dans Grafana (ou actualisez les tableaux de bord existants) et corrigez les tâches d'actualisation du cache d'alias dans aucun espace de travail du NOC.
4. **Vérification du déploiement**
   - Surveillez `torii_sorafs_alias_cache_refresh_total` et `torii_sorafs_alias_cache_age_seconds` pendant 30 minutes. Les courbes des pics nas `error`/`expired` doivent être corrélées avec les lignes de rafraîchissement ; C'est de plus en plus important que les opérateurs doivent inspecter les pseudonymes et les fournisseurs avant de continuer.
   - Confirmez que les journaux du client sont affichés lors de certaines décisions politiques (les SDK émettent des erreurs lorsque la preuve est périmée ou expirée). Ausencia des avertissements du client indique une configuration incorrecte.
5. **Retour**
   - Si l'émission d'un alias atrasar et une janvier de rafraîchissement disparaissent avec la fréquence, relâchez temporairement la politique en augmentant `refresh_window` et `positive_ttl` dans la configuration, après réimplantation. Mantenha `hard_expiry` intacte pour que cela s'avère vraiment périmé et soit rejeté.
   - Retour à la configuration antérieure du restaurant ou à l'instantané antérieur de `iroha_config` pour que la télémétrie continue à indiquer les contagieux `error` élevés, puis ouvrez un incident pour détecter des atrasos dans la génération d'alias.

## Matériel lié

- `docs/source/sorafs/pin_registry_plan.md` - feuille de route de mise en œuvre et contexte de gouvernance.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - les opérations du travailleur de stockage, complètent ce playbook du registre.