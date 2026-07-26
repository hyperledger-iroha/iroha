---
lang: es
direction: ltr
source: docs/portal/docs/nexus/transition-notes.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas-de-transición-nexus
título: Notas de transición de Nexus
descripción: Espejo de `docs/source/nexus_transition_notes.md`, que cubre evidencia de transición de Fase B, el calendario de auditoría y las mitigaciones.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transición de Nexus

Este registro rastrea el trabajo pendiente de **Phase B - Nexus Transition Foundations** hasta que finalice la checklist de lanzamiento multi-lane. Complementa las entradas de hitos en `roadmap.md` y mantiene la evidencia referenciada por B1-B4 en un solo lugar para que gobernanza, SRE y líderes de SDK compartan la misma fuente de verdad.

## Alcance y cadencia

- Cubre las auditorías routed-trace y los guardrails de telemetria (B1/B2), el conjunto de deltas de configuración aprobado por gobernanza (B3) y los seguimientos del ensayo de lanzamiento multi-lane (B4).
- Reemplaza la nota temporal de cadencia que antes vivía aquí; desde la auditoria del Q1 2026 el reporte detallado reside en `docs/source/nexus_routed_trace_audit_report_2026q1.md`, mientras que esta página mantiene el calendario operativo y el registro de mitigaciones.
- Actualiza las tablas después de cada ventana routed-trace, voto de gobernanza o ensayo de lanzamiento. Cuando los artefactos se mueven, refleja la nueva ubicación dentro de esta página para que los documentos posteriores (estado, paneles, portales SDK) puedan enlazar a un ancla estable.## Instantánea de evidencia (2026 Q1-Q2)

| Flujo de trabajo | Pruebas | Propietario(s) | Estado | Notas |
|------------|----------|----------|--------|-------|
| **B1 - Auditorías de seguimiento enrutado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @operaciones de telemetría, @gobernanza | Completo (Q1 2026) | Tres ventanas de auditorias registradas; el retraso TLS de `TRACE-CONFIG-DELTA` se cerro durante la repetición de Q2. |
| **B2 - Remediación de telemetría y barandillas** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Completo | Paquete de alerta, política del diff bot y tamano de lote OTLP (`nexus.scheduler.headroom` log + panel de Grafana de headroom) enviados; renuncias de pecado abiertas. |
| **B3 - Aprobaciones de deltas de configuración** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Completo | Voto GOV-2026-03-19 registrado; el paquete firmado alimenta el paquete de telemetría citado abajo. |
| **B4 - Ensayo de lanzamiento multicarril** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completo (Q2 2026) | El rerun canary de Q2 cerro la mitigacion del retraso TLS; el validator manifest + `.sha256` captura el rango de slots 912-936, workload seed `NEXUS-REH-2026Q2` y el hash del perfil TLS registrado en el rerun. |

## Calendario trimestral de auditorias routed-trace| ID de seguimiento | Ventana (UTC) | Resultado | Notas |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Aprobado | Queue-admission P95 se mantuvo muy por debajo del objetivo <=750 ms. No se requiere acción. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Aprobado | Hashes de reproducción OTLP adjuntos a `status.md`; la paridad del SDK diff bot confirma cero deriva. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Resuelto | El retraso TLS se cerro durante la repetición de la Q2; el pack de telemetria para `NEXUS-REH-2026Q2` registra el hash del perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) y cero rezagados. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Aprobado | Semilla de carga de trabajo `NEXUS-REH-2026Q2`; pack de telemetria + manifest/digest en `artifacts/nexus/rehearsals/2026q1/` (rango de ranura 912-936) con agenda en `artifacts/nexus/rehearsals/2026q2/`. |

Los trimestres futuros deben agregar nuevas filas y mover las entradas completadas a un apéndice cuando la tabla crezca más allá del trimestre actual. Referencia esta sección desde informes routed-trace o minutas de gobernanza usando el ancla `#quarterly-routed-trace-audit-schedule`.

