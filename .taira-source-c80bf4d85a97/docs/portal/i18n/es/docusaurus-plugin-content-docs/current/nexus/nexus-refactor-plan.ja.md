---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/nexus/nexus-refactor-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f749788058aa7f40f4cabd084737107de6824281c5cda9d7b945138cbf32365
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-refactor-plan
title: Plan de refactorizacion del ledger Sora Nexus
description: Espejo de `docs/source/nexus_refactor_plan.md`, que detalla el trabajo de limpieza por fases para el codebase de Iroha 3.
---

:::note Fuente canonica
Esta pagina refleja `docs/source/nexus_refactor_plan.md`. Manten ambas copias alineadas hasta que la edicion multilingue llegue al portal.
:::

# Plan de refactorizacion del ledger Sora Nexus

Este documento captura el roadmap inmediato para el refactor del Sora Nexus Ledger ("Iroha 3"). Refleja el layout actual del repositorio y las regresiones observadas en la contabilidad de genesis/WSV, el consenso Sumeragi, triggers de smart contracts, consultas de snapshots, bindings de host pointer-ABI y codecs Norito. El objetivo es converger en una arquitectura coherente y comprobable sin intentar aterrizar todas las correcciones en un unico parche monolitico.

## 0. Principios guia
- Preservar el comportamiento determinista en hardware heterogeneo; usar aceleracion solo con feature flags opt-in y fallbacks identicos.
- Norito es la capa de serializacion. Cualquier cambio de estado/esquema debe incluir pruebas de round-trip Norito encode/decode y actualizaciones de fixtures.
- La configuracion fluye por `iroha_config` (user -> actual -> defaults). Eliminar toggles ad-hoc de entorno en paths de produccion.
- La politica ABI sigue en V1 y no es negociable. Los hosts deben rechazar pointer types/syscalls desconocidos de forma determinista.
- `cargo test --workspace` y los golden tests (`ivm`, `norito`, `integration_tests`) siguen siendo la compuerta base para cada hito.

## 1. Snapshot de la topologia del repositorio
- `crates/iroha_core`: actores Sumeragi, WSV, loader de genesis, pipelines (query, overlay, zk lanes), glue del host de smart contracts.
- `crates/iroha_data_model`: esquema autoritativo para datos y queries on-chain.
- `crates/iroha`: API de cliente usada por CLI, tests, SDK.
- `crates/iroha_cli`: CLI de operadores, actualmente refleja numerosas APIs en `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, puntos de entrada de integracion pointer-ABI del host.
- `crates/norito`: codec de serializacion con adaptadores JSON y backends AoS/NCB.
- `integration_tests`: assertions cross-component que cubren genesis/bootstrap, Sumeragi, triggers, paginacion, etc.
- Los docs ya delinean metas del Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), pero la implementacion esta fragmentada y parcialmente obsoleta respecto al codigo.

## 2. Pilares de refactor y hitos

### Fase A - Fundaciones y observabilidad
1. **Telemetria WSV + Snapshots**
   - Establecer una API canonica de snapshots en `state` (trait `WorldStateSnapshot`) usada por queries, Sumeragi y CLI.
   - Usar `scripts/iroha_state_dump.sh` para producir snapshots deterministas via `iroha state dump --format norito`.
2. **Determinismo de Genesis/Bootstrap**
   - Refactorizar la ingesta de genesis para que pase por un unico pipeline con Norito (`iroha_core::genesis`).
   - Agregar cobertura de integracion/regresion que reprocesa genesis mas el primer bloque y afirma roots WSV identicos entre arm64/x86_64 (seguido en `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Tests de fixity cross-crate**
   - Expandir `integration_tests/tests/genesis_json.rs` para validar invariantes de WSV, pipeline y ABI en un solo harness.
   - Introducir un scaffold `cargo xtask check-shape` que haga panic ante schema drift (seguido bajo el backlog de tooling DevEx; ver el action item en `scripts/xtask/README.md`).

### Fase B - WSV y superficie de queries
1. **Transacciones de state storage**
   - Colapsar `state/storage_transactions.rs` en un adaptador transaccional que imponga orden de commits y deteccion de conflictos.
   - Los unit tests ahora verifican que modificaciones de assets/world/triggers hagan rollback ante fallos.
2. **Refactor del modelo de queries**
   - Mover la logica de paginacion/cursor a componentes reutilizables bajo `crates/iroha_core/src/query/`. Alinear representaciones Norito en `iroha_data_model`.
   - Agregar snapshot queries para triggers, assets y roles con orden determinista (seguido via `crates/iroha_core/tests/snapshot_iterable.rs` para la cobertura actual).
3. **Consistencia de snapshots**
   - Asegurar que `iroha ledger query` CLI use la misma ruta de snapshot que Sumeragi/fetchers.
   - Los tests de regresion de snapshots en CLI viven en `tests/cli/state_snapshot.rs` (feature-gated para runs lentos).

