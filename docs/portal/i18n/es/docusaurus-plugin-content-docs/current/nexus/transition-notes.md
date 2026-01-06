---
lang: es
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-transition-notes
title: Notas de transicion de Nexus
description: Espejo de `docs/source/nexus_transition_notes.md`, que cubre evidencia de transicion de Phase B, el calendario de auditoria y las mitigaciones.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transicion de Nexus

Este registro rastrea el trabajo pendiente de **Phase B - Nexus Transition Foundations** hasta que finalice la checklist de lanzamiento multi-lane. Complementa las entradas de milestones en `roadmap.md` y mantiene la evidencia referenciada por B1-B4 en un solo lugar para que gobernanza, SRE y lideres de SDK compartan la misma fuente de verdad.

## Alcance y cadencia

- Cubre las auditorias routed-trace y los guardrails de telemetria (B1/B2), el conjunto de deltas de configuracion aprobado por gobernanza (B3) y los seguimientos del ensayo de lanzamiento multi-lane (B4).
- Reemplaza la nota temporal de cadencia que antes vivia aqui; desde la auditoria de Q1 2026 el reporte detallado reside en `docs/source/nexus_routed_trace_audit_report_2026q1.md`, mientras que esta pagina mantiene el calendario operativo y el registro de mitigaciones.
- Actualiza las tablas despues de cada ventana routed-trace, voto de gobernanza o ensayo de lanzamiento. Cuando los artefactos se muevan, refleja la nueva ubicacion dentro de esta pagina para que los docs posteriores (status, dashboards, portales SDK) puedan enlazar a un ancla estable.

## Snapshot de evidencia (2026 Q1-Q2)

| Workstream | Evidencia | Owner(s) | Estado | Notas |
|------------|----------|----------|--------|-------|
| **B1 - Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Completo (Q1 2026) | Tres ventanas de auditoria registradas; el retraso TLS de `TRACE-CONFIG-DELTA` se cerro durante el rerun de Q2. |
| **B2 - Remediacion de telemetria y guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Completo | Alert pack, politica del diff bot y tamano de lote OTLP (`nexus.scheduler.headroom` log + panel de Grafana de headroom) enviados; sin waivers abiertos. |
| **B3 - Aprobaciones de deltas de configuracion** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Completo | Voto GOV-2026-03-19 registrado; el bundle firmado alimenta el pack de telemetria citado abajo. |
| **B4 - Ensayo de lanzamiento multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completo (Q2 2026) | El rerun canary de Q2 cerro la mitigacion del retraso TLS; el validator manifest + `.sha256` captura el rango de slots 912-936, workload seed `NEXUS-REH-2026Q2` y el hash del perfil TLS registrado en el rerun. |

## Calendario trimestral de auditorias routed-trace

| Trace ID | Ventana (UTC) | Resultado | Notas |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Aprobado | Queue-admission P95 se mantuvo muy por debajo del objetivo <=750 ms. No se requiere accion. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Aprobado | OTLP replay hashes adjuntos a `status.md`; la paridad del SDK diff bot confirmo cero drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Resuelto | El retraso TLS se cerro durante el rerun de Q2; el pack de telemetria para `NEXUS-REH-2026Q2` registra el hash del perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) y cero rezagados. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Aprobado | Workload seed `NEXUS-REH-2026Q2`; pack de telemetria + manifest/digest en `artifacts/nexus/rehearsals/2026q1/` (slot range 912-936) con agenda en `artifacts/nexus/rehearsals/2026q2/`. |

Los trimestres futuros deben agregar nuevas filas y mover las entradas completadas a un apendice cuando la tabla crezca mas alla del trimestre actual. Referencia esta seccion desde reportes routed-trace o minutas de gobernanza usando el ancla `#quarterly-routed-trace-audit-schedule`.

## Mitigaciones y items de backlog

