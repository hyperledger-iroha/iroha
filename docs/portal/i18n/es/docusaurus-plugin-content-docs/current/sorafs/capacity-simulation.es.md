---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulación de capacidad
título: Runbook de simulación de capacidad de SoraFS
sidebar_label: Runbook de simulación de capacidad
descripción: Ejecutar el kit de herramientas de simulación del mercado de capacidad SF-2c con accesorios reproducibles, exportaciones de Prometheus y paneles de control de Grafana.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantén ambas copias sincronizadas hasta que el conjunto de documentación heredada en Sphinx se haya migrado por completo.
:::

Este runbook explica cómo ejecutar el kit de simulación del mercado de capacidad SF-2c y visualizar las métricas resultantes. Valida la negociación de cuotas, el manejo de failover y la remediación de slashing de extremo a extremo usando los accesorios deterministas en `docs/examples/sorafs_capacity_simulation/`. Los payloads de capacidad aún usan `sorafs_manifest_stub capacity`; usa `iroha app sorafs toolkit pack` para los flujos de embalaje de manifiesto/CAR.

## 1. Generar artefactos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` envuelve `sorafs_manifest_stub capacity` para emitir payloads Norito, blobs base64, cuerpos de solicitud para Torii y resúmenes JSON para:- Tres declaraciones de proveedores que participan en el escenario de negociación de cuotas.
- Una orden de replicación que asigna el manifiesto en puesta en escena entre esos proveedores.
- Instantáneas de telemetría para la línea base previa a la caída, el intervalo de caída y la recuperación por conmutación por error.
- Un payload de disputa solicitando slashing tras la caída simulada.

Todos los artefactos se escriben bajo `./artifacts` (puedes reemplazarlo pasando un directorio diferente como primer argumento). Inspecciona los archivos `_summary.json` para contexto legible.

## 2. Agregar resultados y emitir métricas

```bash
./analyze.py --artifacts ./artifacts
```

El analizador produce:

- `capacity_simulation_report.json` - asignaciones agregadas, deltas de failover y metadatos de disputa.
- `capacity_simulation.prom` - métricas de textfile de Prometheus (`sorafs_simulation_*`) adecuadas para el recopilador de archivos de texto de node-exporter o un scrape job independiente.

Ejemplo de configuración de scrape de Prometheus:

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

Apunta el recopilador de archivos de texto a `capacity_simulation.prom` (si usas node-exporter, cópialo al directorio pasado vía `--collector.textfile.directory`).

## 3. Importar el tablero de Grafana1. En Grafana, importa `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincula la variable del datasource `Prometheus` al scrape target configurado arriba.
3. Verifica los paneles:
   - **Quota Allocation (GiB)** muestra los saldos comprometidos/asignados de cada proveedor.
   - **Failover Trigger** cambia a *Failover Active* cuando entran las métricas de caída.
   - **Caída del tiempo de actividad durante la interrupción** gráfica la pérdida porcentual para el proveedor `alpha`.
   - **Porcentaje de barra solicitada** visualiza la proporción de remediación extraída del accesorio de disputa.

## 4. Comprobaciones esperadas

- `sorafs_simulation_quota_total_gib{scope="assigned"}` equivale a `600` mientras el total comprometido se mantiene >=600.
- `sorafs_simulation_failover_triggered` reporta `1` y la métrica del proveedor de reemplazo resalta `beta`.
- `sorafs_simulation_slash_requested` reporta `0.15` (15% de barra diagonal) para el identificador de proveedor `alpha`.

Ejecuta `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que los dispositivos siguen siendo aceptados por el esquema de la CLI.