---
lang: es
direction: ltr
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1773b8fda6cda00e38b333096bfe5d6f6181c883ece5a62c11a190a09870d29
source_last_modified: "2025-12-12T12:49:53.638997+00:00"
translation_last_reviewed: 2026-01-01
---

## Desglose de tareas Sumeragi + NPoS

Esta nota amplia la hoja de ruta de la Fase A en tareas de ingenieria pequenas para poder incorporar el trabajo restante de Sumeragi/NPoS de forma incremental. Las anotaciones de estado siguen la convencion: `DONE` completado, `IN PROGRESS` en progreso, `NOT STARTED` sin iniciar, y `NEEDS TESTS` requiere pruebas.

### A2 - Adopcion de mensajes a nivel wire
- DONE: Exponer los tipos Norito `Proposal`/`Vote`/`Qc` en `BlockMessage` y ejecutar round-trips de encode/decode (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- DONE: Bloquear los frames anteriores `BlockSigned/BlockCommitted`; el toggle de migracion quedo en `false` antes del retiro.
- DONE: Retirar el knob de migracion que alternaba los mensajes de bloque antiguos; el modo Vote/QC ahora es el unico camino wire.
- DONE: Actualizar routers Torii, comandos CLI y consumidores de telemetria para preferir snapshots JSON `/v1/sumeragi/*` sobre los frames de bloque anteriores.
- DONE: La cobertura de integracion ejercita los endpoints `/v1/sumeragi/*` solo sobre el pipeline Vote/QC (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- DONE: Eliminar los frames antiguos cuando haya paridad de funciones y pruebas de interoperabilidad.

### Plan de eliminacion de frames
1. DONE: Pruebas soak multi-nodo corrieron 72 h en los harnesses de telemetria y CI; los snapshots de Torii mostraron throughput estable del proposer y formacion de QC sin regresiones.
2. DONE: La cobertura de pruebas de integracion ahora corre solo por el camino Vote/QC (`sumeragi_vote_qc_commit.rs`), asegurando que peers mixtos alcancen consenso sin los frames antiguos.
3. DONE: La documentacion para operadores y la ayuda del CLI ya no mencionan el camino wire previo; la guia de troubleshooting ahora apunta a la telemetria Vote/QC.
4. DONE: Se eliminaron variantes de mensajes, contadores de telemetria y caches de commit pendientes; la matriz de superficie ahora refleja la superficie solo Vote/QC.

### A3 - Aplicacion del motor y pacemaker
- DONE: Invariantes Lock/HighestQC aplicadas en `handle_message` (ver `block_created_header_sanity`).
- DONE: El seguimiento de disponibilidad de datos valida el hash del payload RBC al registrar la entrega (`Actor::ensure_block_matches_rbc_payload`), para que sesiones desajustadas no se traten como entregadas.
- DONE: Integrar el requisito PrecommitQC (`require_precommit_qc`) en configuraciones por defecto y agregar pruebas negativas (por defecto ahora `true`; las pruebas cubren rutas con gating y opt-out).
- DONE: Reemplazar las heuristicas de redundant-send a nivel de view por controladores de pacemaker respaldados por EMA (`aggregator_retry_deadline` ahora deriva de la EMA en vivo y define los deadlines de redundant send).
- DONE: Bloquear el armado de propuestas bajo backpressure de la cola (`BackpressureGate` ahora detiene el pacemaker cuando la cola esta saturada y registra deferrals para status/telemetry).
- DONE: Los votos de availability se emiten tras la validacion de la propuesta cuando se requiere DA (sin esperar el `DELIVER` local de RBC), y la evidencia de availability se rastrea via `AvailabilityQC` como prueba de seguridad mientras el commit avanza sin esperar. Esto evita esperas circulares entre transporte de payload y votacion.
- DONE: La cobertura de restart/liveness ahora ejercita la recuperacion RBC en cold-start (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) y la reanudacion del pacemaker tras downtime (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`).
- DONE: Agregar pruebas de regresion deterministas de restart/view-change que cubren la convergencia de lock (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 - Pipeline de collectors y aleatoriedad
- DONE: Los helpers de rotacion determinista de collectors viven en `collectors.rs`.
- DONE: GA-A4.1 - La seleccion de collectors respaldada por PRF ahora registra seeds deterministas y height/view en `/status` y telemetria; los hooks de refresh de VRF propagan el contexto tras commits y reveals. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md` (cerrado).
- DONE: GA-A4.2 - Exponer telemetria de participacion de reveal + comandos CLI de inspeccion y actualizar manifests Norito. Owners: `@telemetry-ops`, `@torii-sdk`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:6`.
- DONE: GA-A4.3 - Codificar recuperacion de late-reveal y pruebas de epoch sin participacion bajo `integration_tests/tests/sumeragi_randomness.rs` (`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`), ejerciendo telemetria de limpieza de penalizaciones. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:7`.

### A5 - Reconfiguracion conjunta y evidencia
- DONE: El scaffolding de evidence, la persistencia en WSV y los roundtrips Norito ahora cubren double-vote, invalid proposal, invalid QC y variantes de double exec con deduplicacion determinista y poda del horizonte (`sumeragi::evidence`).
- DONE: GA-A5.1 - La activacion de joint-consensus (el set viejo commitea, el nuevo se activa en el siguiente bloque) se impone con cobertura de integracion dirigida.
- DONE: GA-A5.2 - Se actualizaron docs de governance y flujos CLI para slashing/jailing, con pruebas de sincronizacion mdBook que fijan defaults y el wording del evidence horizon.
- DONE: GA-A5.3 - Pruebas de evidence de ruta negativa (duplicate signer, forged signature, stale epoch replay, mixed manifest payloads) mas fixtures fuzz aterrizaron y corren nightly para proteger la validacion de roundtrip Norito.

### A6 - Tooling, docs, validacion
- DONE: Telemetria/reporting de RBC en su lugar; el reporte DA genera metricas reales (incluye contadores de eviction).
- DONE: GA-A6.1 - La prueba happy-path NPoS con VRF y 4 peers ahora corre en CI con umbrales pacemaker/RBC aplicados via `integration_tests/tests/sumeragi_npos_happy_path.rs`. Owners: `@qa-consensus`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:11`.
- DONE: GA-A6.2 - Capturar baseline de rendimiento NPoS (bloques de 1 s, k=3) y publicar en `status.md`/docs de operador con seeds reproducibles del harness + matriz de hardware. Owners: `@performance-lab`, `@telemetry-ops`. Report: `docs/source/generated/sumeragi_baseline_report.md`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:12`. La corrida en vivo quedo registrada en Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) usando el comando documentado en `scripts/run_sumeragi_baseline.py`.
- DONE: GA-A6.3 - Las guias de troubleshooting para instrumentacion de RBC/pacemaker/backpressure llegaron (`docs/source/telemetry.md:523`); la correlacion de logs ahora la maneja `scripts/sumeragi_backpressure_log_scraper.py`, para que los operadores puedan extraer pares de deferral de pacemaker/availability faltante sin greps manuales. Owners: `@operator-docs`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:13`.
- DONE: Se agregaron escenarios de rendimiento de RBC store/chunk-loss (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`), cobertura de fan-out redundante (`npos_redundant_send_retries_update_metrics`) y un harness de jitter acotado (`npos_pacemaker_jitter_within_band`) para que la suite A6 ejercite deferrals de soft-limit del store, drops deterministas de chunks, telemetria de redundant-send y bandas de jitter del pacemaker bajo estres. [integration_tests/tests/sumeragi_npos_performance.rs:633] [integration_tests/tests/sumeragi_npos_performance.rs:760] [integration_tests/tests/sumeragi_npos_performance.rs:800] [integration_tests/tests/sumeragi_npos_performance.rs:639]

### Pasos inmediatos
1. DONE: El harness de jitter acotado ejercita metricas de jitter del pacemaker (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. DONE: Promover las aserciones de deferral RBC en `npos_queue_backpressure_triggers_metrics` al cebar presion determinista del store RBC (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. DONE: Extender el soak de `/v1/sumeragi/telemetry` para cubrir epochs largas y collectors adversarios, comparando snapshots contra contadores de Prometheus en multiples heights. Cubierto por `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`.

Registrar esta lista aqui mantiene a `roadmap.md` enfocado en hitos mientras da al equipo una lista viva para completar. Actualiza las entradas (y marca la finalizacion) a medida que lleguen los cambios.
