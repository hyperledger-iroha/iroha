---
lang: es
direction: ltr
source: docs/source/nexus_refactor_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44b7100fddd377c97dfcab678ce425ec35edfa4a1276f9b6a22aa2c64135a94d
source_last_modified: "2025-11-02T04:40:40.017979+00:00"
translation_last_reviewed: 2026-01-01
---

# Plan de refactorizacion del ledger Sora Nexus

Este documento captura el roadmap inmediato para la refactorizacion del Ledger Sora Nexus ("Iroha 3"). Refleja el layout actual del repositorio y las regresiones observadas en contabilidad de genesis/WSV, consenso Sumeragi, triggers de smart-contract, consultas de snapshot, bindings de host pointer-ABI y codecs Norito. El objetivo es converger en una arquitectura coherente y testeable sin intentar aterrizar todas las correcciones en un parche monolitico.

## 0. Principios guia
- Preservar comportamiento determinista en hardware heterogeneo; aprovechar aceleracion solo via flags opt-in con fallbacks identicos.
- Norito es la capa de serializacion. Cualquier cambio de estado/esquema debe incluir pruebas de round-trip Norito encode/decode y actualizaciones de fixtures.
- La configuracion fluye a traves de `iroha_config` (user -> actual -> defaults). Eliminar toggles ad-hoc de entorno en rutas de produccion.
- La politica ABI se mantiene en V1 y no es negociable. Los hosts deben rechazar tipos de puntero/syscalls desconocidos de forma determinista.
- `cargo test --workspace` y las pruebas golden (`ivm`, `norito`, `integration_tests`) siguen siendo el gate base para cada hito.

## 1. Resumen de topologia del repositorio
- `crates/iroha_core`: actores Sumeragi, WSV, cargador genesis, pipelines (query, overlay, zk lanes), glue de host para smart-contracts.
- `crates/iroha_data_model`: esquema autoritativo para datos on-chain y queries.
- `crates/iroha`: API de cliente usada por CLI, pruebas y SDK.
- `crates/iroha_cli`: CLI de operador, actualmente refleja numerosas APIs en `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, puntos de entrada de integracion host pointer-ABI.
- `crates/norito`: codec de serializacion con adaptadores JSON y backends AoS/NCB.
- `integration_tests`: asserts cross-component cubriendo genesis/bootstrap, Sumeragi, triggers, paginacion, etc.
- Los docs ya describen objetivos del Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), pero la implementacion esta fragmentada y parcialmente desactualizada respecto al codigo.

## 2. Pilares e hitos de refactorizacion

### Fase A - Fundamentos y observabilidad
1. **Telemetria WSV + snapshots**
   - Establecer API de snapshot canonica en `state` (trait `WorldStateSnapshot`) usada por queries, Sumeragi y CLI.
   - Usar `scripts/iroha_state_dump.sh` para producir snapshots deterministas via `iroha state dump --format norito`.
2. **Determinismo de genesis/bootstrap**
   - Refactorizar la ingestion de genesis para fluir por un solo pipeline con Norito (`iroha_core::genesis`).
   - Agregar cobertura de integracion/regresion que reproduce genesis mas el primer bloque y afirma raices WSV identicas en arm64/x86_64 (trackeado en `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Pruebas de fijacion cross-crate**
   - Expandir `integration_tests/tests/genesis_json.rs` para validar invariantes de WSV, pipeline y ABI en un solo harness.
   - Introducir un scaffold `cargo xtask check-shape` que haga panic ante schema drift (trackeado en el backlog de tooling DevEx; ver el action item de `scripts/xtask/README.md`).

### Fase B - WSV y superficie de queries
1. **Transacciones de state storage**
   - Colapsar `state/storage_transactions.rs` en un adaptador transaccional que imponga orden de commit y deteccion de conflictos.
   - Las pruebas unitarias ahora verifican que modificaciones de assets/world/triggers hacen rollback al fallar.
2. **Refactor del modelo de queries**
   - Mover la logica de paginacion/cursor a componentes reutilizables bajo `crates/iroha_core/src/query/`. Alinear representaciones Norito en `iroha_data_model`.
   - Agregar snapshot queries para triggers, assets y roles con orden determinista (trackeado via `crates/iroha_core/tests/snapshot_iterable.rs` para la cobertura actual).
3. **Consistencia de snapshots**
   - Asegurar que `iroha ledger query` CLI use la misma ruta de snapshot que Sumeragi/fetchers.
   - Las pruebas de regresion de snapshots en CLI viven en `tests/cli/state_snapshot.rs` (feature-gated para ejecuciones lentas).

### Fase C - Pipeline Sumeragi
1. **Topologia y gestion de epocas**
   - Extraer `EpochRosterProvider` a un trait con implementaciones respaldadas por snapshots de stake WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` ofrece un constructor simple y facil de mock para benches/tests.
2. **Simplificacion del flujo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` en modulos: `pacemaker`, `aggregation`, `availability`, `witness` con tipos compartidos bajo `consensus`.
   - Reemplazar message passing ad-hoc con envelopes Norito tipados e introducir property tests de view-change (trackeado en el backlog de mensajeria Sumeragi).