| Item | Descripcion | Owner | Objetivo | Estado / Notas |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Terminar de propagar el perfil TLS que se retraso durante `TRACE-CONFIG-DELTA`, capturar evidencia del rerun y cerrar el registro de mitigacion. | @release-eng, @sre-core | Ventana routed-trace de Q2 2026 | Cerrado - hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado en `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; el rerun confirmo que no hay rezagados. |
| `TRACE-MULTILANE-CANARY` prep | Programar el ensayo de Q2, adjuntar fixtures al pack de telemetria y asegurar que los SDK harnesses reutilicen el helper validado. | @telemetry-ops, SDK Program | Llamada de planeacion 2026-04-30 | Completado - agenda almacenada en `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` con metadatos de slot/workload; reutilizacion del harness anotada en el tracker. |
| Telemetry pack digest rotation | Ejecutar `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada ensayo/release y registrar digests junto al tracker de config delta. | @telemetry-ops | Por release candidate | Completado - `telemetry_manifest.json` + `.sha256` emitidos en `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests copiados en el tracker y el indice de evidencia. |

## Integracion del bundle de config delta

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` sigue siendo el resumen canonico de diffs. Cuando lleguen nuevos `defaults/nexus/*.toml` o cambios de genesis, actualiza ese tracker primero y luego refleja los puntos clave aqui.
- Los signed config bundles alimentan el pack de telemetria del ensayo. El pack, validado por `scripts/telemetry/validate_nexus_telemetry_pack.py`, debe publicarse junto con la evidencia de config delta para que los operadores puedan reproducir los artefactos exactos usados durante B4.
- Los bundles de Iroha 2 siguen sin lanes: las configs con `nexus.enabled = false` ahora rechazan overrides de lane/dataspace/routing a menos que el perfil Nexus este habilitado (`--sora`), asi que elimina las secciones `nexus.*` de las plantillas single-lane.
- Manten el registro del voto de gobernanza (GOV-2026-03-19) enlazado tanto desde el tracker como desde esta nota para que futuras votaciones puedan copiar el formato sin redescubrir el ritual de aprobacion.

## Seguimientos del ensayo de lanzamiento

- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura el plan canary, la lista de participantes y los pasos de rollback; actualiza el runbook cuando cambie la topologia de lanes o los exporters de telemetria.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefacto revisado durante el ensayo del 9 de abril y ahora incluye notas/agenda de preparacion Q2. Agrega futuros ensayos al mismo tracker en lugar de abrir trackers aislados para mantener la evidencia monotona.
- Publica snippets del colector OTLP y exports de Grafana (ver `docs/source/telemetry.md`) cuando cambie la guia de batching del exporter; la actualizacion de Q1 subio el batch size a 256 muestras para evitar alertas de headroom.
- La evidencia de CI/tests multi-lane ahora vive en `integration_tests/tests/nexus/multilane_pipeline.rs` y corre bajo el workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), reemplazando la referencia retirada `pytests/nexus/test_multilane_pipeline.py`; manten el hash para `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) en sync con el tracker al refrescar bundles de ensayo.

## Ciclo de vida de lanes en runtime

- Los planes de ciclo de vida de lanes en runtime ahora validan los bindings de dataspace y abortan cuando falla la reconciliacion de Kura/almacenamiento por niveles, dejando el catalogo sin cambios. Los helpers podan relays de lanes en cache para lanes retiradas, de modo que la sintesis merge-ledger no reutilice proofs obsoletas.
- Aplica planes a traves de los helpers de config/lifecycle de Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para agregar/retirar lanes sin reinicio; routing, snapshots TEU y registries de manifests se recargan automaticamente tras un plan exitoso.
- Guia para operadores: cuando un plan falla, revisa dataspaces faltantes o storage roots que no puedan crearse (tiered cold root/directorios Kura por lane). Corrige las rutas base y reintenta; los planes exitosos re-emiten el diff de telemetria de lane/dataspace para que los dashboards reflejen la nueva topologia.

## Telemetria NPoS y evidencia de backpressure

El retro de ensayo de lanzamiento de Phase B pidio capturas de telemetria deterministas que prueben que el pacemaker NPoS y las capas de gossip se mantienen dentro de sus limites de backpressure. El harness de integracion en `integration_tests/tests/sumeragi_npos_performance.rs` ejercita esos escenarios y emite resumenes JSON (`sumeragi_baseline_summary::<scenario>::...`) cuando se agregan nuevas metricas. Ejecutalo localmente con:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Configura `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` o `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologias de mayor estres; los valores por defecto reflejan el perfil de recolectores 1 s/`k=3` usado en B4.

