---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-refactor-nexus
título: Plan de refactorización del libro mayor Sora Nexus
descripción: Espejo de `docs/source/nexus_refactor_plan.md`, que detalla el trabajo de limpieza por fases para el código base de Iroha 3.
---

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_refactor_plan.md`. Mantenga ambas copias alineadas hasta que la edición multilingue llegue al portal.
:::

# Plan de refactorización del libro mayor Sora Nexus

Este documento captura el roadmap inmediato para el refactor del Sora Nexus Ledger ("Iroha 3"). Refleja el diseño actual del repositorio y las regresiones observadas en la contabilidad de genesis/WSV, el consenso Sumeragi, disparadores de contratos inteligentes, consultas de instantáneas, enlaces de host pointer-ABI y códecs Norito. El objetivo es converger en una arquitectura coherente y comprobable sin intentar aterrizar todas las correcciones en un único parche monolítico.## 0. Principios guía
- Preservar el comportamiento determinista en hardware heterogéneo; Usar aceleración solo con indicadores de características opcionales y respaldos idénticos.
- Norito es la capa de serialización. Cualquier cambio de estado/esquema debe incluir pruebas de codificación/decodificación de ida y vuelta Norito y actualizaciones de accesorios.
- La configuración fluye por `iroha_config` (usuario -> actual -> valores predeterminados). Eliminar alterna ad-hoc de entorno en rutas de producción.
- La política ABI sigue en V1 y no es negociable. Los hosts deben rechazar tipos de puntero/llamadas al sistema desconocidos de forma determinista.
- `cargo test --workspace` y los golden tests (`ivm`, `norito`, `integration_tests`) siguen siendo la compuerta base para cada hito.## 1. Instantánea de la topología del repositorio
- `crates/iroha_core`: actores Sumeragi, WSV, cargador de génesis, canalizaciones (query, overlay, zk lanes), pegamento del host de contratos inteligentes.
- `crates/iroha_data_model`: esquema autoritativo para datos y consultas on-chain.
- `crates/iroha`: API de cliente usada por CLI, pruebas, SDK.
- `crates/iroha_cli`: CLI de operadores, actualmente refleja numerosas API en `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, puntos de entrada de integración pointer-ABI del host.
- `crates/norito`: códec de serialización con adaptadores JSON y backends AoS/NCB.
- `integration_tests`: afirmaciones entre componentes que cubren genesis/bootstrap, Sumeragi, triggers, paginacion, etc.
- Los docs ya delinean metas del Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), pero la implementación esta fragmentada y parcialmente obsoleta respecto al código.

## 2. Pilares de refactor y hitos### Fase A - Fundaciones y observabilidad
1. **Telemetria WSV + Instantáneas**
   - Establecer una API canonica de snapshots en `state` (trait `WorldStateSnapshot`) usada por consultas, Sumeragi y CLI.
   - Usar `scripts/iroha_state_dump.sh` para producir instantáneas deterministas vía `iroha state dump --format norito`.