3. **Integracion de lanes/pruebas**
   - Alinear pruebas de lanes con compromisos DA y asegurar que el gating RBC sea uniforme.
   - El test de integracion end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs` ahora verifica el camino con RBC habilitado.

### Fase D - Smart contracts y hosts pointer-ABI
1. **Auditoria de frontera de host**
   - Consolidar chequeos de tipos de puntero (`ivm::pointer_abi`) y adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - Expectativas de tabla de punteros y bindings de host manifest estan cubiertos por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` y `ivm_host_mapping.rs`, que ejercitan los mappings TLV golden.
2. **Sandbox de ejecucion de triggers**
   - Refactorizar triggers para ejecutar via un `TriggerExecutor` comun que imponga gas, validacion de punteros y journaling de eventos.
   - Agregar pruebas de regresion para triggers de llamada/tiempo cubriendo rutas de fallo (trackeado via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alineacion CLI y cliente**
   - Asegurar que operaciones CLI (`audit`, `gov`, `sumeragi`, `ivm`) dependan de funciones compartidas del cliente `iroha` para evitar drift.
   - Las pruebas de snapshot JSON del CLI viven en `tests/cli/json_snapshot.rs`; mantenerlas al dia para que la salida de comandos core siga la referencia JSON canonica.

### Fase E - Endurecimiento del codec Norito
1. **Schema registry**
   - Crear un registry de schema Norito bajo `crates/norito/src/schema/` para obtener codificaciones canonicas de tipos core.
   - Agregar doc tests que verifican encoding de payloads de ejemplo (`norito::schema::SamplePayload`).
2. **Refresh de fixtures golden**
   - Actualizar `crates/norito/tests/*` golden fixtures para que coincidan con el nuevo schema WSV cuando la refactorizacion aterrice.
   - `scripts/norito_regen.sh` regenera los goldens Norito JSON de forma determinista via el helper `norito_regen_goldens`.
3. **Integracion IVM/Norito**
   - Validar la serializacion de manifest Kotodama end-to-end via Norito, asegurando que la metadata de pointer ABI sea consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantiene paridad Norito encode/decode para manifests.

## 3. Preocupaciones transversales
- **Estrategia de pruebas**: Cada fase promueve pruebas unitarias -> pruebas de crate -> pruebas de integracion. Las pruebas fallidas capturan regresiones actuales; las nuevas pruebas evitan que reaparezcan.
- **Documentacion**: Despues de cada fase, actualizar `status.md` y mover items abiertos a `roadmap.md` mientras se podan tareas completadas.
- **Benchmarks de performance**: Mantener benches existentes en `iroha_core`, `ivm` y `norito`; agregar mediciones base post-refactor para validar que no hay regresiones.
- **Feature flags**: Mantener toggles a nivel crate solo para backends que requieren toolchains externos (`cuda`, `zk-verify-batch`). Las rutas SIMD de CPU siempre se compilan y se seleccionan en runtime; proveer fallbacks escalares deterministas para hardware no soportado.

## 4. Acciones inmediatas
- Scaffolding de Fase A (snapshot trait + wiring de telemetria) - ver tareas accionables en las actualizaciones de roadmap.
- La auditoria reciente de defectos para `sumeragi`, `state` e `ivm` detecto los siguientes puntos:
  - `sumeragi`: allowances de codigo muerto guardan broadcast de proofs de view-change, estado de replay VRF y export de telemetria EMA. Se mantienen gated hasta que la simplificacion del flujo de consenso y la integracion de lanes/pruebas de la Fase C aterricen.
  - `state`: limpieza de `Cell` y el ruteo de telemetria pasan al track de telemetria WSV de Fase A, mientras las notas de SoA/parallel-apply entran al backlog de optimizacion de pipeline de Fase C.
  - `ivm`: exposicion del toggle CUDA, validacion de envelopes y cobertura Halo2/Metal se mapean al trabajo de frontera de host de Fase D junto con el tema transversal de aceleracion GPU; los kernels permanecen en el backlog GPU dedicado hasta estar listos.
- Preparar un RFC cross-team que resuma este plan para el sign-off antes de aterrizar cambios de codigo invasivos.

## 5. Preguntas abiertas
- Debe RBC seguir opcional despues de P1, o es obligatorio para lanes del ledger Nexus? Requiere decision de stakeholders.
- Se fuerzan grupos de composabilidad DS en P1 o se mantienen deshabilitados hasta que maduren las pruebas de lanes?
- Cual es la ubicacion canonica para los parametros ML-DSA-87? Candidato: nuevo crate `crates/fastpq_isi` (creacion pendiente).

---

_Ultima actualizacion: 2025-09-12_