## Mitigaciones y elementos de backlog| Artículo | Descripción | Propietario | Objetivo | Estado / Notas |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Terminar de propagar el perfil TLS que se retrasa durante `TRACE-CONFIG-DELTA`, capturar evidencia del reejecución y cerrar el registro de mitigación. | @release-eng, @sre-core | Ventana routed-trace del segundo trimestre de 2026 | Cerrado - hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado en `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; la repetición confirma que no hay rezagados. |
| Preparación `TRACE-MULTILANE-CANARY` | Programar el ensayo de Q2, adjuntar accesorios al paquete de telemetría y asegurar que los arneses SDK reutilicen el ayudante validado. | @telemetry-ops, Programa SDK | Llamada de planeación 2026-04-30 | Completado - agenda almacenada en `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` con metadatos de slot/workload; reutilizacion del arnés anotado en el tracker. |
| Rotación de resumen del paquete de telemetría | Ejecutar `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada ensayo/release y registrar digests junto al tracker de config delta. | @operaciones de telemetría | Por liberación candidato | Completado - `telemetry_manifest.json` + `.sha256` emitidos en `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, semilla `NEXUS-REH-2026Q2`); resúmenes copiados en el tracker y el índice de evidencia. |

## Integración del paquete de configuración delta- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` sigue siendo el resumen canónico de diferencias. Cuando lleguen nuevos `defaults/nexus/*.toml` o cambios de génesis, actualiza ese tracker primero y luego refleja los puntos clave aquí.
- Los paquetes de configuración firmados alimentan el paquete de telemetría del ensayo. El paquete, validado por `scripts/telemetry/validate_nexus_telemetry_pack.py`, debe publicarse junto con la evidencia de config delta para que los operadores puedan reproducir los artefactos exactos usados ​​durante B4.
- Los paquetes de Iroha 2 siguen sin carriles: las configuraciones con `nexus.enabled = false` ahora rechazan overrides de lane/dataspace/routing a menos que el perfil Nexus este habilitado (`--sora`), asi que elimina las secciones `nexus.*` de las plantillas carril único.
- Mantenga el registro del voto de gobernanza (GOV-2026-03-19) enlazado tanto desde el tracker como desde esta nota para que futuras votaciones puedan copiar el formato sin redescubrir el ritual de aprobación.

## Seguimientos del ensayo de lanzamiento- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura el plan canary, la lista de participantes y los pasos de rollback; actualiza el runbook cuando cambia la topología de carriles o los exportadores de telemetría.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefacto revisado durante el ensayo del 9 de abril y ahora incluye notas/agenda de preparación Q2. Agrega futuros ensayos al mismo tracker en lugar de abrir trackers aislados para mantener la evidencia monotona.
- Publica snippets del colector OTLP y exports de Grafana (ver `docs/source/telemetry.md`) cuando cambie la guía de procesamiento por lotes del exportador; la actualización de Q1 subio el tamaño del lote a 256 muestras para evitar alertas de headroom.
- La evidencia de CI/tests multi-lane ahora vive en `integration_tests/tests/nexus/multilane_pipeline.rs` y corre bajo el flujo de trabajo `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), reemplazando la referencia retirada `pytests/nexus/test_multilane_pipeline.py`; mantenga el hash para `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) en sincronización con el tracker al refrescar paquetes de ensayo.

## Ciclo de vida de carriles en tiempo de ejecución- Los planes de ciclo de vida de lanes en runtime ahora validan los enlaces de dataspace y abortan cuando falla la reconciliacion de Kura/almacenamiento por niveles, dejando el catalogo sin cambios. Los ayudantes pueden retransmitir de carriles en caché para retiradas de carriles, de modo que la síntesis merge-ledger no reutiliza pruebas obsoletas.
- Aplica planos a través de los ayudantes de config/lifecycle de Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para agregar/retirar carriles sin reiniciar; enrutamiento, instantáneas TEU y registros de manifiestos se recargan automáticamente tras un plan exitoso.
- Guía para operadores: cuando un plan falla, revisa los espacios de datos faltantes o las raíces de almacenamiento que no pueden crearse (tiered cold root/directorios Kura por lane). Corrige las rutas base y reintenta; los planes exitosos re-emiten el diff de telemetria de lane/dataspace para que los tableros reflejen la nueva topologia.

## Telemetria NPoS y evidencia de contrapresión

