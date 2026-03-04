---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: es
direction: ltr
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d9fbcf5301bd36a0bdc8a010ae214f7b1561373237054ab3c8a45a411cf6c0e
source_last_modified: "2025-12-14T09:53:36.241505+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de finalidad de lanes de Nexus y oráculo

**Estado:** Activo — satisface el entregable de dashboard/runbook NX-18.  
**Audiencia:** Core Consensus WG, SRE/Telemetry, Release Engineering, líderes on-call.  
**Alcance:** Cubre los SLOs de duración de slot, quórum DA, oráculo y buffer de liquidación que gobiernan la promesa de finalidad de 1 s. Úselo junto con `dashboards/grafana/nexus_lanes.json` y los helpers de telemetría bajo `scripts/telemetry/`.

## Dashboards

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — publica el tablero “Nexus Lane Finality & Oracles”. Los paneles rastrean:
  - `histogram_quantile()` sobre `iroha_slot_duration_ms` (p50/p95/p99) más el gauge de la muestra más reciente.
  - `iroha_da_quorum_ratio` y `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` para resaltar churn de DA.
  - Superficies del oráculo: `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`, `iroha_oracle_twap_window_seconds`, y `iroha_oracle_haircut_basis_points`.
  - Panel de buffer de liquidación (`iroha_settlement_buffer_xor`) que muestra débitos por lane en vivo a partir de recibos `LaneBlockCommitment`.
- **Reglas de alerta** — reutilizan las cláusulas SLO de Slot/DA de `ans3.md`. Pager cuando:
  - p95 de duración de slot > 1000 ms durante dos ventanas consecutivas de 5 m,
  - ratio de quórum DA < 0.95 o `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m]) > 0`,
  - obsolescencia del oráculo > 90 s o ventana TWAP ≠ 60 s configurados,
  - buffer de liquidación < 25 % (soft) / 10 % (hard) una vez que la métrica esté activa.

## Hoja de referencia de métricas

| Métrica | Objetivo / Alerta | Notas |
|--------|-------------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 ms (hard), 950 ms warning | Use el panel del dashboard o ejecute `scripts/telemetry/check_slot_duration.py` (`--json-out artifacts/nx18/slot_summary.json`) contra el export de Prometheus recolectado durante caos. |
| `iroha_slot_duration_ms_latest` | Refleja el slot más reciente; investigue si > 1100 ms incluso cuando los cuantiles parecen OK. | Exporte el valor al abrir incidentes. |
| `iroha_da_quorum_ratio` | ≥ 0.95 en ventana móvil de 30 m. | Derivado de reprogramaciones DA durante commits de bloques. |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | Debe permanecer en 0 fuera de ensayos de caos. | Trate cualquier incremento sostenido como `missing-availability warning`. |

Cada reprogramación también dispara una advertencia del pipeline Torii con `kind = "missing-availability warning"`. Capture esos eventos junto con el pico de la métrica para identificar el encabezado de bloque afectado, el intento de reintento y los conteos de requeue sin rastrear logs de validadores.【crates/iroha_core/src/sumeragi/main_loop.rs:5164】
| `iroha_oracle_staleness_seconds` | ≤ 60 s. Alerta a 75 s. | Indica feeds TWAP de 60 s obsoletos. |
| `iroha_oracle_twap_window_seconds` | Exactamente 60 s ± tolerancia de 5 s. | La divergencia indica mala configuración del oráculo. |
| `iroha_oracle_haircut_basis_points` | Coincide con el tier de liquidez del lane (0/25/75 bps). | Escale si los haircuts suben inesperadamente. |
| `iroha_settlement_buffer_xor` | Soft 25 %, hard 10 %. Forzar modo solo XOR por debajo de 10 %. | El panel expone débitos micro‑XOR por lane/dataspace; exporte antes de ajustar la política del router. |

## Playbook de respuesta

