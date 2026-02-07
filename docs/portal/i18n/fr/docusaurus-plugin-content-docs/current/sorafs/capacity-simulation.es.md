---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : simulation de capacité
titre : Runbook de simulation de capacité de SoraFS
sidebar_label : Runbook de simulation de capacité
description : Exécuter la boîte à outils de simulation du marché de capacité SF-2c avec des luminaires reproductibles, des exportations de Prometheus et des tableaux de bord de Grafana.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Il s'agit de copies synchronisées jusqu'à ce que le ensemble de documents hérités de Sphinx ait été migré complètement.
:::

Ce runbook explique comment exécuter le kit de simulation du marché de capacité SF-2c et visualiser les valeurs résultantes. Validez la négociation de comptes, la procédure de basculement et la correction des coupures de l'extrême à l'extrême en utilisant les luminaires déterminés en `docs/examples/sorafs_capacity_simulation/`. Les charges utiles de capacité utilisent `sorafs_manifest_stub capacity` ; usa `iroha app sorafs toolkit pack` pour les flujos de empaquetado de manifest/CAR.

## 1. Générer des artefacts de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` envoie `sorafs_manifest_stub capacity` pour émettre des charges utiles Norito, des blobs base64, des services de sollicitation pour Torii et des résumés JSON pour :- Trois déclarations des fournisseurs qui participent au scénario de négociation de comptes.
- Un ordre de réplication qui assigne le manifeste à la mise en scène entre ces fournisseurs.
- Instantanés de télémétrie pour la ligne de base précédente au démarrage, l'intervalle de démarrage et la récupération lors du basculement.
- Une charge utile de litige sollicitée pour couper la caisse simulée.

Tous les objets sont écrits sous `./artifacts` (vous pouvez remplacer un répertoire différent comme argument principal). Inspecciona los archivos `_summary.json` para contexto lisible.

## 2. Regrouper les résultats et émettre des métriques

```bash
./analyze.py --artifacts ./artifacts
```

L'analyseur produit :

- `capacity_simulation_report.json` - attributions groupées, deltas de basculement et métadonnées de litige.
- `capacity_simulation.prom` - Métriques de fichier texte de Prometheus (`sorafs_simulation_*`) adaptées au collecteur de fichiers texte de l'exportateur de nœuds ou à un travail de scrape indépendant.

Exemple de configuration de scrape de Prometheus :

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

Connectez le collecteur de fichiers texte à `capacity_simulation.prom` (si vous utilisez un exportateur de nœuds, copiez le répertoire passé via `--collector.textfile.directory`).

## 3. Importer le tableau de bord de Grafana1. En Grafana, importez `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Sélectionnez la variable de la source de données `Prometheus` à la cible de scrape configurée à l'arrivée.
3. Vérifiez les panneaux :
   - **Allocation de quotas (GiB)** pour les soldes compromis/asignados de chaque fournisseur.
   - **Failover Trigger** change avec *Failover Active* lorsque vous entrez dans les paramètres de caisse.
   - **Chute de disponibilité pendant une panne** graphique de la perte potentielle pour le fournisseur `alpha`.
   - **Pourcentage Slash demandé** Visualisez la proportion de réparation extrayée du luminaire en litige.

## 4. Comprobaciones espéradas

- `sorafs_simulation_quota_total_gib{scope="assigned"}` équivaut à `600` lorsque le compromis total se maintient >=600.
- `sorafs_simulation_failover_triggered` reporta `1` et la métrique du fournisseur de remplacement resalta `beta`.
- `sorafs_simulation_slash_requested` rapporte `0.15` (15 % de barre oblique) pour l'identifiant du fournisseur `alpha`.

Éjectez `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` pour confirmer que les appareils sont toujours acceptés par le schéma de la CLI.