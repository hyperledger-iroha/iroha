---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : simulation de capacité
titre : Runbook de simulation de capacité du SoraFS
sidebar_label : Runbook de simulation de capacité
description : Exécuter la boîte à outils de simulation du marché de capacité SF-2c avec reproduction de luminaires, exportations vers Prometheus et tableaux de bord vers Grafana.
---

:::note Fonte canônica
Cette page s'affiche `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantenha ambas comme copies synchronisées.
:::

Ce runbook explique comment exécuter le kit de simulation du marché de capacité SF-2c et visualiser les résultats métriques. Il valide la négociation des coûts, le traitement du basculement et la correction des coupures de pont à pont en utilisant les luminaires déterminants sur `docs/examples/sorafs_capacity_simulation/`. Les charges utiles de capacité sont également utilisées par `sorafs_manifest_stub capacity` ; utilisez `iroha app sorafs toolkit pack` pour les flux d'emballage de manifeste/CAR.

## 1. Gerar artefatos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsule `sorafs_manifest_stub capacity` pour les charges utiles émettrices Norito, blobs base64, corps de demande pour Torii et résumés JSON pour :

- Trois déclarations des fournisseurs qui participent au scénario de négociation de contrats.
- Un ordre de réplication qui place le manifeste dans la mise en scène entre ces fournisseurs.
- Instantanés de télémétrie pour la ligne de base avant l'échec, l'intervalle d'échec et la récupération de basculement.
- Une charge utile de litige sollicitant une coupure après une fausse simulation.Tous les articles sont destinés à `./artifacts` (en passant par un répertoire différent comme premier argument). Inspectez les archives `_summary.json` dans un contexte légal.

## 2. Regrouper les résultats et émettre des valeurs

```bash
./analyze.py --artifacts ./artifacts
```

L'analyseur produit :

- `capacity_simulation_report.json` - allocations groupées, deltas de basculement et métadonnées de litige.
- `capacity_simulation.prom` - les paramètres de fichier texte de Prometheus (`sorafs_simulation_*`) sont adéquats pour le collecteur de fichiers texte de l'exportateur de nœuds ou d'un travail de scrape indépendant.

Exemple de configuration de scrape pour Prometheus :

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

Connectez-vous au collecteur de fichiers texte pour `capacity_simulation.prom` (et utilisez node-exporter, copiez pour le répertoire passé via `--collector.textfile.directory`).

## 3. Importer le tableau de bord via Grafana

1. No Grafana, importez `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Sélectionnez une variable de la source de données `Prometheus` et grattez la cible configurée clairement.
3. Vérifiez les painéis :
   - **Allocation de quotas (GiB)** montre les soldes compromis/atribuídos de chaque fournisseur.
   - **Failover Trigger** est adapté à *Failover Active* lorsque vous avez des erreurs à effectuer.
   - **Chute de disponibilité pendant une panne** représente un pourcentage de perte pour la preuve `alpha`.
   - **Pourcentage Slash demandé** pour visualiser la résolution du problème extrayée lors du litige.

## 4. Vérifications attendues- `sorafs_simulation_quota_total_gib{scope="assigned"}` équivaut à `600` en quantité ou durée totale du compromis >=600.
- `sorafs_simulation_failover_triggered` rapporte `1` et la métrique de la preuve ou remplace la `beta`.
- `sorafs_simulation_slash_requested` rapporte `0.15` (15 % de barre oblique) pour l'identifiant du fournisseur `alpha`.

Exécutez `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` pour confirmer que les appareils sont bien réglés sur l'image de la CLI.