| Escenario / test | Cobertura | Telemetria clave |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloquea 12 rondas con el block time del ensayo para registrar envelopes de latencia EMA, profundidades de cola y gauges de redundant-send antes de serializar el bundle de evidencia. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inunda la cola de transacciones para asegurar que las deferencias de admision se activen de forma determinista y que la cola exporte contadores de capacidad/saturacion. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Muestrea el jitter del pacemaker y los timeouts de vista hasta demostrar que la banda +/-125 permille esta aplicada. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empuja payloads RBC grandes hasta los limites soft/hard del store para mostrar que las sesiones y contadores de bytes suben, retroceden y se estabilizan sin sobrepasar el store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Fuerza retransmisiones para que los gauges de ratio redundant-send y los contadores de collectors-on-target avancen, demostrando que la telemetria pedida por el retro esta conectada end-to-end. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Descarta chunks a intervalos deterministas para verificar que los monitores de backlog levanten fallas en lugar de drenar silenciosamente los payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Adjunta las lineas JSON que imprime el harness junto con el scrape de Prometheus capturado durante la ejecucion siempre que gobernanza solicite evidencia de que las alarmas de backpressure coinciden con la topologia del ensayo.

## Checklist de actualizacion

1. Agrega nuevas ventanas routed-trace y retira las antiguas cuando roten los trimestres.
2. Actualiza la tabla de mitigaciones despues de cada seguimiento de Alertmanager, incluso si la accion es cerrar el ticket.
3. Cuando cambien los config deltas, actualiza el tracker, esta nota y la lista de digests del telemetry pack en el mismo pull request.
4. Enlaza aqui cualquier nuevo artefacto de ensayo/telemetria para que futuras actualizaciones de roadmap puedan referenciar un solo documento en lugar de notas ad-hoc dispersas.

## Indice de evidencia

| Activo | Ubicacion | Notas |
|-------|----------|-------|
| Reporte de auditoria routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fuente canonica para evidencia de Phase B1; reflejado para el portal en `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contiene los resumenes de diffs TRACE-CONFIG-DELTA, iniciales de revisores y el log de voto GOV-2026-03-19. |
| Plan de remediacion de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | Documenta el pack de alertas, el tamano de lote OTLP y los guardrails de presupuesto de exportacion vinculados a B2. |
| Tracker de ensayo multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lista los artefactos del ensayo del 9 de abril, manifest/digest del validator, notas/agenda Q2 y evidencia de rollback. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Registra el slot range 912-936, seed `NEXUS-REH-2026Q2` y hashes de artefactos para bundles de gobernanza. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash del perfil TLS aprobado capturado durante el rerun de Q2; citar en apendices routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planificacion para el ensayo Q2 (ventana, slot range, workload seed, owners de acciones). |
| Runbook de ensayo de lanzamiento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operativa para staging -> ejecucion -> rollback; actualizar cuando cambie la topologia de lanes o la guia de exporters. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenciado por el retro B4; archiva digests junto al tracker cuando el pack cambia. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prueba `nexus.enabled = true` para configs multi-lane, preserva los hashes del catalogo Sora y provisiona rutas Kura/merge-log por lane (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` antes de publicar digests de artefactos. |
