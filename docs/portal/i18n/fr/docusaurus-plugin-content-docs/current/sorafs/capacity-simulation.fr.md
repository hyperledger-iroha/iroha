---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : simulation de capacité
titre : Runbook de simulation de capacité SoraFS
sidebar_label : Runbook de simulation de capacité
description : Exercer le kit de simulation du marché de capacité SF-2c avec des luminaires reproductibles, des exports Prometheus et des tableaux de bord Grafana.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Gardez les deux copies synchronisées jusqu'à ce que l'ensemble de la documentation Sphinx hérité soit entièrement migré.
:::

Ce runbook explique comment exécuter le kit de simulation du marché de capacité SF-2c et visualiser les métriques résultantes. Il valide la négociation de quotas, la gestion du failover et la remédiation du slashing de bout en bout à l'aide des luminaires déterministes dans `docs/examples/sorafs_capacity_simulation/`. Les charges utiles de capacité utilisent toujours `sorafs_manifest_stub capacity`; utilisez `iroha app sorafs toolkit pack` pour les flux d'empaquetage manifest/CAR.

## 1. Générer les artefacts CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsule `sorafs_manifest_stub capacity` pour émettre des payloads Norito, des blobs base64, des corps de requête Torii et des résumés JSON pour :- Trois déclarations de fournisseurs participant au scénario de négociation de quotas.
- Un ordre de réplication allouant le manifeste en mise en scène entre ces fournisseurs.
- Des instantanés de télémétrie pour la ligne de base pré‑panne, l'intervalle de panne et la récupération de basculement.
- Un payload de litige demandant un slashing après la panne simulée.

Tous les artefacts sont déposés sous `./artifacts` (remplacez en passant un autre répertoire en premier argument). Inspectez les fichiers `_summary.json` pour un contexte lisible.

## 2. Agréger les résultats et émettre les métriques

```bash
./analyze.py --artifacts ./artifacts
```

L'analyseur produit :

- `capacity_simulation_report.json` - allocations agrégées, deltas de basculement et métadonnées de litige.
- `capacity_simulation.prom` - métriques textfile Prometheus (`sorafs_simulation_*`) adapté au textfile collector de node-exporter ou à un scrape job indépendant.

Exemple de configuration de scrape Prometheus :

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

Dirigez le textfile collector vers `capacity_simulation.prom` (si vous utilisez node-exporter, copiez-le dans le répertoire passé via `--collector.textfile.directory`).

## 3. Importer le tableau de bord Grafana1. Dans Grafana, importez `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Associez la variable de datasource `Prometheus` à la cible de scrape configurée ci-dessus.
3. Vérifiez les panneaux :
   - **Quota Allocation (GiB)** affiche les ventes engagées/assignés pour chaque fournisseur.
   - **Failover Trigger** bascule sur *Failover Active* lorsque les mesures de panne arrivent.
   - **Uptime Drop Playing Outage** trace la perte en pourcentage pour le fournisseur `alpha`.
   - **Pourcentage Slash demandé** visualisez le ratio de remédiation extrait du luminaire de litige.

## 4. Vérifications attendues

- `sorafs_simulation_quota_total_gib{scope="assigned"}` est égal à `600` tant que le total engagé reste >=600.
- `sorafs_simulation_failover_triggered` indique `1` et la métrique du fournisseur de remplacement met en avant `beta`.
- `sorafs_simulation_slash_requested` indique `0.15` (15 % de slash) pour l'identifiant de fournisseur `alpha`.

Exécutez `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` pour confirmer que les luminaires sont toujours acceptés par le schéma CLI.