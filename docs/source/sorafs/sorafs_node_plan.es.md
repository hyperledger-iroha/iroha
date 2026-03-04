---
lang: es
direction: ltr
source: docs/source/sorafs/sorafs_node_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a42f39287fcb33f421b891f15ac9aef28a57826719ccfb388bd6eaa062a7a199
source_last_modified: "2026-01-03T18:07:58.370260+00:00"
translation_last_reviewed: 2026-01-30
---

# Plan de implementación del prototipo de nodo SoraFS (SF-3)

SF-3 entrega el primer crate ejecutable `sorafs-node` que convierte un proceso
Iroha/Torii en un proveedor de almacenamiento SoraFS. Este plan traduce el
diseño de almacenamiento de alto nivel en tareas de ingeniería concretas,
hitos y cobertura de pruebas. Debe usarse junto con
`sorafs_node_storage.md`, la política de admisión de proveedores y la hoja de
ruta del marketplace de capacidad.

> **Portal:** reflejado en `docs/portal/docs/sorafs/node-plan.md`. Actualice
> ambas copias para mantener a los revisores alineados.

## Alcance objetivo (Hito M1)

1. **Integración del chunk store**: envolver `sorafs_car::ChunkStore` con un
   backend persistente que almacene bytes de chunk, manifiestos y árboles PoR en
   el directorio de datos configurado.
2. **Endpoints del gateway**: exponer endpoints HTTP Norito para envío de pins,
   fetch de chunks, muestreo PoR y telemetría de almacenamiento dentro del
   proceso Torii.
3. **Cableado de configuración**: añadir la estructura de config `SoraFsStorage`
   (habilitado, capacidad, directorios, límites de concurrencia) conectada a
   `iroha_config`, `iroha_core` e `iroha_torii`.
4. **Cuotas/planificación**: aplicar límites de disco/paralelismo definidos por
   el operador y encolar solicitudes con back-pressure.
5. **Telemetría**: emitir métricas/logs para éxito de pins, latencia de fetch de
   chunks, utilización de capacidad y resultados de muestreo PoR.

## Desglose de trabajo

### A. Estructura de crate y módulos

| Tarea | Responsable(s) | Notas |
|------|----------------|-------|
| Crear `crates/sorafs_node` con módulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Storage Team | Re-exportar tipos reutilizables para la integración con Torii. |
| Implementar `StorageConfig` mapeado desde `SoraFsStorage` (actual/default/user). | Storage Team / Config WG | Garantizar paridad serde/Norito y overrides por entorno. |
| Proveer un facade `NodeHandle` que Torii use para enviar pins/fetches. | Storage Team | Encapsular los internos de almacenamiento. |

### B. Chunk store persistente

