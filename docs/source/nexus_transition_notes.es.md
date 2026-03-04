<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: es
direction: ltr
source: docs/source/nexus_transition_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: baf4e91fdb2c447c453711170e7df58a9a4831b15957052818736e0e7914e8a3
source_last_modified: "2025-12-13T06:33:05.401788+00:00"
translation_last_reviewed: 2026-01-01
---

# Notas de transicion de Nexus

Este registro sigue el trabajo pendiente de **Fase B - Fundamentos de transicion Nexus**
hasta que la lista de lanzamiento multi-lane termine. Complementa las entradas de
hitos en `roadmap.md` y mantiene la evidencia referenciada por B1-B4 en un solo lugar
para que gobernanza, SRE y lideres de SDK compartan la misma fuente de verdad.

## Alcance y cadencia

- Cubre las auditorias de routed-trace y los guardrails de telemetria (B1/B2), el
  conjunto de deltas de configuracion aprobados por gobernanza (B3) y los seguimientos
  del ensayo de lanzamiento multi-lane (B4).
- Reemplaza la nota temporal de cadencia que antes vivia aqui; desde la auditoria 2026
  Q1 el informe detallado reside en
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`, mientras esta pagina
  conserva el calendario activo y el registro de mitigaciones.
- Actualiza las tablas despues de cada ventana de routed-trace, voto de gobernanza
  o ensayo de lanzamiento. Cuando los artefactos se muevan, refleja la nueva ubicacion
  dentro de esta pagina para que los docs posteriores (status, dashboards, portales SDK)
  puedan enlazar a un ancla estable.

## Resumen de evidencia (2026 Q1-Q2)

| Flujo de trabajo | Evidencia | Responsables | Estado | Notas |
|-----------------|-----------|--------------|--------|-------|
| **B1 - Auditorias routed-trace** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Completo (Q1 2026) | Tres ventanas de auditoria registradas; el retraso TLS de `TRACE-CONFIG-DELTA` se cerro durante el rerun de Q2. |
| **B2 - Remediacion de telemetria y guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Completo | Paquete de alertas, politica del diff bot y tamano de lote OTLP (`nexus.scheduler.headroom` log + panel de headroom en Grafana) enviados; sin exenciones abiertas. |
| **B3 - Aprobaciones de delta de config** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Completo | Voto GOV-2026-03-19 capturado; el bundle firmado alimenta el paquete de telemetria mencionado abajo. |
| **B4 - Ensayo de lanzamiento multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completo (Q2 2026) | El rerun canario Q2 cerro la mitigacion de retraso TLS; manifest de validadores + `.sha256` capturan el rango de slots 912-936, la semilla de carga `NEXUS-REH-2026Q2` y el hash de perfil TLS registrado del rerun. |

## Calendario trimestral de auditorias routed-trace

| ID de trace | Ventana (UTC) | Resultado | Notas |
|------------|--------------|----------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Aprobado | El P95 de admission de cola se mantuvo muy por debajo del objetivo <=750 ms. Sin accion requerida. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Aprobado | Hashes de replay OTLP adjuntos a `status.md`; el diff bot de SDK confirmo cero drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Resuelto | El retraso TLS se cerro durante el rerun Q2; el paquete de telemetria para `NEXUS-REH-2026Q2` registra el hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) y cero rezagados. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Aprobado | Semilla de carga `NEXUS-REH-2026Q2`; paquete de telemetria + manifest/digest bajo `artifacts/nexus/rehearsals/2026q1/` (rango de slots 912-936) con agenda en `artifacts/nexus/rehearsals/2026q2/`. |

Los trimestres futuros deben agregar nuevas filas y mover las entradas completadas a un
apendice cuando la tabla crezca mas alla del trimestre actual. Referencia esta seccion
desde reportes routed-trace o actas de gobernanza usando el ancla
`#quarterly-routed-trace-audit-schedule`.

## Elementos de mitigacion y backlog

