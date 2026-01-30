---
id: taikai-monitoring-dashboards
lang: es
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

La preparacion del routing-manifest de Taikai (TRM) depende de dos tableros
Grafana y sus alertas asociadas. Esta pagina refleja los puntos destacados de
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json` y
`dashboards/alerts/taikai_viewer_rules.yml` para que los reviewers puedan seguir
sin clonar el repositorio.

## Dashboard de viewer (`taikai_viewer.json`)

- **Live edge y latencia:** Los paneles visualizan los histogramas de latencia
  p95/p99 (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`)
  por cluster/stream. Vigila p99 > 900 ms o drift > 1.5 s (dispara la alerta
  `TaikaiLiveEdgeDrift`).
- **Errores de segmentos:** Desglosa `taikai_ingest_segment_errors_total{reason}`
  para exponer fallas de decode, intentos de lineage replay o mismatches de
  manifest. Adjunta capturas a incidentes SN13-C cuando este panel suba por encima
  de la banda “warning”.
- **Salud de viewer y CEK:** Los paneles basados en metricas `taikai_viewer_*`
  siguen la edad de rotacion de CEK, el mix de PQ guard, conteos de rebuffer y
  roll-ups de alertas. El panel de CEK hace cumplir el SLA de rotacion que la
  gobernanza revisa antes de aprobar nuevos aliases.
- **Snapshot de telemetria de aliases:** La tabla `/status → telemetry.taikai_alias_rotations`
  vive en el tablero para que los operadores confirmen digests de manifest antes
  de adjuntar evidencia de gobernanza.

## Dashboard de cache (`taikai_cache.json`)

- **Presion por tier:** Los paneles grafican `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  y `sorafs_taikai_cache_promotions_total`. Usalos para ver si una rotacion TRM
  sobrecarga tiers especificos.
- **Denegaciones de QoS:** `sorafs_taikai_qos_denied_total` aparece cuando la presion
  de cache fuerza throttling; anota el drill log cada vez que la tasa se aleja de cero.
- **Utilizacion de egress:** Ayuda a confirmar que las salidas SoraFS alcanzan a los
  viewers de Taikai cuando rotan las ventanas CMAF.

## Alertas y captura de evidencia

- Las reglas de paging viven en `dashboards/alerts/taikai_viewer_rules.yml` y
  se mapean uno a uno con los paneles de arriba (`TaikaiLiveEdgeDrift`,
  `TaikaiIngestFailure`, `TaikaiCekRotationLag`, proof-health warnings). Asegura que
  todos los clusters de produccion las conecten a Alertmanager.
- Los snapshots/capturas tomadas durante drills deben almacenarse en
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` junto con los spool files y el
  JSON de `/status`. Usa `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  para anexar la ejecucion al drill log compartido.
- Cuando cambien los dashboards, incluye el digest SHA-256 del archivo JSON en la
  descripcion del PR del portal para que los auditores puedan empatar la carpeta
  Grafana administrada con la version del repo.

## Checklist del bundle de evidencia

Las revisiones SN13-C esperan que cada drill o incidente entregue los mismos
artefactos listados en el runbook del ancla Taikai. Capturalos en el orden
siguiente para que el bundle quede listo para revision de gobernanza:

1. Copia los archivos mas recientes `taikai-anchor-request-*.json`,
   `taikai-trm-state-*.json` y `taikai-lineage-*.json` desde
   `config.da_ingest.manifest_store_dir/taikai/`. Estos artefactos de spool prueban
   que routing manifest (TRM) y la ventana de lineage estuvieron activos. El helper
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   copiara los spool files, emitira hashes y opcionalmente firmara el resumen.
2. Registra la salida de `/v1/status` filtrada a
   `.telemetry.taikai_alias_rotations[]` y guardala junto a los spool files.
   Los reviewers comparan el `manifest_digest_hex` reportado y los limites de
   ventana con el estado de spool copiado.
3. Exporta snapshots de Prometheus para las metricas listadas arriba y toma
   capturas de los dashboards viewer/cache con los filtros relevantes de
   cluster/stream visibles. Deja el JSON/CSV crudo y las capturas en la carpeta
   de artefactos.
4. Incluye IDs de incidentes de Alertmanager (si los hay) que referencien reglas
   de `dashboards/alerts/taikai_viewer_rules.yml` y anota si se auto-cerraron cuando
   la condicion se limpio.

Guarda todo bajo `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` para que los
recordatorios de auditoria y las revisiones de gobernanza SN13-C puedan obtener
un solo archivo.

## Cadencia de drills y logging

- Ejecuta el drill de ancla Taikai el primer martes de cada mes a las 15:00 UTC.
  Este calendario mantiene la evidencia fresca antes de la sync de gobernanza SN13.
- Despues de capturar los artefactos anteriores, anexa la ejecucion al ledger
  compartido con `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
  El helper emite la entrada JSON requerida por `docs/source/sorafs/runbooks-index.md`.
- Enlaza los artefactos archivados en la entrada del runbook index y escala
  cualquier alerta fallida o regresion de dashboard dentro de 48 horas via el
  canal Media Platform WG/SRE.
- Mantiene el set de capturas de resumen del drill (latencia, drift, errores,
  rotacion de CEK, presion de cache) junto al spool bundle para que los operadores
  puedan mostrar exactamente como se comportaron los dashboards durante el ensayo.

Consulta el [Taikai Anchor Runbook](./taikai-anchor-runbook.md) para el
procedimiento completo de Sev 1 y el checklist de evidencia. Esta pagina solo
captura la guia especifica de dashboards que SN13-C requiere antes de dejar 🈺.