### Fase C - Pipeline Sumeragi
1. **Topologia y gestion de epocas**
   - Extraer `EpochRosterProvider` a un trait con implementaciones respaldadas por snapshots de stake en WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` ofrece un constructor simple y amigable para mocks en benches/tests.
2. **Simplificacion del flujo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` en modulos: `pacemaker`, `aggregation`, `availability`, `witness` con tipos compartidos bajo `consensus`.
   - Reemplazar el message passing ad-hoc por envelopes Norito tipados e introducir property tests de view-change (seguido en el backlog de mensajeria Sumeragi).
3. **Integracion lane/proof**
   - Alinear lane proofs con commitments de DA y asegurar que RBC gating sea uniforme.
   - El test de integracion end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs` ahora verifica la ruta con RBC habilitado.

### Fase D - Smart contracts y hosts pointer-ABI
1. **Auditoria de limite del host**
   - Consolidar los checks de pointer-type (`ivm::pointer_abi`) y los adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - Las expectativas de pointer table y los bindings de host manifest estan cubiertos por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` y `ivm_host_mapping.rs`, que ejercitan los mappings TLV golden.
2. **Sandbox de ejecucion de triggers**
   - Refactorizar triggers para ejecutar via un `TriggerExecutor` comun que impone gas, validacion de pointers y journaling de eventos.
   - Agregar tests de regresion para triggers de call/time cubriendo paths de fallo (seguido via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alineacion de CLI y client**
   - Asegurar que las operaciones CLI (`audit`, `gov`, `sumeragi`, `ivm`) dependan de las funciones compartidas de cliente `iroha` para evitar drift.
   - Los tests de snapshots JSON del CLI viven en `tests/cli/json_snapshot.rs`; mantenlos al dia para que la salida de comandos siga coincidiendo con la referencia JSON canonica.

### Fase E - Endurecimiento del codec Norito
1. **Registro de schemas**
   - Crear un registro de schema Norito bajo `crates/norito/src/schema/` para abastecer encodings canonicos de tipos core.
   - Se agregaron doc tests que verifican la codificacion de payloads de muestra (`norito::schema::SamplePayload`).
2. **Actualizacion de golden fixtures**
   - Actualizar los golden fixtures de `crates/norito/tests/*` para que coincidan con el nuevo schema WSV cuando aterrice el refactor.
   - `scripts/norito_regen.sh` regenera los golden JSON de Norito de forma determinista via el helper `norito_regen_goldens`.
3. **Integracion IVM/Norito**
   - Validar la serializacion de manifests Kotodama end-to-end via Norito, asegurando que la metadata pointer ABI sea consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantiene la paridad Norito encode/decode para manifests.

## 3. Temas transversales
- **Estrategia de tests**: Cada fase promueve unit tests -> crate tests -> integration tests. Los tests fallidos capturan regresiones actuales; los nuevos tests evitan que reaparezcan.
- **Documentacion**: Tras cada fase, actualizar `status.md` y mover items abiertos a `roadmap.md` mientras se podan tareas completadas.
- **Benchmarks de rendimiento**: Mantener benches existentes en `iroha_core`, `ivm` y `norito`; agregar mediciones base post-refactor para validar que no hay regresiones.
- **Feature flags**: Mantener toggles a nivel crate solo para backends que requieren toolchains externos (`cuda`, `zk-verify-batch`). Los paths SIMD de CPU siempre se construyen y se seleccionan en runtime; proveer fallbacks escalares deterministas para hardware no soportado.

## 4. Acciones inmediatas
- Scaffolding de Fase A (snapshot trait + wiring de telemetria) - ver tareas accionables en actualizaciones del roadmap.
- La auditoria reciente de defectos para `sumeragi`, `state` e `ivm` revelo los siguientes puntos:
  - `sumeragi`: allowances de dead-code protegen broadcast de pruebas de view-change, estado de replay VRF y export de telemetria EMA. Estos permanecen gated hasta que la simplificacion del flujo de consenso de la Fase C y los entregables de integracion lane/proof aterricen.
  - `state`: la limpieza de `Cell` y el ruteo de telemetria pasan al track de telemetria WSV de la Fase A, mientras que las notas de SoA/parallel-apply se integran al backlog de optimizacion de pipeline de la Fase C.
  - `ivm`: la exposicion de toggles CUDA, la validacion de envelopes y la cobertura Halo2/Metal se mapean al trabajo de host-boundary de la Fase D mas el tema transversal de aceleracion GPU; los kernels permanecen en el backlog dedicado de GPU hasta estar listos.
- Preparar un RFC cross-team que resuma este plan para sign-off antes de aterrizar cambios de codigo invasivos.

## 5. Preguntas abiertas
- Debe RBC seguir siendo opcional mas alla de P1, o es obligatorio para lanes del ledger Nexus? Requiere decision de stakeholders.
- Impulsamos grupos de composabilidad DS en P1 o los mantenemos deshabilitados hasta que maduren las lane proofs?
- Cual es la ubicacion canonica para los parametros ML-DSA-87? Candidato: nuevo crate `crates/fastpq_isi` (creacion pendiente).

---

_Ultima actualizacion: 2025-09-12_
