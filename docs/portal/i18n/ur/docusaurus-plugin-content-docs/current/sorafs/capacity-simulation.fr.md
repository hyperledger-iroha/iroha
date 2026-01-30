---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: capacity-simulation
title: Runbook de simulation de capacité SoraFS
sidebar_label: Runbook de simulation de capacité
description: Exercer le kit de simulation du marketplace de capacité SF-2c avec des fixtures reproductibles, des exports Prometheus et des tableaux de bord Grafana.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Gardez les deux copies synchronisées jusqu'à ce que l'ensemble de documentation Sphinx hérité soit entièrement migré.
:::

Ce runbook explique comment exécuter le kit de simulation du marketplace de capacité SF-2c et visualiser les métriques résultantes. Il valide la négociation de quotas, la gestion du failover et la remédiation du slashing de bout en bout à l'aide des fixtures déterministes dans `docs/examples/sorafs_capacity_simulation/`. Les payloads de capacité utilisent toujours `sorafs_manifest_stub capacity`; utilisez `iroha app sorafs toolkit pack` pour les flux d'empaquetage manifest/CAR.

## 1. Générer les artefacts CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsule `sorafs_manifest_stub capacity` pour émettre des payloads Norito, des blobs base64, des corps de requête Torii et des résumés JSON pour :

- Trois déclarations de fournisseurs participant au scénario de négociation de quotas.
- Un ordre de réplication allouant le manifeste en staging entre ces fournisseurs.
- Des snapshots de télémétrie pour la ligne de base pré‑panne, l'intervalle de panne et la récupération de failover.
- Un payload de litige demandant un slashing après la panne simulée.

Tous les artefacts sont déposés sous `./artifacts` (remplacez en passant un autre répertoire en premier argument). Inspectez les fichiers `_summary.json` pour un contexte lisible.

## 2. Agréger les résultats et émettre les métriques

```bash
./analyze.py --artifacts ./artifacts
```

L'analyseur produit :

- `capacity_simulation_report.json` - allocations agrégées, deltas de failover et métadonnées de litige.
- `capacity_simulation.prom` - métriques textfile Prometheus (`sorafs_simulation_*`) adaptées au textfile collector de node-exporter ou à un scrape job indépendant.

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

## 3. Importer le dashboard Grafana

1. Dans Grafana, importez `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Associez la variable de datasource `Prometheus` à la cible de scrape configurée ci-dessus.
3. Vérifiez les panneaux :
   - **Quota Allocation (GiB)** affiche les soldes engagés/assignés pour chaque fournisseur.
   - **Failover Trigger** bascule sur *Failover Active* lorsque les métriques de panne arrivent.
   - **Uptime Drop During Outage** trace la perte en pourcentage pour le fournisseur `alpha`.
   - **Requested Slash Percentage** visualise le ratio de remédiation extrait du fixture de litige.

## 4. Vérifications attendues

- `sorafs_simulation_quota_total_gib{scope="assigned"}` est égal à `600` tant que le total engagé reste >=600.
- `sorafs_simulation_failover_triggered` indique `1` et la métrique du fournisseur de remplacement met en avant `beta`.
- `sorafs_simulation_slash_requested` indique `0.15` (15 % de slash) pour l'identifiant de fournisseur `alpha`.

Exécutez `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` pour confirmer que les fixtures sont toujours acceptées par le schéma CLI.
