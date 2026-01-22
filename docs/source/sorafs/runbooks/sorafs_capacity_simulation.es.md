<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/sorafs/runbooks/sorafs_capacity_simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a74e1cb5abc86822ff9d24b9ce42a6567d964cbc01ca4c619b49ca6d239101da
source_last_modified: "2025-11-05T18:02:08.787799+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de simulación de capacidad de SoraFS

Este runbook explica cómo ejercitar el toolkit de simulación del marketplace de capacidad SF-2c y visualizar las métricas resultantes. El objetivo es validar la negociación de cuotas, el manejo de failover y la remediación de slashing de extremo a extremo usando los fixtures reproducibles en `docs/examples/sorafs_capacity_simulation/`. Los payloads de capacidad aún usan `sorafs_manifest_stub capacity`; usa `iroha app sorafs toolkit pack` para los flujos de empaquetado de manifest/CAR.

## 1. Generar artefactos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

El script invoca `sorafs_manifest_stub capacity` para emitir payloads Norito deterministas, codificaciones base64, cuerpos de solicitud para Torii y resúmenes JSON para:

- Tres declaraciones de proveedores que participan en el escenario de negociación de cuotas.
- Una orden de replicación que asigna el manifiesto en staging entre los proveedores.
- Snapshots de telemetría que capturan el baseline previo a la caída, la ventana de caída y la recuperación por failover.
- Un payload de disputa solicitando slashing tras la caída simulada.

Los artefactos se escriben en `./artifacts` (o en la ruta suministrada como primer argumento). Inspecciona los archivos `_summary.json` para estado legible.

## 2. Agregar resultados y emitir métricas

```bash
./analyze.py --artifacts ./artifacts
```

El script de análisis produce:

- `capacity_simulation_report.json` — asignaciones agregadas, deltas de failover y metadatos de disputa.
- `capacity_simulation.prom` — métricas de textfile de Prometheus (`sorafs_simulation_*`) adecuadas para importar vía el textfile collector de node-exporter o un scrape job independiente de Prometheus.

Ejemplo de configuración de scrape `prometheus.yml`:

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

Apunta el textfile collector al `.prom` generado (para node-exporter, cópialo al `--collector.textfile.directory` configurado).

## 3. Importar dashboard de Grafana

1. En Grafana, importa `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincula la entrada del datasource `Prometheus` a la configuración de scrape anterior.
3. Verifica los paneles:
   - **Quota Allocation (GiB)** muestra el balance comprometido/asignado de cada proveedor.
   - **Failover Trigger** cambia a *Failover Active* cuando se cargan las métricas de caída.
   - **Uptime Drop During Outage** grafica la pérdida porcentual del proveedor `alpha`.
   - **Requested Slash Percentage** visualiza la proporción de remediación extraída del fixture de disputa.

## 4. Comprobaciones esperadas

- `sorafs_simulation_quota_total_gib{scope="assigned"}` equivale a 600 mientras el total comprometido se mantiene ≥600.
- `sorafs_simulation_failover_triggered` reporta `1` y la métrica del proveedor de reemplazo resalta `beta`.
- `sorafs_simulation_slash_requested` reporta `0.15` (15% de slash) para el identificador de proveedor `alpha`.

Ejecuta `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que los fixtures siguen validando con el esquema de la CLI.
