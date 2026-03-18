---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pin-registry-ops
titre : Registre des épingles
sidebar_label : Registre des épingles
description : SoraFS Pin Registry et réplication SLA
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانی Sphinx دستاویزات ریٹائر نہ ہوں دونوں ورژنز کو ہم آہنگ رکھیں۔
:::

## جائزہ

Le runbook est un gestionnaire de registre SoraFS et un registre de broches et une réplication pour un service de sauvegarde (SLA) ٹرائج کیسے کیا جائے۔ Il s'agit d'un espace de noms `iroha_torii` et d'un espace de noms Prometheus. ہیں۔ Torii Il s'agit d'un registre de 30 ans d'âge. Les points de terminaison `/v1/sorafs/pin/*` sont également disponibles pour les points de terminaison `/v1/sorafs/pin/*`. تیار شدہ ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) امپورٹ کریں تاکہ Grafana کا ایک تیار layout ملے جو نیچے کے حصوں سے براہ راست میپ ہوتا ہے۔

## میٹرک حوالہ

| میٹرک | Étiquettes | وضاحت |
| ----- | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | آن چین manifeste کا انوینٹری لائف سائیکل اسٹیٹ کے مطابق۔ |
| `torii_sorafs_registry_aliases_total` | — | registre میں ریکارڈ شدہ فعال manifeste alias کی تعداد۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | commandes de réplication et arriérés de commandes |
| `torii_sorafs_replication_backlog_total` | — | Jauge de jauge et commandes `pending` pour la jauge de pression |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA اکاؤنٹنگ: `met` et commandes en ligne et commandes en ligne `missed` pour les commandes + expirations جمع کرتا ہے، `pending` زیر التواء commandes کو ظاہر کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | latence d'achèvement مجموعی طور پر (émission et achèvement کے درمیان époques)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | commandes en attente et fenêtres lentes (délai moins époque d'émission) |

Les jauges d'instantané sont prises en charge par l'échantillon `1m` pour la cadence et l'échantillon کرنا چاہیے۔

## Grafana maintenant

Un code JSON est disponible pour les utilisateurs en ligne et en ligne. اگر آپ اپنی مرضی کے چارٹس بنانا چاہیں تو نیچے سوالات بطور فوری حوالہ درج ہیں۔

