---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Runbook de observabilidad del ancla Taikai

Esta copia del portal refleja el runbook canonico en
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
Usalo cuando ensayes anclas de routing-manifest (TRM) de SN13-C para que los
operadores de SoraFS/SoraNet puedan correlacionar artefactos de spool, telemetria
Prometheus y evidencias de gobernanza sin salir del preview del portal.

## Alcance y owners

- **Programa:** SN13-C — manifiestos Taikai y anclas SoraNS.
- **Owners:** Media Platform WG, DA Program, Networking TL, Docs/DevRel.
- **Objetivo:** Proveer un playbook determinista para alertas Sev 1/Sev 2, validacion
  de telemetria y captura de evidencias mientras los routing manifests de Taikai
  avanzan a traves de aliases.

## Quickstart (Sev 1/Sev 2)

1. **Captura artefactos de spool** — copia los archivos mas recientes
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` y
   `taikai-lineage-*.json` desde
   `config.da_ingest.manifest_store_dir/taikai/` antes de reiniciar workers.
2. **Vuelca telemetria `/status`** — registra el arreglo
   `telemetry.taikai_alias_rotations` para demostrar que ventana de manifiesto esta
   activa:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Revisa dashboards y alertas** — carga
   `dashboards/grafana/taikai_viewer.json` (filtros de cluster + stream) y nota
   si alguna regla en
   `dashboards/alerts/taikai_viewer_rules.yml` se disparo (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, eventos de salud PoR de SoraFS).
4. **Inspecciona Prometheus** — ejecuta las consultas de la seccion "Referencia de
   metricas" para confirmar que la latencia/deriva de ingest y los contadores de
   rotacion de alias se comportan como se espera. Escala si
   `taikai_trm_alias_rotations_total` se estanca por varias ventanas o si los
   contadores de error aumentan.

## Referencia de metricas

| Metrica | Proposito |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | Histograma de latencia de ingest CMAF por cluster/stream (objetivo: p95 < 750 ms, p99 < 900 ms). |
| `taikai_ingest_live_edge_drift_ms` | Drift de live-edge entre encoder y workers de anclaje (pagea si p99 > 1.5 s durante 10 min). |
| `taikai_ingest_segment_errors_total{reason}` | Contadores de error por razon (`decode`, `manifest_mismatch`, `lineage_replay`, ...). Cualquier incremento dispara `TaikaiIngestFailure`. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | Incrementa cada vez que `/v2/da/ingest` acepta un nuevo TRM para un alias; usa `rate()` para validar la cadencia de rotacion. |
| `/status → telemetry.taikai_alias_rotations[]` | Snapshot JSON con `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` y timestamps para bundles de evidencia. |
| `taikai_viewer_*` (rebuffer, edad de rotacion CEK, salud PQ, alertas) | KPIs del viewer para asegurar que la rotacion CEK + circuitos PQ se mantienen sanos durante los anchors. |

### Snippets PromQL

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## Dashboards y alertas

- **Grafana viewer board:** `dashboards/grafana/taikai_viewer.json` — latencia p95/p99,
  drift de live-edge, errores de segmentos, edad de rotacion CEK, alertas del viewer.
- **Grafana cache board:** `dashboards/grafana/taikai_cache.json` — promociones hot/warm/cold
  y denegaciones de QoS cuando rotan las ventanas de alias.
- **Alertmanager rules:** `dashboards/alerts/taikai_viewer_rules.yml` — paging por drift,
  avisos de falla de ingest, lag de rotacion CEK y penalizaciones/cooldowns de salud
  PoR de SoraFS. Asegura receptores para cada cluster de produccion.

## Checklist de bundle de evidencias

- Artefactos de spool (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- Ejecuta `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`
  para emitir un inventario JSON firmado de sobres pendientes/entregados y copiar
  archivos de request/SSM/TRM/lineage al bundle del drill. La ruta de spool por
  defecto es `storage/da_manifests/taikai` desde `torii.toml`.
- Snapshot de `/status` que cubra `telemetry.taikai_alias_rotations`.
- Exportaciones Prometheus (JSON/CSV) para las metricas anteriores en la ventana del incidente.
- Capturas de Grafana con filtros visibles.
- IDs de Alertmanager que referencien los disparos relevantes.
- Enlace a `docs/examples/taikai_anchor_lineage_packet.md` describiendo el paquete
  canonico de evidencia.

## Replicado de dashboards y cadencia de drills

Satisfacer el requisito SN13-C significa demostrar que los dashboards Taikai
viewer/cache se reflejan dentro del portal **y** que el drill de evidencias de
anclaje corre en una cadencia predecible.

1. **Mirroring del portal.** Cuando `dashboards/grafana/taikai_viewer.json` o
   `dashboards/grafana/taikai_cache.json` cambien, resume los deltas en
   `sorafs/taikai-monitoring-dashboards` (este portal) y anota los checksums JSON
   en la descripcion del PR del portal. Destaca nuevos paneles/umbrales para que
   revisores puedan correlacionar con la carpeta Grafana administrada.
2. **Drill mensual.**
   - Ejecuta el drill el primer martes de cada mes a las 15:00 UTC para que la
     evidencia llegue antes del sync de gobernanza de SN13.
   - Captura artefactos de spool, telemetria `/status` y capturas Grafana dentro de
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Registra la ejecucion con
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Revision y publicacion.** En 48 horas, revisa alertas/falsos positivos con el
   DA Program + NetOps, registra follow-ups en el drill log y enlaza la carga al
   bucket de gobernanza desde `docs/source/sorafs/runbooks-index.md`.

Si dashboards o drills se retrasan, SN13-C no puede salir de 🈺; mantén esta
seccion actualizada cuando cambie la cadencia o las expectativas de evidencia.

## Comandos utiles

```bash
# Snapshot de telemetria de rotacion de alias en un directorio de artefactos
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# Lista entradas del spool para un alias/evento especifico
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspecciona razones de mismatch TRM desde el spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Mantén esta copia del portal sincronizada con el runbook canonico cuando cambien
la telemetria de anclaje Taikai, los dashboards o los requisitos de evidencia de
la gobernanza.
