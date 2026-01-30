---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f5711b403ebff93327cfd2896d03635cb12413e0a7166e5faea26c05bf6cedb
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: node-plan
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fuente canónica
Esta página refleja `docs/source/sorafs/sorafs_node_plan.md`. Mantén ambas copias sincronizadas hasta que la documentación heredada de Sphinx se retire.
:::

SF-3 entrega el primer crate ejecutable `sorafs-node` que convierte un proceso Iroha/Torii en un proveedor de almacenamiento SoraFS. Usa este plan junto con la [guía de almacenamiento del nodo](node-storage.md), la [política de admisión de proveedores](provider-admission-policy.md) y la [hoja de ruta del marketplace de capacidad de almacenamiento](storage-capacity-marketplace.md) al secuenciar entregables.

## Alcance objetivo (Hito M1)

1. **Integración del almacén de chunks.** Envuelve `sorafs_car::ChunkStore` con un backend persistente que guarde bytes de chunk, manifests y árboles PoR en el directorio de datos configurado.
2. **Endpoints de gateway.** Expone endpoints HTTP Norito para envío de pins, fetch de chunks, muestreo PoR y telemetría de almacenamiento dentro del proceso Torii.
3. **Plomería de configuración.** Agrega una estructura de config `SoraFsStorage` (flag habilitado, capacidad, directorios, límites de concurrencia) cableada a través de `iroha_config`, `iroha_core` y `iroha_torii`.
4. **Cuotas/planificación.** Impone límites de disco/paralelismo definidos por el operador y encola solicitudes con back-pressure.
5. **Telemetría.** Emite métricas/logs para éxito de pins, latencia de fetch de chunks, utilización de capacidad y resultados de muestreo PoR.

## Desglose del trabajo

### A. Estructura de crate y módulos

| Tarea | Responsable(s) | Notas |
|------|---------------|------|
| Crear `crates/sorafs_node` con módulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Equipo de Storage | Reexporta tipos reutilizables para integración con Torii. |
| Implementar `StorageConfig` mapeado desde `SoraFsStorage` (usuario → real → defaults). | Equipo de Storage / Config WG | Asegura que las capas Norito/`iroha_config` permanezcan deterministas. |
| Proveer una fachada `NodeHandle` que Torii use para enviar pins/fetches. | Equipo de Storage | Encapsula internos de almacenamiento y plomería async. |

### B. Almacén de chunks persistente

| Tarea | Responsable(s) | Notas |
|------|---------------|------|
| Construir un backend en disco que envuelva `sorafs_car::ChunkStore` con un índice de manifests en disco (`sled`/`sqlite`). | Equipo de Storage | Layout determinista: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Mantener metadatos PoR (árboles de 64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | Equipo de Storage | Soporta replay tras reinicios; falla rápido ante corrupción. |
| Implementar replay de integridad al inicio (rehash de manifests, podar pins incompletos). | Equipo de Storage | Bloquea el arranque de Torii hasta completar el replay. |

### C. Endpoints de gateway

| Endpoint | Comportamiento | Tareas |
|----------|----------------|--------|
| `POST /sorafs/pin` | Acepta `PinProposalV1`, valida manifests, encola la ingestión, responde con el CID del manifest. | Valida el perfil de chunker, impone cuotas, streamea datos vía chunk store. |
| `GET /sorafs/chunks/{cid}` + consulta por rango | Sirve bytes de chunk con headers `Content-Chunker`; respeta la especificación de capacidad de rango. | Usa scheduler + presupuestos de stream (vincular a capacidad de rango SF-2d). |
| `POST /sorafs/por/sample` | Ejecuta muestreo PoR para un manifest y devuelve bundle de pruebas. | Reusa el muestreo del chunk store, responde con payloads Norito JSON. |
| `GET /sorafs/telemetry` | Resúmenes: capacidad, éxito de PoR, conteos de errores de fetch. | Proporciona datos para dashboards/operadores. |

La plomería en runtime enlaza las interacciones PoR a través de `sorafs_node::por`: el tracker registra cada `PorChallengeV1`, `PorProofV1` y `AuditVerdictV1` para que las métricas de `CapacityMeter` reflejen los veredictos de gobernanza sin lógica Torii personalizada.【crates/sorafs_node/src/scheduler.rs#L147】

Notas de implementación:

- Usa el stack Axum de Torii con payloads `norito::json`.
- Agrega esquemas Norito para respuestas (`PinResultV1`, `FetchErrorV1`, estructuras de telemetría).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` ahora expone la profundidad del backlog más la época/límite más antiguos y los timestamps de éxito/fallo más recientes por proveedor, impulsado por `sorafs_node::NodeHandle::por_ingestion_status`, y Torii registra los gauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para dashboards.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Scheduler y cumplimiento de cuotas

| Tarea | Detalles |
|------|----------|
| Cuota de disco | Seguimiento de bytes en disco; rechaza nuevos pins al superar `max_capacity_bytes`. Provee hooks de expulsión para políticas futuras. |
| Concurrencia de fetch | Semáforo global (`max_parallel_fetches`) más presupuestos por proveedor obtenidos de los límites de rango SF-2d. |
| Cola de pins | Limita los trabajos de ingestión pendientes; expone endpoints Norito de estado para profundidad de cola. |
| Cadencia PoR | Worker en segundo plano impulsado por `por_sample_interval_secs`. |

### E. Telemetría y logging

Métricas (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histograma con etiquetas `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Logs / eventos:

- Telemetría Norito estructurada para ingestión de gobernanza (`StorageTelemetryV1`).
- Alertas cuando la utilización > 90% o la racha de fallos PoR supera el umbral.

### F. Estrategia de pruebas

1. **Pruebas unitarias.** Persistencia del chunk store, cálculos de cuota, invariantes del scheduler (ver `crates/sorafs_node/src/scheduler.rs`).
2. **Pruebas de integración** (`crates/sorafs_node/tests`). Pin → fetch round trip, recuperación tras reinicio, rechazo por cuota, verificación de pruebas de muestreo PoR.
3. **Pruebas de integración de Torii.** Ejecuta Torii con almacenamiento habilitado, ejercita endpoints HTTP vía `assert_cmd`.
4. **Hoja de ruta de caos.** Futuros drills simulan agotamiento de disco, IO lento, retiro de proveedores.

## Dependencias

- Política de admisión SF-2b — asegurar que los nodos verifiquen envelopes de admisión antes de anunciarse.
- Marketplace de capacidad SF-2c — vincular la telemetría de vuelta a las declaraciones de capacidad.
- Extensiones de advert SF-2d — consumir capacidad de rango + presupuestos de stream cuando estén disponibles.

## Criterios de salida del hito

- `cargo run -p sorafs_node --example pin_fetch` funciona contra fixtures locales.
- Torii compila con `--features sorafs-storage` y pasa pruebas de integración.
- Documentación ([guía de almacenamiento del nodo](node-storage.md)) actualizada con defaults de configuración + ejemplos de CLI; runbook de operador disponible.
- Telemetría visible en dashboards de staging; alertas configuradas para saturación de capacidad y fallos PoR.

## Entregables de documentación y operaciones

- Actualizar la [referencia de almacenamiento del nodo](node-storage.md) con defaults de configuración, uso de CLI y pasos de troubleshooting.
- Mantener el [runbook de operaciones de nodo](node-operations.md) alineado con la implementación conforme evoluciona SF-3.
- Publicar referencias de API para endpoints `/sorafs/*` dentro del portal de desarrolladores y conectarlas al manifiesto OpenAPI una vez que los handlers de Torii estén listos.
