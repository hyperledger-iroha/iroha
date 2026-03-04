---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: סימולציית קיבולת
כותרת: Runbook de simulación de capacidad de SoraFS
sidebar_label: Runbook de simulación de capacidad
תיאור: ערכת כלים של סימולציה של השוק SF-2c עם רכיבי שחזור, יצוא של Prometheus ודשבורדים של Grafana.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantén ambas copias sincronizadas hasta que el conjunto de documentación heredada en Sphinx se haya migrado por completo.
:::

ראה ספר הפעלה הסבר על הוצאת ערכת סימולציה של השוק SF-2c עם תוצאות חזותיות. Valida la negociación de cuotas, el manejo de failover y la remediación de slashing de extremo a extremo usando los fixtures deterministas en `docs/examples/sorafs_capacity_simulation/`. Los loads de capacidad aún usan `sorafs_manifest_stub capacity`; ארה"ב `iroha app sorafs toolkit pack` עבור los flujos de empaquetado de manifest/CAR.

## 1. חפצי אמנות כלליים של CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` envuelve `sorafs_manifest_stub capacity` עבור מטענים emitir Norito, blobs base64, cuerpos de solicitud para Torii y resúmenes JSON para:

- Tres declaraciones de proveedores que participan en el escenario de negociación de cuotas.
- Una orden de replicación que asigna el manifiesto in staging entre esos proveedores.
- תמונות Snapshots de telemetria para el baseline previo a la caída, el intervalo de caída y la recuperación por failover.
- Un payload de disputa solicitando slashing tras la caída simulada.

Todos los artefactos se escriben bajo `./artifacts` (puedes reemplazarlo pasando un directorio diferente como primer argumento). בדיקת ארכיון `_summary.json` לקריאה בהקשר.

## 2. Agregar resultados y emitir métricas

```bash
./analyze.py --artifacts ./artifacts
```

תוצרת El analizador:

- `capacity_simulation_report.json` - asignaciones agregadas, deltas de failover y metadatos de disputa.
- `capacity_simulation.prom` - מדדי טקסט של Prometheus (`sorafs_simulation_*`) adecuadas עבור אספן קבצי טקסט של צומת-יצואן או עבודה עצמאית לגרד.

דוגמה לתצורה של גרידה של Prometheus:

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

אוסף קבצי טקסט ב-`capacity_simulation.prom` (בארצות הברית ליצוא צמתים, מרכז מידע על `--collector.textfile.directory`).

## 3. ייבוא לוח המחוונים של Grafana

1. En Grafana, importa `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincula la variable del datasource `Prometheus` ל-Scrape target configurado arriba.
3. Verifica los paneles:
   - **הקצאת מכסות (GiB)** muestra los balances comprometidos/asignados de cada proveedor.
   - **טריגר כושל** כולל * פעיל בכשלים* cuando entran las métricas de caída.
   - **ירידה בזמן פעילות בזמן הפסקה** גרפיקה להצגה פורציונלית עבור אל הוכחה `alpha`.
   - **אחוז החתך המבוקש** מראה את הפרופורציה של התיקון האקסטרה של מתקן המחלוקת.

## 4. Comprobaciones esperadas- `sorafs_simulation_quota_total_gib{scope="assigned"}` שווה ערך ל-`600` מיינטרס אל סך הקומפרומטידו של המניין >=600.
- `sorafs_simulation_failover_triggered` reporta `1` y la métrica del proveedor de reemplazo resalta `beta`.
- `sorafs_simulation_slash_requested` reporta `0.15` (15% דה נטוי) עבור אל זיהוי מוכיח `alpha`.

Ejecuta `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que los fixtures siguen siendo aceptados por el esquema de la CLI.