---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de nodo
título: Plan de implementación del nodo SoraFS
sidebar_label: plan de implementación del nodo
descripción: Hoja de ruta de almacenamiento SF-3 کو hitos, tareas اور cobertura de prueba کے ساتھ trabajo de ingeniería procesable میں تبدیل کرتا ہے۔
---

:::nota مستند ماخذ
:::

SF-3 caja ejecutable `sorafs-node` caja de almacenamiento Iroha/Torii proceso y proveedor de almacenamiento SoraFS caja de almacenamiento اس پلان کو [guía de almacenamiento de nodos](node-storage.md) ، [política de admisión de proveedores](provider-admission-policy.md) اور [hoja de ruta del mercado de capacidad de almacenamiento](storage-capacity-marketplace.md) کے ساتھ استعمال کریں جب secuencia de entregables کریں۔

## Alcance objetivo (Milestone M1)1. **Integración de almacén de fragmentos.** `sorafs_car::ChunkStore` Incluye backend persistente, envoltura, directorio de datos configurado, bytes de fragmentos, manifiestos y árboles PoR.
2. **Puntos finales de puerta de enlace.** Proceso Torii کے اندر envío de pin, búsqueda de fragmentos, muestreo PoR اور telemetría de almacenamiento کے لیے Los puntos finales HTTP Norito exponen کریں۔
3. **Plomería de configuración.** Estructura de configuración `SoraFsStorage` (bandera habilitada, capacidad, directorios, límites de concurrencia) ذریعے alambre کریں۔
4. **Cuota/programación.** Los límites de disco/paralelismo definidos por el operador imponen solicitudes y contrapresión y colas.
5. **Telemetría.** éxito del pin, latencia de recuperación de fragmentos, utilización de la capacidad, resultados de muestreo de PoR, emisión de métricas/registros

## Desglose del trabajo

### A. Estructura de caja y módulo| Tarea | Propietario(s) | Notas |
|------|----------|-------|
| `crates/sorafs_node` Módulos de computadora negros `config`, `store`, `gateway`, `scheduler`, `telemetry` Módulos ہوں۔ | Equipo de almacenamiento | Torii integración کے لیے tipos reutilizables reexportación کریں۔ |
| `StorageConfig` implementa کریں جو `SoraFsStorage` سے mapeado ہو (usuario → real → valores predeterminados) ۔ | Equipo de almacenamiento/WG de configuración | Norito/`iroha_config` capas کو determinista رکھیں۔ |
| Torii کے pins/fetches کے لیے `NodeHandle` fachada فراہم کریں۔ | Equipo de almacenamiento | componentes internos de almacenamiento y encapsulación de plomería asíncrona |

### B. Almacenamiento de fragmentos persistente

| Tarea | Propietario(s) | Notas |
|------|----------|-------|
| `sorafs_car::ChunkStore` کو índice de manifiesto en disco (`sled`/`sqlite`) کے ساتھ backend de disco میں wrap کریں۔ | Equipo de almacenamiento | Diseño determinista: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| `ChunkStore::sample_leaves` کے ذریعے Los metadatos PoR (árboles de 64 KiB/4 KiB) mantienen کریں۔ | Equipo de almacenamiento | Reiniciar کے بعد soporte de reproducción؛ corrupción پر fracasar rápido۔ |
| Implementación de repetición de integridad de inicio کریں (repetición de manifiestos, poda de pines incompletos) ۔ | Equipo de almacenamiento | repetición مکمل ہونے تک Torii bloque de inicio کریں۔ |

### C. Puntos finales de puerta de enlace| Punto final | Comportamiento | Tareas |
|----------|-----------|-------|
| `POST /sorafs/pin` | `PinProposalV1` قبول کریں، manifiestos validan کریں، cola de ingestión کریں، manifiesto CID y دیں۔ | perfil de fragmento validar cuotas hacer cumplir almacén de fragmentos flujo de datos |
| `GET /sorafs/chunks/{cid}` + consulta de rango | `Content-Chunker` encabezados کے ساتھ fragmentos de bytes sirven کریں؛ rango capacidad especificación respeto کریں۔ | planificador + presupuestos de flujo استعمال کریں (capacidad de alcance SF-2d کے ساتھ tie کریں)۔ |
| `POST /sorafs/por/sample` | manifiesto کے لیے muestreo PoR چلائیں اور paquete de prueba واپس کریں۔ | Reutilización de muestreo de almacén de fragmentos کریں، Norito Cargas útiles JSON کے ساتھ responder کریں۔ |
| `GET /sorafs/telemetry` | Resúmenes: capacidad, éxito de PoR, recuentos de errores de recuperación۔ | paneles/operadores کے لیے datos فراہم کریں۔ |