| Tarea | Responsable(s) | Notas |
|------|----------------|-------|
| Construir backend en disco envolviendo `sorafs_car::ChunkStore` con un índice de manifiestos on-disk (sled/sqlite?). | Storage Team | Layout determinista: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Mantener metadatos PoR (árboles 64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | Storage Team | Soportar reanudación tras reinicio. |
| Implementar replay de integridad al inicio (rehash de entradas de manifiesto, poda de pins incompletos). | Storage Team | Fallar rápido si se detecta corrupción. |

### C. Endpoints del gateway

| Endpoint | Comportamiento | Tareas |
|----------|----------------|--------|
| `POST /sorafs/pin` | Acepta `PinProposalV1`, valida manifiestos, encola la ingesta, responde con el CID del manifiesto. | Validar perfil de chunk, aplicar cuotas, transmitir datos vía chunk store. |
| `GET /sorafs/chunks/{cid}` + query de rango | Sirve bytes de chunk con headers `Content-Chunker`; respeta la especificación de capacidad range. | Usar scheduler + presupuestos de streams (vinculado a SF-2d). |
| `POST /sorafs/por/sample` | Ejecuta muestreo PoR para un manifiesto y devuelve el bundle de prueba. | Reutilizar muestreo del chunk store, responder con JSON Norito. |
| `GET /sorafs/telemetry` | Resúmenes: capacidad, éxito de PoR, conteos de errores de fetch. | Proveer datos para dashboards/operadores. |

El runtime ahora encamina estas interacciones PoR mediante `sorafs_node::por`: el
tracker registra cada `PorChallengeV1`, `PorProofV1` y `AuditVerdictV1` para que
el `CapacityMeter` y las métricas del scheduler reflejen los veredictos de
la gobernanza sin cableado a medida en Torii.

Notas de implementación:
- Use Axum (el stack de Torii) con `norito::json` para los payloads.
- Añada esquemas Norito para respuestas (p. ej., `PinResultV1`, `FetchErrorV1`).

### D. Planificador y aplicación de cuotas

| Tarea | Detalles |
|------|----------|
| Cuota de disco | Seguir bytes en disco; rechazar nuevos pins al superar `max_capacity_bytes`. Proveer hooks de expulsión para políticas futuras. |
| Concurrencia de fetch | Semáforo global (`max_parallel_fetches`) + presupuestos por proveedor (de SF-2d). |
| Cola de pins | Limitar trabajos de ingesta pendientes; proveer endpoint de estado Norito. |
| Cadencia PoR | Worker en background disparado por `por_sample_interval_secs`. |

### E. Telemetría y logging

Métricas (Prometheus):
- `sorafs_pin_success_total`, `sorafs_pin_failure_total`.
- `sorafs_chunk_fetch_duration_seconds` (histograma con etiquetas `result`).
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`.
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`.
- `torii_sorafs_storage_fetch_bytes_per_sec`.
- `torii_sorafs_storage_por_inflight`.
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`.

La implementación inicial del runtime ahora respalda estos gauges mediante
`StorageSchedulersRuntime`, que aplica los presupuestos de concurrencia de
pin/fetch/PoR y agrega estadísticas de throughput/colas para que Torii las
exponga vía Prometheus.【crates/sorafs_node/src/scheduler.rs:147】

Logs / eventos:
- Telemetría Norito estructurada para ingestión de gobernanza (`StorageTelemetryV1`).
- Ingesta de telemetría de capacidad con control de gobernanza (remitentes autorizados + nonces) que limita ventanas a la capacidad declarada, rechaza payloads de capacidad cero, aplica ventanas monótonas con límites de brecha/antireplay y emite métricas de rechazo antes del manejo de fees/strikes.【crates/iroha_core/src/smartcontracts/isi/sorafs.rs】【crates/iroha_config/src/parameters/{actual,user}.rs】【crates/iroha_telemetry/src/metrics.rs】
- Alertas cuando la utilización > 90% o la racha de fallos PoR supera el umbral.

### F. Estrategia de pruebas

1. **Unit tests**: persistencia del chunk store, cálculos de cuotas, scheduler
   (ver `crates/sorafs_node/src/scheduler.rs` para cobertura de colas y rate-limit).
2. **Integration tests** (nuevo `crates/sorafs_node/tests`):
   - Pin → fetch round trip usando manifiesto/plan de fixtures.
   - Recuperación tras reinicio: pin, reinicio, verificar registro de manifiesto.
   - Rechazo por cuota: definir capacidad baja, intentar pin adicional.
   - Endpoint de muestreo PoR verificando que la prueba coincide con la raíz del chunk store.
3. **Torii integration tests**: ejecutar Torii con almacenamiento habilitado,
   ejercer endpoints HTTP usando `assert_cmd`.
4. **Chaos tests (futuro)**: simular agotamiento de disco, IO lento, eliminación
   de proveedor (seguido en hitos posteriores).

### Dependencias

- Política de admisión SF-2b (verificador de proveedores) — asegurar que el nodo
  comprueba los sobres de admisión antes de anunciarse.
- Marketplace de capacidad SF-2c — más adelante vincular telemetría de
  almacenamiento a las declaraciones de capacidad.
- Extensiones de advert SF-2d — consumir capacidad range + presupuestos de
  streams cuando estén disponibles.

### Criterios de salida del hito

- `cargo run -p sorafs_node --example pin_fetch` funciona contra fixtures locales.
- Torii compila con `--features sorafs-storage` y pasa las pruebas de integración.
- La documentación (`sorafs_node_storage.md`) se actualiza para reflejar la
  implementación; se redacta la guía del operador.
- Telemetría visible en dashboards de staging; alertas configuradas para
  saturación de capacidad y fallos PoR.

## Tareas de integración restantes (enfoque M2)

| Tarea | Descripción | Dependencias |
|------|-------------|--------------|
| Worker de ingesta PoR | Expandir `NodeHandle::ingest_por_proof` para aceptar pruebas en streaming, persistirlas en `PorCoordinatorRuntime::storage`, y exponer un endpoint de estado Norito (`/v1/sorafs/por/ingestion`). | `crates/iroha_torii/src/sorafs/por.rs`, `crates/sorafs_node/src/por.rs`. |
| Cableado de cola de desafíos | Suscribirse a eventos del coordinador emitidos por `PorCoordinatorRuntime::run_epoch`, distribuir desafíos a workers de storage, y asegurar que los reintentos sean idempotentes entre reinicios. | Hooks de wiring de runtime introducidos en `PorCoordinatorRuntime`. |
| Telemetría de gobernanza | Emitir métricas `sorafs_por_ingest_backlog` + `sorafs_por_ingest_failures_total`, conectarlas a los dashboards del gateway y documentar umbrales de alerta en `docs/source/sorafs_observability_plan.md`. | Plan de observabilidad + `crates/iroha_telemetry`. |
| Herramientas de operador | Añadir un helper `sorafs-node ingest por --manifest <cid>` y actualizaciones de runbook para que los operadores puedan reproducir pruebas localmente antes de enviarlas. | Añadidos de CLI en `crates/sorafs_node/src/bin/sorafs-node.rs`. |

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` ahora delega en
  `sorafs_node::NodeHandle::por_ingestion_status`, devolviendo profundidad de
  backlog, el epoch/límite más antiguo y los timestamps de éxito/fracaso más
  recientes por proveedor mientras Torii actualiza
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para
  que los dashboards sigan manifestos atascados automáticamente.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】
- ✅ `sorafs-node ingest por` ahora reproduce desafíos PoR, pruebas y veredictos
  opcionales contra el worker de storage embebido, emitiendo resúmenes JSON para
  que los operadores validen artefactos y archiven evidencia antes de llamar a
  la API HTTP. Las pruebas de regresión cubren el nuevo flujo y los runbooks/
  docs del portal describen el flujo de trabajo para SREs que preparan tickets
  de gobernanza.【crates/sorafs_node/src/bin/sorafs-node.rs:184】【crates/sorafs_node/tests/cli.rs:103】【docs/source/sorafs/runbooks/sorafs_node_ops.md:57】【docs/portal/docs/sorafs/node-operations.md:59】

Estos ítems mantienen SF‑3 alineado con SF‑9 (automatización PoR) y se siguen en
`roadmap.md` como parte del llamado “deliver SoraFS tasks” de marzo.

## Entregables de documentación y ops

- Actualizar `docs/source/sorafs/sorafs_node_storage.md` con valores por defecto
  de configuración y ejemplos de CLI.
- Crear el runbook del operador (`docs/source/sorafs/runbooks/sorafs_node_ops.md`)
  que cubra despliegue, monitoreo y troubleshooting.
- Publicar la referencia API para los nuevos endpoints en el portal de docs.

El progreso debe reflejarse en el roadmap marcando los ítems de la sección SF-3
conforme las funcionalidades aterricen.