| Elemento | Descripcion | Responsable | Objetivo | Estado / Notas |
|---------|-------------|------------|---------|----------------|
| `NEXUS-421` | Terminar de propagar el perfil TLS que se atraso durante `TRACE-CONFIG-DELTA`, capturar evidencia del rerun y cerrar el registro de mitigacion. | @release-eng, @sre-core | Ventana routed-trace Q2 2026 | Cerrado - hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado en `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; el rerun confirmo sin rezagados. |
| Preparacion `TRACE-MULTILANE-CANARY` | Programar el ensayo Q2, adjuntar fixtures al paquete de telemetria y asegurar que los harness de SDK reutilicen el helper validado. | @telemetry-ops, SDK Program | Llamada de planificacion 2026-04-30 | Completado - agenda almacenada en `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` con metadatos de slot/carga; reutilizacion del harness anotada en el tracker. |
| Rotacion de digests del paquete de telemetria | Ejecutar `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada ensayo/release y registrar digests junto al tracker de delta de config. | @telemetry-ops | Por release candidate | Completado - `telemetry_manifest.json` + `.sha256` emitidos en `artifacts/nexus/rehearsals/2026q1/` (rango de slots `912-936`, semilla `NEXUS-REH-2026Q2`); digests copiados al tracker e indice de evidencia. |