Plomería en tiempo de ejecución `sorafs_node::por` کے ذریعے Interacciones PoR کو hilo کرتی ہے: rastreador ہر `PorChallengeV1`, `PorProofV1`, `AuditVerdictV1` کو registro کرتا ہے Veredictos de gobernanza de métricas `CapacityMeter` y lógica específica de Torii reflejan کریں۔【crates/sorafs_node/src/scheduler.rs#L147】

Notas de implementación:

- Torii کے Pila de Axum کو `norito::json` cargas útiles کے ساتھ استعمال کریں۔
- respuestas کے لیے Norito esquemas شامل کریں (`PinResultV1`, `FetchErrorV1`, estructuras de telemetría) ۔- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` اب profundidad de trabajo pendiente کے ساتھ época/fecha límite más antigua اور ہر proveedor کے marcas de tiempo recientes de éxito/fracaso دکھاتا ہے، جو `sorafs_node::NodeHandle::por_ingestion_status` سے alimentado ہے، اور Torii tableros de instrumentos کے لیے `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` registro de medidores کرتا ہے۔【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Programador y aplicación de cuotas

| Tarea | Detalles |
|------|---------|
| Cuota de disco | Seguimiento de bytes de disco کریں؛ `max_capacity_bytes` پر نئے pines rechazan کریں۔ مستقبل کی políticas کے لیے ganchos de desalojo فراہم کریں۔ |
| Obtener concurrencia | Semáforo global (`max_parallel_fetches`) کے ساتھ presupuestos por proveedor جو límites de rango SF-2d سے آتے ہیں۔ |
| Cola de pines | Trabajos de ingesta destacados محدود کریں؛ profundidad de la cola کے لیے Norito los puntos finales de estado exponen کریں۔ |
| Cadencia PoR | `por_sample_interval_secs` سے چلنے y trabajador en segundo plano ۔ |

### E. Telemetría y registro

Métricas (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histograma con etiquetas `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Registros/eventos:

- ingesta de gobernanza کے لیے telemetría estructurada Norito (`StorageTelemetryV1`).
- utilización > 90 % یا Umbral de racha de fallos de PoR سے اوپر ہو تو alertas۔### F. Estrategia de prueba

1. **Pruebas unitarias.** Persistencia del almacén de fragmentos, cálculos de cuotas, invariantes del programador (consulte `crates/sorafs_node/src/scheduler.rs`).
2. **Pruebas de integración** (`crates/sorafs_node/tests`). Pin → buscar viaje de ida y vuelta, reiniciar recuperación, rechazo de cuota, verificación de prueba de muestreo PoR۔
3. **Pruebas de integración Torii.** Torii کو almacenamiento habilitado کے ساتھ چلائیں، Puntos finales HTTP کو `assert_cmd` کے ذریعے ejercicio کریں۔
4. **Hoja de ruta del caos.** مستقبل کے perfora el agotamiento del disco, IO lento y la eliminación del proveedor simula کریں گے۔

## Dependencias

- Política de admisión SF-2b: los nodos کو publican anuncios کرنے سے پہلے sobres de admisión verifican کرنے ہوں گے۔
- Mercado de capacidad SF-2c: telemetría کو declaraciones de capacidad کے ساتھ tie کریں۔
- Extensiones de anuncios SF-2d: capacidad de alcance + presupuestos de transmisión دستیاب ہونے پر consumir کریں۔

## Criterios de salida de hitos

- `cargo run -p sorafs_node --example pin_fetch` accesorios locales کے خلاف کام کرے۔
- Torii `--features sorafs-storage` کے ساتھ build ہو اور pruebas de integración پاس کرے۔
- Documentación ([guía de almacenamiento de nodos](node-storage.md)) valores de configuración predeterminados + ejemplos de CLI کے ساتھ actualizado ہو؛ runbook del operador دستیاب ہو۔
- Paneles de preparación de telemetría میں نظر آئے؛ saturación de capacidad اور fallas de PoR کے لیے alertas configurar ہوں۔

## Entregables de documentación y operaciones- [referencia de almacenamiento de nodo](node-storage.md) Configuración predeterminada, uso de CLI y pasos para la solución de problemas y actualización
- [runbook de operaciones de nodo](node-operations.md) کو implementación کے ساتھ align رکھیں جیسے SF-3 evolucionar ہو۔
- Puntos finales `/sorafs/*` Portal de desarrolladores de referencias de API Publicación de controladores Torii Manifestación de cables OpenAPI