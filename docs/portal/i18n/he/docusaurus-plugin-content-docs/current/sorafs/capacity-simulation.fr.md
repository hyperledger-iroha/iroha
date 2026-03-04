---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: סימולציית קיבולת
כותרת: Runbook de Simation de capacité SoraFS
sidebar_label: Runbook de simulation de capacité
תיאור: תרגול ערכת סימולציה לשוק הקיבול SF-2c עם ציוד לשחזור, יצוא Prometheus ושולחן עבודה Grafana.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Gardez les deux copies Syncées jusqu'à ce que l'ensemble de documentation Sphinx hérité soit entièrement migré.
:::

זה מנהל הערה מפורשת של ערכת סימולציה לשוק הקיבול SF-2c ותוצאות חזותיות של מדדים. Il valide la négociation de quotas, la gestion du failover et la remédiation du slashing de bout en bout à l'aide des fixtures déterministes dans `docs/examples/sorafs_capacity_simulation/`. Les payloads de capacité utilisent toujours `sorafs_manifest_stub capacity`; utilisez `iroha app sorafs toolkit pack` pour les flux d'empaquetage manifest/CAR.

## 1. Générer les artefacts CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsule `sorafs_manifest_stub capacity` pour émettre des payloads Norito, des blobs base64, des corps de requête Torii וקורות חיים: JSON

- Trois declarations de fournisseurs participant au scénario de négociation de quotas.
- Un ordre de réplication allouant le manifeste en staging entre ces fournisseurs.
- תמונת מצב של télémétrie pour la ligne de base pre-panne, l'intervalle de panne et la récupération de failover.
- Un payload de litige demandant un slashing après la panne simulée.

Tous les artefacts sont déposés sous `./artifacts` (remplacez en passant un autre répertoire en premier argument). Inspectez les fichiers `_summary.json` pour un contexte lisible.

## 2. Agréger les résultats et émettre les métriques

```bash
./analyze.py --artifacts ./artifacts
```

המוצר לניתוח:

- `capacity_simulation_report.json` - הקצאות אגרות, deltas de failover et métadonnées de litige.
- `capacity_simulation.prom` - מדדי קובץ טקסט Prometheus (`sorafs_simulation_*`) מותאם לאספן קבצי טקסט ליצוא צומתים או מגרדת עבודה בלתי תלויה.

דוגמה לתצורה של גרידה Prometheus :

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

Dirigez le textfile collector vers `capacity_simulation.prom` (אני משתמש ב-Node-exporter, copiez-le dans le répertoire passé via `--collector.textfile.directory`).

## 3. היבואן של לוח המחוונים Grafana

1. Dans Grafana, יבוא `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Associez la variable de datasource `Prometheus` à la cible de scrape configurée ci-dessus.
3. Vérifiez les panneaux:
   - **הקצאת מכסות (GiB)** affiche les soldes engagés/assignés pour chaque fournisseur.
   - **טריגר תקלות** bascule sur *failover Active* lorsque les métriques de panne arrivent.
   - **ירידה בזמן הפעילות במהלך הפסקה** עקבו אחרי הפעילות של הארבעה `alpha`.
   - **אחוז החתך המבוקש** הדמיין את יחס התיקון לחוץ מתקן המשפט.

## 4. נוכחות אימות- `sorafs_simulation_quota_total_gib{scope="assigned"}` est egal à `600` tant que le total engagé reste >=600.
- `sorafs_simulation_failover_triggered` indique `1` et la métrique du fournisseur de remplacement met en avant `beta`.
- `sorafs_simulation_slash_requested` indique `0.15` (15% דה סלאש) pour l'identifiant de fournisseur `alpha`.

Exécutez `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` pour confirmer que les fixtures sont toujours acceptées par le schéma CLI.