El retro de ensayo de lanzamiento de Phase B pidio capturas de telemetría deterministas que prueban que el marcapasos NPoS y las capas de chismes se mantienen dentro de sus límites de contrapresión. El arnés de integración en `integration_tests/tests/sumeragi_npos_performance.rs` ejercita esos escenarios y emite resúmenes JSON (`sumeragi_baseline_summary::<scenario>::...`) cuando se agregan nuevas métricas. Ejecutalo localmente con:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```Configure `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` o `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologías de mayores tensiones; los valores por defecto reflejan el perfil de recolectores 1 s/`k=3` usado en B4.| Escenario/prueba | Cobertura | Clave de telemetria |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloquea 12 rondas con el bloque de tiempo del ensayo para registrar sobres de latencia EMA, profundidades de cola y calibres de envío redundante antes de serializar el paquete de evidencia. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inunda la cola de transacciones para asegurar que las deferencias de admisión se activen de forma determinista y que la cola exporte contadores de capacidad/saturación. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Muestrea el jitter del marcapasos y los tiempos de espera de vista hasta demostrar que la banda +/-125 permille está aplicada. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empuja payloads RBC grandes hasta los límites soft/hard del store para mostrar que las sesiones y contadores de bytes suben, retroceden y se estabilizan sin sobrepasar el store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Fuerza retransmisiones para que los calibres de ratio redundant-send y los contadores de coleccionistas-on-target avancen, demostrando que la telemetria pedida por el retro esta conectada de extremo a extremo. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. || `npos_rbc_chunk_loss_fault_reports_backlog` | Descartes divide a intervalos deterministas para verificar que los monitores de backlog levanten fallas en lugar de drenar silenciosamente las cargas útiles. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Adjunta las lineas JSON que imprime el arnés junto con el scrape de Prometheus capturado durante la ejecución siempre que gobernanza solicita evidencia de que las alarmas de contrapresión coinciden con la topología del ensayo.

## Lista de verificación de actualización

1. Agrega nuevas ventanas routed-trace y retira las antiguas cuando roten los trimestres.
2. Actualiza la tabla de mitigaciones después de cada seguimiento de Alertmanager, incluso si la acción es cerrar el ticket.
3. Cuando cambien los deltas de configuración, actualice el rastreador, esta nota y la lista de resúmenes del paquete de telemetría en el mismo pull request.
4. Enlaza aquí cualquier nuevo artefacto de ensayo/telemetría para que futuras actualizaciones de roadmap puedan hacer referencia a un solo documento en lugar de notas ad-hoc dispersas.

## Índice de evidencia| Activo | Ubicación | Notas |
|-------|----------|-------|
| Informe de auditoría routed-trace (primer trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fuente canónica para evidencia de Fase B1; reflejado para el portal en `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Rastreador de configuración delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contiene los resúmenes de diferencias TRACE-CONFIG-DELTA, iniciales de revisores y el registro de voto GOV-2026-03-19. |
| Plan de remediación de telemetría | `docs/source/nexus_telemetry_remediation_plan.md` | Documenta el pack de alertas, el tamaño de lote OTLP y los guardrails de presupuesto de exportación vinculados a B2. |
| Tracker de ensayo multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lista los artefactos del ensayo del 9 de abril, manifest/digest del validator, notas/agenda Q2 y evidencia de rollback. |
| Manifiesto/resumen del paquete de telemetría (más reciente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Registre el slot range 912-936, seed `NEXUS-REH-2026Q2` y hashes de artefactos para paquetes de gobernanza. |
| Manifiesto de perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash del perfil TLS aprobado capturado durante la repetición de Q2; citar en apéndices routed-trace. |
| Agenda TRAZA-MULTILANO-CANARIO | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planificacion para el ensayo Q2 (ventana, rango de ranuras, semilla de carga de trabajo, propietarios de acciones). |
| Runbook de ensayo de lanzamiento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Lista de verificación operativa para staging -> ejecucion -> rollback; cuando cambie la topología de carriles o la guía de exportadores. || Validador de paquetes de telemetría | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenciado por el retro B4; archiva digests junto al tracker cuando el pack cambia. |
| Regresión multicarril | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prueba `nexus.enabled = true` para configuraciones de varios carriles, conserva los hashes del catálogo Sora y provisiona rutas Kura/merge-log por carril (`blocks/lane_{id:03}_{slug}`) vía `ConfigLaneRouter` antes de publicar resúmenes de artefactos. |