### Incumplimiento de duración de slot
1. Confirme vía dashboard + `promql` (p95/p99).  
2. Capture la salida de `scripts/telemetry/check_slot_duration.py --json-out <path>` (y el snapshot de métricas) para que revisores CXO verifiquen la puerta de 1 s.  
3. Inspeccione entradas de RCA: profundidad de cola de mempool, reprogramaciones DA, trazas IVM.  
4. Abra incidente, adjunte captura de Grafana y programe un drill de caos si la regresión persiste.

### Degradación de quórum DA
1. Revise `iroha_da_quorum_ratio` y el contador de reprogramaciones; correlacione con logs `missing-availability warning`.  
2. Si el ratio <0.95, fije attesters fallando, amplíe parámetros de muestreo o cambie el perfil a modo solo XOR.  
3. Ejecute `scripts/telemetry/check_nexus_audit_outcome.py` durante ensayos de routed‑trace para probar que los eventos `nexus.audit.outcome` siguen pasando tras la mitigación.  
4. Archive los paquetes de recibos DA con el ticket del incidente.

### Obsolescencia del oráculo / deriva de haircut
1. Use los paneles 5–8 para verificar precio, obsolescencia, ventana TWAP y haircut.  
2. Para obsolescencia >90 s: reinicie o haga failover del feed del oráculo y vuelva a ejecutar el harness de caos.  
3. Para mismatch de haircut: inspeccione la configuración del perfil de liquidez y cambios recientes de gobernanza; notifique a tesorería si se requiere intervención en líneas de swap.

### Alertas de buffer de liquidación
1. Use `iroha_settlement_buffer_xor` (más recibos nocturnos) para confirmar margen antes de ajustar la política del router.  
2. Cuando la métrica cruce un umbral, dispare procedimientos:
   - **Incumplimiento soft (<25 %)**: involucrar tesorería, considerar uso de líneas de swap y registrar la alerta.  
   - **Incumplimiento hard (<10 %)**: forzar inclusión solo XOR, rechazar lanes subsidiados y documentar en `ops/drill-log.md`.  
3. Referencie `docs/source/settlement_router.md` para palancas de repo/reverse‑repo.

## Evidencia y automatización

- **CI** — conecte `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` y `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` al flujo de aceptación RC para que cada release candidate entregue el resumen de duración de slot más los resultados de la puerta DA/oráculo/buffer junto con el snapshot de métricas mencionado arriba. El helper ya se invoca desde `ci/check_nexus_lane_smoke.sh`.  
- **Paridad de dashboard** — ejecute `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` para asegurar que el tablero publicado coincide con los exports de staging/prod.  
- **Artefactos de trazas** — durante ensayos TRACE o drills de caos NX-18, invoque `scripts/telemetry/check_nexus_audit_outcome.py` para archivar el payload `nexus.audit.outcome` más reciente (`docs/examples/nexus_audit_outcomes/`). Adjunte tanto el archivo como capturas de Grafana al drill log.
- **Empaquetado de evidencia de slot** — después de generar el JSON de resumen, ejecute `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` para que el `slot_bundle_manifest.json` resultante capture digests SHA-256 para ambos artefactos. Suba el directorio tal cual con el bundle de evidencia RC. El pipeline de release lo ejecuta automáticamente (se puede omitir con `--skip-nexus-lane-smoke`) y copia `artifacts/nx18/` en la salida del release.

## Lista de mantenimiento

- Mantenga `dashboards/grafana/nexus_lanes.json` en sincronía con exports de Grafana tras cada cambio de esquema; documente las ediciones en los commits referenciando NX-18.  
- Actualice este runbook cuando lleguen nuevas métricas (p. ej., gauges del buffer de liquidación) o umbrales de alerta.  
- Registre cada ensayo de caos (latencia de slot, jitter DA, stall del oráculo, agotamiento de buffer) con `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>`.

Seguir este runbook proporciona la evidencia de “dashboards/runbooks de operador” exigida por NX-18 y asegura que el SLO de finalidad se mantenga exigible antes de Nexus GA.
