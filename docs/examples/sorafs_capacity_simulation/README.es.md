---
lang: es
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-11-05T17:59:15.481814+00:00"
translation_last_reviewed: 2026-01-01
---

# Toolkit de simulacion de capacidad SoraFS

Este directorio entrega los artefactos reproducibles para la simulacion del mercado de capacidad SF-2c. El toolkit ejercita negociacion de cuotas, manejo de failover y remediacion de slashing usando los helpers de CLI de produccion y un script de analisis liviano.

## Prerrequisitos

- Toolchain de Rust capaz de ejecutar `cargo run` para miembros del workspace.
- Python 3.10+ (solo libreria estandar).

## Inicio rapido

```bash
# 1. Generar artefactos CLI canonicos
./run_cli.sh ./artifacts

# 2. Agregar resultados y emitir metricas Prometheus
./analyze.py --artifacts ./artifacts
```

El script `run_cli.sh` invoca `sorafs_manifest_stub capacity` para construir:

- Declaraciones deterministas de providers para el set de fixtures de negociacion de cuotas.
- Un orden de replicacion que coincide con el escenario de negociacion.
- Snapshots de telemetria para la ventana de failover.
- Un payload de disputa que captura la solicitud de slashing.

El script escribe bytes Norito (`*.to`), payloads base64 (`*.b64`), cuerpos de
solicitudes Torii y resumentes legibles (`*_summary.json`) bajo el directorio
de artefactos elegido.

`analyze.py` consume los resumentes generados, produce un reporte agregado
(`capacity_simulation_report.json`) y emite un textfile Prometheus
(`capacity_simulation.prom`) con:

- Gauges `sorafs_simulation_quota_*` que describen capacidad negociada y cuota
  de asignacion por provider.
- Gauges `sorafs_simulation_failover_*` que destacan deltas de downtime y el
  provider de reemplazo seleccionado.
- `sorafs_simulation_slash_requested` registrando el porcentaje de remediacion
  extraido del payload de disputa.

Importa el bundle Grafana en `dashboards/grafana/sorafs_capacity_simulation.json`
y apunta a un datasource Prometheus que scrapee el textfile generado (por ejemplo
via el textfile collector de node-exporter). El runbook en
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` describe el flujo completo,
incluyendo tips de configuracion de Prometheus.

## Fixtures

- `scenarios/quota_negotiation/` - Especificaciones de provider y orden de replicacion.
- `scenarios/failover/` - Ventanas de telemetria para la caida primaria y el lift de failover.
- `scenarios/slashing/` - Especificacion de disputa que referencia el mismo orden de replicacion.

Estas fixtures se validan en `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`
para garantizar que sigan en sync con el esquema de CLI.