2. **Determinismo de Génesis/Bootstrap**
   - Refactorizar la ingesta de génesis para que pase por un único pipeline con Norito (`iroha_core::genesis`).
   - Agregar cobertura de integración/regresión que reprocesa génesis mas el primer bloque y afirma root WSV identicos entre arm64/x86_64 (seguido en `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Pruebas de fijación entre cajas**
   - Expandir `integration_tests/tests/genesis_json.rs` para validar invariantes de WSV, pipeline y ABI en un solo arnés.
   - Introducir un scaffold `cargo xtask check-shape` que haga pánico ante la deriva del esquema (seguido bajo el backlog de tooling DevEx; ver el action item en `scripts/xtask/README.md`).### Fase B - WSV y superficie de consultas
1. **Transacciones de almacenamiento estatal**
   - Colapsar `state/storage_transactions.rs` en un adaptador transaccional que imponga orden de commits y detección de conflictos.
   - Las pruebas unitarias ahora verifican que las modificaciones de activos/mundo/triggers hagan rollback ante fallos.
2. **Refactor del modelo de consultas**
   - Mover la lógica de paginación/cursor a componentes reutilizables bajo `crates/iroha_core/src/query/`. Representaciones alineales Norito en `iroha_data_model`.
   - Agregar consultas de instantáneas para activadores, activos y roles con orden determinista (seguido vía `crates/iroha_core/tests/snapshot_iterable.rs` para la cobertura actual).
3. **Consistencia de instantáneas**
   - Asegúrese de que `iroha ledger query` CLI use la misma ruta de instantánea que Sumeragi/fetchers.
   - Las pruebas de regresión de instantáneas en CLI viven en `tests/cli/state_snapshot.rs` (con función activada para ejecuciones lentas).### Fase C - Tubería Sumeragi
1. **Topología y gestión de épocas**
   - Extraer `EpochRosterProvider` a un rasgo con implementaciones respaldadas por instantáneas de participación en WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` ofrece un constructor simple y amigable para simulacros en bancos/pruebas.
2. **Simplificación del flujo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` en módulos: `pacemaker`, `aggregation`, `availability`, `witness` con tipos compartidos bajo `consensus`.
   - Reemplazar el message pass ad-hoc por sobres Norito tipados e introducir property tests de view-change (seguido en el backlog de mensajeria Sumeragi).
3. **Carril de integración/prueba**
   - Pruebas de carriles alineales con compromisos de DA y asegurar que RBC gating sea uniforme.
   - El test de integración de extremo a extremo `integration_tests/tests/extra_functional/seven_peer_consistency.rs` ahora verifica la ruta con RBC habilitado.### Fase D - Contratos inteligentes y hosts pointer-ABI
1. **Auditoria de límite del anfitrión**
   - Consolidar los checks de pointer-type (`ivm::pointer_abi`) y los adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - Las expectativas de pointer table y los enlaces de host manifest están cubiertos por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` y `ivm_host_mapping.rs`, que ejercitan los mapeos TLV golden.
2. **Sandbox de ejecución de triggers**
   - Refactorizar triggers para ejecutar vía un `TriggerExecutor` común que impone gas, validación de punteros y registro de eventos.
   - Agregar pruebas de regresión para triggers de call/time cubriendo paths de fallo (seguido vía `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alineación de CLI y cliente**
   - Asegúrese de que las operaciones CLI (`audit`, `gov`, `sumeragi`, `ivm`) dependan de las funciones compartidas de cliente `iroha` para evitar derivas.
   - Las pruebas de instantáneas JSON del CLI viven en `tests/cli/json_snapshot.rs`; mantenlos al día para que la salida de comandos siga coincidiendo con la referencia JSON canonica.### Fase E - Endurecimiento del códec Norito
1. **Registro de esquemas**
   - Crear un registro de esquema Norito bajo `crates/norito/src/schema/` para abastecer codificaciones canónicas de tipos core.
   - Se agregaron pruebas de documentos que verifican la codificación de cargas útiles de muestra (`norito::schema::SamplePayload`).
2. **Actualización de accesorios dorados**
   - Actualizar los accesorios dorados de `crates/norito/tests/*` para que coincidan con el nuevo esquema WSV cuando aterrice el refactor.
   - `scripts/norito_regen.sh` regenera los golden JSON de Norito de forma determinista a través del ayudante `norito_regen_goldens`.
3. **Integración IVM/Norito**
   - Validar la serialización de manifiestos Kotodama de extremo a extremo mediante Norito, asegurando que el puntero de metadatos ABI sea consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantiene la paridad Norito codificación/decodificación para manifiestos.## 3. Temas transversales
- **Estrategia de pruebas**: Cada fase promueve pruebas unitarias -> pruebas de cajas -> pruebas de integración. Los tests fallidos capturan regresiones actuales; los nuevos tests evitan que reaparezcan.
- **Documentación**: Tras cada fase, actualice `status.md` y mueva elementos abiertos a `roadmap.md` mientras se pueden completar tareas.
- **Benchmarks de rendimiento**: Mantener bancos existentes en `iroha_core`, `ivm` y `norito`; Agregar mediciones base post-refactor para validar que no hay regresiones.
- **Indicadores de funciones**: Mantener alterna un nivel de caja solo para backends que requieren cadenas de herramientas externas (`cuda`, `zk-verify-batch`). Los paths SIMD de CPU siempre se construyen y se seleccionan en tiempo de ejecución; proporcionar respaldos escalares deterministas para hardware no soportado.## 4. Acciones inmediatas
- Scaffolding de Fase A (snapshot trace + cableado de telemetria) - ver tareas accionables en actualizaciones del roadmap.
- La auditoria reciente de defectos para `sumeragi`, `state` e `ivm` revela los siguientes puntos:
  - `sumeragi`: asignaciones de código muerto protegen transmisión de pruebas de cambio de vista, estado de reproducción VRF y exportación de telemetría EMA. Estos permanecen cerrados hasta que la simplificacion del flujo de consenso de la Fase C y los entregables de integracion lane/proof aterricen.
  - `state`: la limpieza de `Cell` y el ruteo de telemetria pasan al track de telemetria WSV de la Fase A, mientras que las notas de SoA/parallel-apply se integran al backlog de optimización de pipeline de la Fase C.
  - `ivm`: la exposición de toggles CUDA, la validación de sobres y la cobertura Halo2/Metal se mapean al trabajo de host-boundary de la Fase D más el tema transversal de aceleración GPU; los kernels permanecen en el backlog dedicado de GPU hasta estar listos.
- Preparar un RFC cross-team que resume este plan para firmar antes de aterrizar cambios de código invasivos.## 5. Preguntas abiertas
- ¿Debe RBC seguir siendo opcional más allá de P1, o es obligatorio para los carriles del libro mayor Nexus? Requiere decisión de las partes interesadas.
- Impulsamos grupos de composabilidad DS en P1 o los mantenemos deshabilitados hasta que maduren las pruebas de carril?
- ¿Cuál es la ubicación canónica para los parámetros ML-DSA-87? Candidato: nuevo crate `crates/fastpq_isi` (creación pendiente).

---

_Última actualización: 2025-09-12_