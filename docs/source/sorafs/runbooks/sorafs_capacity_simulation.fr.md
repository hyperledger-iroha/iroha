---
lang: fr
direction: ltr
source: docs/source/sorafs/runbooks/sorafs_capacity_simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a74e1cb5abc86822ff9d24b9ce42a6567d964cbc01ca4c619b49ca6d239101da
source_last_modified: "2025-11-05T18:02:08.787799+00:00"
translation_last_reviewed: 2026-01-30
---

# Runbook de simulation de capacité SoraFS

Ce runbook explique comment exercer le kit de simulation du marketplace de capacité SF-2c et visualiser les métriques résultantes. L'objectif est de valider la négociation de quotas, la gestion du failover et la remédiation du slashing de bout en bout à l'aide des fixtures reproductibles dans `docs/examples/sorafs_capacity_simulation/`. Les payloads de capacité utilisent toujours `sorafs_manifest_stub capacity`; utilisez `iroha app sorafs toolkit pack` pour les flux d'empaquetage manifest/CAR.

## 1. Générer les artefacts CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

Le script invoque `sorafs_manifest_stub capacity` pour émettre des payloads Norito déterministes, des encodages base64, des corps de requête Torii et des résumés JSON pour :

- Trois déclarations de fournisseurs participant au scénario de négociation de quotas.
- Un ordre de réplication allouant le manifeste en staging entre les fournisseurs.
- Des snapshots de télémétrie capturant la ligne de base pré‑panne, la fenêtre de panne et la récupération de failover.
- Un payload de litige demandant un slashing après la panne simulée.

Les artefacts sont écrits dans `./artifacts` (ou le chemin fourni comme premier argument). Inspectez les fichiers `_summary.json` pour un état lisible.

## 2. Agréger les résultats et émettre les métriques

```bash
./analyze.py --artifacts ./artifacts
```

Le script d'analyse produit :

- `capacity_simulation_report.json` — allocations agrégées, deltas de failover et métadonnées de litige.
- `capacity_simulation.prom` — métriques textfile Prometheus (`sorafs_simulation_*`) adaptées à l'import via le textfile collector de node-exporter ou un scrape job Prometheus autonome.

Exemple de configuration de scrape `prometheus.yml` :

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

Dirigez le textfile collector vers le `.prom` généré (pour node-exporter, copiez-le dans le `--collector.textfile.directory` configuré).

## 3. Importer le dashboard Grafana

1. Dans Grafana, importez `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Associez l'entrée de datasource `Prometheus` à la configuration de scrape ci-dessus.
3. Vérifiez les panneaux :
   - **Quota Allocation (GiB)** affiche le solde engagé/assigné pour chaque fournisseur.
   - **Failover Trigger** bascule sur *Failover Active* lorsque les métriques de panne sont chargées.
   - **Uptime Drop During Outage** trace la perte en pourcentage du fournisseur `alpha`.
   - **Requested Slash Percentage** visualise le ratio de remédiation extrait du fixture de litige.

## 4. Vérifications attendues

- `sorafs_simulation_quota_total_gib{scope="assigned"}` est égal à 600 tant que le total engagé reste ≥600.
- `sorafs_simulation_failover_triggered` indique `1` et la métrique du fournisseur de remplacement met en avant `beta`.
- `sorafs_simulation_slash_requested` indique `0.15` (15 % de slash) pour l'identifiant de fournisseur `alpha`.

Exécutez `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` pour confirmer que les fixtures valident toujours avec le schéma CLI.
