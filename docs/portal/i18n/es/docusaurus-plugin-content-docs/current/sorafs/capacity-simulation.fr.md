---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulación de capacidad
título: Runbook de simulación de capacidad SoraFS
sidebar_label: Runbook de simulación de capacidad
descripción: Ejercite el kit de simulación del mercado de capacidad SF-2c con accesorios reproducibles, exportaciones Prometheus y tablas de borde Grafana.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Guarde las dos copias sincronizadas justo con el conjunto de documentación Sphinx herité soit entièrement migré.
:::

Este runbook comentario explícito ejecuta el kit de simulación del mercado de capacidad SF-2c y visualiza las métricas resultantes. La negociación de cuotas, la gestión de conmutación por error y la solución de reducción de cuotas se validan con la ayuda de los dispositivos determinados en `docs/examples/sorafs_capacity_simulation/`. Les payloads de capacité utilisent toujours `sorafs_manifest_stub capacity`; utilice `iroha app sorafs toolkit pack` para el flujo de embalaje manifest/CAR.

## 1. Generar artefactos CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` encapsula `sorafs_manifest_stub capacity` para editar las cargas útiles Norito, los blobs base64, el cuerpo de solicitudes Torii y los currículums JSON para:- Tres declaraciones de proveedores participantes en el escenario de negociación de cuotas.
- Un ordre de replication allouant le manifeste en staging entre ces fournisseurs.
- Las instantáneas de télémétrie para la línea de base previa, el intervalo de pantalla y la recuperación de conmutación por error.
- Un payload de litige demandant un slashing après la panne simulée.

Todos los artefactos son depositados bajo `./artifacts` (reemplace de paso un otro repertorio en primer argumento). Inspeccione los archivos `_summary.json` para un contexto lisible.

## 2. Agréger les résultats et émettre les métriques

```bash
./analyze.py --artifacts ./artifacts
```

El analizador de producto :

- `capacity_simulation_report.json` - asignaciones agregadas, deltas de conmutación por error y metadonnées de litigio.
- `capacity_simulation.prom`: archivo de texto de métricas Prometheus (`sorafs_simulation_*`) adaptado al recopilador de archivos de texto, al exportador de nodos o a un trabajo scrape independiente.

Ejemplo de configuración de scrape Prometheus:

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

Dirige el recopilador de archivos de texto a la versión `capacity_simulation.prom` (si utilizas node-exporter, copia el repertorio pasado a través de `--collector.textfile.directory`).

## 3. Panel de control del importador Grafana1. En Grafana, importe `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Asocie la variable de fuente de datos `Prometheus` al cible de scrape configurado ci-dessus.
3. Verifique los paneles:
   - **Asignación de cuota (GiB)** cartel de los soldados comprometidos/asignados para cada proveedor.
   - **Failover Trigger** activa en *Failover Active* cuando llegan las métricas de panel.
   - **Caída del tiempo de actividad durante una interrupción** rastrea la pérdida en porcentaje para el proveedor `alpha`.
   - **Porcentaje de barra diagonal solicitada** visualizar la proporción de reparación extraído del accesorio de litigio.

## 4. Asistentes a verificaciones

- `sorafs_simulation_quota_total_gib{scope="assigned"}` es igual a `600` hasta que el total de participación sea >=600.
- `sorafs_simulation_failover_triggered` indica `1` y la métrica del proveedor de reemplazo se reunió antes de `beta`.
- `sorafs_simulation_slash_requested` indica `0.15` (15 % de barra diagonal) para el identificador del proveedor `alpha`.

Ejecute `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para confirmar que los accesorios todavía son aceptados según el esquema CLI.