1. **Cycle de vie du manifeste** – `torii_sorafs_registry_manifests_total` (groupe `status` کے مطابق).
2. **Tendance du catalogue d'alias** – `torii_sorafs_registry_aliases_total`.
3. **File d'attente de commande par statut** – `torii_sorafs_registry_orders_total` (groupe `status` کے مطابق).
4. **Carnet de commandes par rapport aux commandes expirées** – `torii_sorafs_replication_backlog_total` et `torii_sorafs_registry_orders_total{status="expired"}` pour une saturation optimale.
5. **Taux de réussite des SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latence par rapport au délai de relâchement** – `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` et `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}` en anglais Il s'agit d'un plancher mou absolu et de transformations Grafana et de vues `min_over_time`.

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Commandes manquées (tarif 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Seuils d'alerte- **Succès SLA < 0,95 pendant 15 min**
  - Seuil : `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Action : SRE est disponible triage du backlog de réplication
- **arriéré en attente supérieur à 10**
  - Seuil : `torii_sorafs_replication_backlog_total > 10` 10 منٹ تک برقرار
  - Action : les fournisseurs utilisent le planificateur de capacité Torii.
- **Commandes expirées > 0**
  - Seuil : `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Action : la gouvernance se manifeste par le désabonnement des fournisseurs.
- **Achèvement p95 > délai moyen slack**
  - Seuil : `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Action : respecter les délais des fournisseurs et s'engager dans les délais requis réaffectations پر غور کریں۔

### مثال Prometheus قواعد

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
          summary: "SoraFS replication SLA ہدف سے کم"
          description: "SLA کامیابی کا تناسب 15 منٹ تک 95% سے کم رہا۔"

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog حد سے اوپر"
          description: "زیر التواء replication orders مقررہ backlog بجٹ سے بڑھ گئے۔"

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders ختم ہو گئیں"
          description: "پچھلے پانچ منٹ میں کم از کم ایک replication order ختم ہوئی۔"
```

## Workflow de tri

1. **وجہ کی شناخت**
   - Les SLA manquent le backlog des fournisseurs et les fournisseurs (échecs du PoR, achèvements tardifs)
   - L'arriéré des demandes d'admission est manqué (`/v1/sorafs/pin/*`) et les manifestes sont affichés. منتظر ہیں واضح ہوں۔
2. **Fournisseurs de fournisseurs de services**
   - `iroha app sorafs providers list` La réplication est une solution de réplication de base de données.
   - Les jauges `torii_sorafs_capacity_*` permettent d'évaluer le succès du GiB et du PoR provisionné.
3. **Réplication et réaffectation**
   - Marge du carnet de commandes (`stat="avg"`) 5 époques pour les commandes de livraison et `sorafs_manifest_stub capacity replication-order` pour les commandes de livraison (manifeste/emballage de voiture) `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - Les alias pour les liaisons manifestes et la gouvernance pour les liens (`torii_sorafs_registry_aliases_total` میں غیر متوقع کمی)۔
4. **نتیجہ دستاویزی بنائیں**
   - Journal des opérations SoraFS avec horodatages et résumés de manifeste et notes d'incidents
   - Les modes de défaillance et les tableaux de bord ainsi que le runbook et les tableaux de bord

## Plan de déploiement

La politique de cache d'alias est également une question de sécurité et de confidentialité :1. ** Configuration de la configuration **
   - `iroha_config` et `torii.sorafs_alias_cache` (utilisateur -> réel) et les TTL et les fenêtres de grâce sont disponibles : `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`۔ valeurs par défaut `docs/source/sorafs_alias_policy.md` est disponible en ligne
   - SDK pour créer des couches de configuration et des couches de configuration (liaisons `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` Rust / NAPI / Python ici) pour la passerelle d'application client. کھائے۔
2. **Mise en scène d'un essai à sec**
   - configuration de la mise en scène et du déploiement pour la topologie de production
   - `cargo xtask sorafs-pin-fixtures` prend en charge les appareils d'alias canoniques pour décoder et aller-retour Inadéquation de la dérive en amont
   - Points de terminaison `/v1/sorafs/pin/{digest}` et `/v1/sorafs/aliases` pour preuves synthétiques et preuves fraîches, fenêtre d'actualisation expirée et dure-expirée. Codes d'état HTTP, en-têtes (`Sora-Proof-Status`, `Retry-After`, `Warning`) et champs de corps JSON et runbook pour valider les fichiers.
3. **Production میں فعال کریں**
   - fenêtre de changement standard pour la configuration de la configuration Torii est un nœud de connexion pour les passerelles/services SDK et redémarrage کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` et Grafana pour les panneaux d'actualisation du cache (pour les tableaux de bord et les broches) et les panneaux d'actualisation du cache d'alias et la broche de l'espace de travail NOC کریں۔
4. **Vérification post-déploiement**
   - 30 minutes de `torii_sorafs_alias_cache_refresh_total` à `torii_sorafs_alias_cache_age_seconds` pour les achats en ligne Les courbes `error`/`expired` et les pointes et les fenêtres d'actualisation sont en corrélation avec les valeurs Il existe des opérateurs et des preuves d'alias et des fournisseurs et des fournisseurs de services.
   - Journaux côté client et décisions politiques en cours (SDK obsolètes et preuves expirées et erreurs en cours) avertissements client en cours de configuration et en cours de configuration
5. **Retour**
   - L'émission d'alias est terminée et la fenêtre d'actualisation est activée pour `refresh_window` et `positive_ttl`. عارضی طور پر نرم کریں، پھر redéploiement کریں۔ `hard_expiry` sont des preuves périmées et des preuves périmées.
   - La télémétrie `error` compte pour la configuration et la configuration `iroha_config` instantané pour la configuration et la configuration. Les retards de génération d'alias et les incidents liés aux incidents

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` — feuille de route de mise en œuvre et contexte de gouvernance
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — Opérations du travailleur de stockage, un playbook de registre et un manuel de stockage