## Integracion del bundle de deltas de config

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` sigue siendo el
  resumen canonico de diffs. Cuando lleguen nuevos `defaults/nexus/*.toml` o cambios
  de genesis, actualiza primero ese tracker y luego refleja los puntos clave aqui.
- Los bundles de config firmados alimentan el paquete de telemetria del ensayo. El paquete,
  validado por `scripts/telemetry/validate_nexus_telemetry_pack.py`, debe publicarse
  junto con la evidencia de delta de config para que los operadores puedan reproducir los
  artefactos exactos usados durante B4.
- Los bundles de Iroha 2 siguen sin lanes: los configs con `nexus.enabled = false` ahora
  rechazan overrides de lane/dataspace/routing a menos que el perfil Nexus este habilitado
  (`--sora`), asi que elimina secciones `nexus.*` de plantillas de una sola lane.
- Mantener el log de voto de gobernanza (GOV-2026-03-19) enlazado desde el tracker y
  esta nota para que votos futuros copien el formato sin redescubrir el ritual de aprobacion.

## Seguimientos del ensayo de lanzamiento

- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura el plan canario, la lista
  de participantes y los pasos de rollback; refresca el runbook cuando cambien la
  topologia de lanes o los exporters de telemetria.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefacto revisado
  durante el ensayo del 9 de abril y ahora incluye las notas/agenda de preparacion Q2.
  Agrega ensayos futuros al mismo tracker en lugar de abrir trackers aislados para
  mantener la evidencia monotona.
- Publica snippets del collector OTLP y exportaciones Grafana (ver `docs/source/telemetry.md`)
  cuando cambie la guia de batching del exporter; la actualizacion Q1 subio el tamano
  de lote a 256 muestras para evitar alertas de headroom.
- La evidencia de CI/test multi-lane ahora vive en
  `integration_tests/tests/nexus/multilane_pipeline.rs` y corre bajo el workflow
  `Nexus Multilane Pipeline`
  (`.github/workflows/integration_tests_multilane.yml`), reemplazando la referencia
  retirada `pytests/nexus/test_multilane_pipeline.py`; manten el hash de
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) sincronizado
  con el tracker al refrescar bundles de ensayo.

## Ciclo de vida de lane en runtime

- Los planes de ciclo de vida de lane en runtime ahora validan los bindings de dataspace
  y abortan cuando falla la reconciliacion de Kura/almacenamiento en niveles, dejando
  el catalogo sin cambios. Los helpers podan los relays de lane cacheados para lanes
  retiradas para que la sintesis del merge-ledger no reutilice proofs obsoletas.
- Aplica planes con los helpers de config/lifecycle de Nexus (`State::apply_lane_lifecycle`,
  `Queue::apply_lane_lifecycle`) para agregar/retirar lanes sin reinicio; el ruteo,
  snapshots TEU y registries de manifest se recargan automaticamente tras un plan exitoso.
- Guia para operadores: cuando un plan falla, revisa dataspaces faltantes o roots de
  almacenamiento que no puedan crearse (cold root en niveles/directorios Kura de lane).
  Arregla las rutas base y reintenta; los planes exitosos re-emiten el diff de telemetria
  de lane/dataspace para que los dashboards reflejen la nueva topologia.

## Evidencia de telemetria y backpressure NPoS

La retro del ensayo de lanzamiento de la Fase B pidio capturas deterministas de
telemetria que prueben que el pacemaker NPoS y las capas de gossip se mantienen
dentro de sus limites de backpressure. El harness de integracion en
`integration_tests/tests/sumeragi_npos_performance.rs` ejercita esos escenarios
y emite resumenes JSON (`sumeragi_baseline_summary::<scenario>::...`) cada vez
que llegan nuevas metricas. Ejecutalo localmente con:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Configura `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` o
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologias de mayor carga; los
valores por defecto reflejan el perfil de colectores de 1 s/`k=3` usado en B4.

| Escenario / test | Cobertura | Telemetria clave |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloquea 12 rondas con el block time del ensayo para registrar envolventes de latencia EMA, profundidades de cola y medidores de redundant-send antes de serializar el bundle de evidencia. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inunda la cola de transacciones para asegurar que las demoras de admission se activen deterministamente y que la cola exporte contadores de capacidad/saturacion. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Muestrea el jitter del pacemaker y los timeouts de vista hasta probar que se cumple la banda configurada de +/-125 permille. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empuja payloads RBC grandes hasta los limites soft/hard del store para mostrar que sesiones y contadores de bytes suben, retroceden y se estabilizan sin sobrepasar el store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Fuerza retransmisiones para que los medidores de redundant-send y los contadores de collectors-on-target avancen, probando que la telemetria solicitada esta conectada de punta a punta. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Descarta chunks espaciados de forma determinista para verificar que los monitores de backlog elevan fallas en lugar de drenar payloads en silencio. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Adjunta las lineas JSON que imprime el harness junto con el scrape de Prometheus
capturado durante la ejecucion cuando gobernanza solicite evidencia de que las
alarmas de backpressure coinciden con la topologia del ensayo.

## Lista de actualizacion

1. Agrega nuevas ventanas de routed-trace y retira las antiguas cuando roten los trimestres.
2. Actualiza la tabla de mitigacion despues de cada seguimiento de Alertmanager, incluso si la
   accion es cerrar el ticket.
3. Cuando cambien los deltas de config, actualiza el tracker, esta nota y la lista de digests del
   paquete de telemetria en el mismo pull request.
4. Enlaza aqui cualquier nuevo artefacto de ensayo/telemetria para que futuras actualizaciones
   de status puedan referenciar un solo documento en lugar de notas ad-hoc dispersas.

## Indice de evidencia

| Activo | Ubicacion | Notas |
|--------|-----------|-------|
| Reporte de auditoria routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fuente canonica para evidencia de Fase B1; reflejado para el portal bajo `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de delta de config | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contiene los resumenes TRACE-CONFIG-DELTA, iniciales de revisores y el log de voto GOV-2026-03-19. |
| Plan de remediacion de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | Documenta el paquete de alertas, el tamano de lote OTLP y los guardrails de presupuesto de exportacion ligados a B2. |
| Tracker de ensayo multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lista artefactos del ensayo del 9 de abril, manifest/digest de validadores, notas/agenda Q2 y evidencia de rollback. |
| Manifest/digest del paquete de telemetria (ultimo) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Registra rango de slots 912-936, semilla `NEXUS-REH-2026Q2` y hashes de artefactos para bundles de gobernanza. |
| Manifest de perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash del perfil TLS aprobado capturado durante el rerun Q2; citar en apendices de routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planificacion para el ensayo Q2 (ventana, rango de slots, semilla de carga, responsables). |
| Runbook de ensayo de lanzamiento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operacional para staging -> ejecucion -> rollback; actualizar cuando cambie la topologia de lanes o la guia de exporters. |
| Validador del paquete de telemetria | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenciada por la retro B4; archiva digests junto al tracker cuando cambie el paquete. |
| Regresion multilane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prueba `nexus.enabled = true` para configs multi-lane, preserva hashes del catalogo Sora y provisiona rutas Kura/merge-log locales (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` antes de publicar digests de artefactos. |
