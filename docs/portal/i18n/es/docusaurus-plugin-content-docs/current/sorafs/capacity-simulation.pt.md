---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulación de capacidad
título: Runbook de simulación de capacidad de SoraFS
sidebar_label: Runbook de simulación de capacidad
Descripción: Ejecutar el kit de herramientas de simulación del mercado de capacidad SF-2c con accesorios reproducidos, exportaciones de Prometheus y paneles de control de Grafana.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Mantenha ambas como copias sincronizadas.
:::

Este runbook explica cómo ejecutar el kit de simulación del mercado de capacidad SF-2c y visualizar las métricas resultantes. Ele valida a negociação de cotas, o tratamento de failover y a remediação de slashing de ponta a ponta usando os accesorios determinísticos em `docs/examples/sorafs_capacity_simulation/`. Os payloads de capacidade ainda usam `sorafs_manifest_stub capacity`; use `iroha app sorafs toolkit pack` para los flujos de empacado de manifiesto/CAR.

## 1. Generar artefactos de CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsula `sorafs_manifest_stub capacity` para generar cargas útiles Norito, blobs base64, cuerpos de solicitud para Torii y resúmenes JSON para:

- Tres declaraciones de proveedores que participan en el escenario de negociación de cotas.
- Uma ordem de replicação que aloca o manifesto em staging entre esses provedores.
- Instantáneas de telemetría para la línea de base previa a la falla, el intervalo de falla y la recuperación de conmutación por error.
- Um payload de disputa solicitando slashing após a falha simulada.Todos los artefatos van para `./artifacts` (sustituyendo pasando un directorio diferente como primer argumento). Inspeccione los archivos `_summary.json` para contexto legal.

## 2. Agregar resultados y emitir métricas

```bash
./analyze.py --artifacts ./artifacts
```

El analizador de producto:

- `capacity_simulation_report.json` - ubicaciones agregadas, deltas de conmutación por error y metadados de disputa.
- `capacity_simulation.prom`: métricas de archivos de texto de Prometheus (`sorafs_simulation_*`) adecuadas para el recopilador de archivos de texto, el exportador de nodos o el trabajo de scrape independiente.

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

Aponte el recopilador de archivos de texto para `capacity_simulation.prom` (al usar node-exporter, cópielo al directorio pasado a través de `--collector.textfile.directory`).

## 3. Importar el panel de control desde Grafana

1. No Grafana, importe `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Vincule la variación de la fuente de datos `Prometheus` al objetivo de raspado configurado acima.
3. Verifique los dolores:
   - **Quota Allocation (GiB)** muestra los saldos comprometidos/atribuídos de cada proveedor.
   - **Failover Trigger** cambia para *Failover Active* cuando las métricas de falha chegam.
   - **Caída del tiempo de actividad durante una interrupción** presenta un porcentaje determinado del proveedor `alpha`.
   - **Porcentaje de barra diagonal solicitada** visualiza la razón de reparación extraída del accesorio de disputa.

## 4. Verificações esperadas- `sorafs_simulation_quota_total_gib{scope="assigned"}` equivale a `600` mientras o total comprometida permanece >=600.
- `sorafs_simulation_failover_triggered` reporta `1` y una métrica del proveedor sustituto destaca `beta`.
- `sorafs_simulation_slash_requested` reporta `0.15` (15% de barra diagonal) para el identificador de proveedor `alpha`.

Ejecute `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que los accesorios aún están en el esquema de CLI.