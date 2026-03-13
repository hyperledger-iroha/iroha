---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de nodo
título: Plano de implementación del nodo SoraFS
sidebar_label: Plano de implementación del nodo
descripción: Convierta la hoja de ruta de almacenamiento SF-3 en trabajos de ingeniería accionaria con marcos, tarefas y cobertura de pruebas.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/sorafs_node_plan.md`. Mantenha ambas as copias sincronizadas ate que a documentacao Sphinx alternativa seja retirada.
:::

SF-3 entrega el primer crate ejecutable `sorafs-node` que transforma un proceso Iroha/Torii en un proveedor de almacenamiento SoraFS. Utilice este plano junto con la [guía de almacenamiento del nodo](node-storage.md), la [política de admisión de proveedores](provider-admission-policy.md) y la [hoja de ruta del mercado de capacidad de almacenamiento](storage-capacity-marketplace.md) para secuenciar entregas.

## Escopo alvo (Marco M1)1. **Integracao do chunk store.** Envolver `sorafs_car::ChunkStore` con un backend persistente que armazena bytes de chunk, manifests y arvores PoR no directorio de dados configurado.
2. **Puntos finales de puerta de enlace.** Exporte puntos finales HTTP Norito para enviar pin, recuperar fragmentos, mostrar PoR y telemetría de almacenamiento dentro del proceso Torii.
3. **Plomería de configuración.** Agregar una estructura de configuración `SoraFsStorage` (bandera habilitada, capacidad, directorios, límites de correspondencia) conectada a través de `iroha_config`, `iroha_core` e `iroha_torii`.
4. **Quota/agendamento.** Impor limites de disco/paralelismo definidos por el operador e enfileirar requisicoes com back-pression.
5. **Telemetría.** Emitir métricas/registros para el éxito de pin, latencia de recuperación de fragmentos, utilización de capacidad y resultados de demostración PoR.

## Quebra de trabajo

### A. Estructura de caja y módulos| Tarefa | Don(es) | Notas |
|------|---------|------|
| Criar `crates/sorafs_node` módulos com: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Equipo de almacenamiento | Reexporta tipos reutilizados para integrar con Torii. |
| Implementar `StorageConfig` mapeado de `SoraFsStorage` (usuario -> real -> valores predeterminados). | Equipo de almacenamiento/WG de configuración | Garante que as camadas Norito/`iroha_config` permanecam deterministicas. |
| Fornecer una fachada `NodeHandle` que Torii usa para pins/fetches submétricos. | Equipo de almacenamiento | Encapsula internos de almacenamiento y plomería async. |

### B. Tienda de fragmentos persistente

| Tarefa | Don(es) | Notas |
|------|---------|------|
| Construya un backend en disco involucrando `sorafs_car::ChunkStore` con índice de manifiesto en disco (`sled`/`sqlite`). | Equipo de almacenamiento | Diseño determinístico: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Manter metadados PoR (arvores 64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | Equipo de almacenamiento | Soporte de repetición y reinicio; falha rapido em corrupcao. |
| Implementar repetición de integridad en el inicio (refrito de manifiestos, podar pines incompletos). | Equipo de almacenamiento | Bloquea el inicio de Torii y termina la reproducción. |

### C. Puntos finales de puerta de enlace| Punto final | Compartimento | Tarefas |
|----------|--------------|--------|
| `POST /sorafs/pin` | Aceita `PinProposalV1`, valida manifests, enfileira ingestao, responde com o CID do manifest. | Validar perfil de fragmentador, importar cuotas, transmitir datos a través de la tienda de fragmentos. |
| `GET /sorafs/chunks/{cid}` + consulta de rango | Servir bytes de fragmentos con encabezados `Content-Chunker`; respecto a la especificación de capacidad de alcance. | Usar Scheduler + Orcamentos de Stream (ligar a capacidade de range SF-2d). |
| `POST /sorafs/por/sample` | Rodar amostragem PoR para um manifest e retornar bundle de prova. | Reutilizar amostragem do chunk store, respondedor con cargas útiles Norito JSON. |
| `GET /sorafs/telemetry` | Resumen: capacidade, sucesso PoR, contadores de error de fetch. | Fornecer dados para tableros/operadores. |

O plumbing em runtime passa as interacoes PoR via `sorafs_node::por`: o tracker registra cada `PorChallengeV1`, `PorProofV1` e `AuditVerdictV1` para que as métricas `CapacityMeter` reflitam vereditos degobernanca sem logica Torii a medida. [crates/sorafs_node/src/scheduler.rs:147]

Notas de implementación:

- Utilice la pila Axum de Torii con cargas útiles `norito::json`.
- Adición de esquemas Norito para respuestas (`PinResultV1`, `FetchErrorV1`, estructuras de telemetría).- `/v2/sorafs/por/ingestion/{manifest_digest_hex}` ahora expone la profundidad del trabajo pendiente, más la época/fecha límite, más antigua y las marcas de tiempo más recientes de éxito/falta por proveedor, a través de `sorafs_node::NodeHandle::por_ingestion_status`, e Torii registra los indicadores `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para tableros. [crates/sorafs_node/src/lib.rs:510] [crates/iroha_torii/src/sorafs/api.rs:1883] [crates/iroha_torii/src/routing.rs:7244] [crates/iroha_telemetry/src/metrics.rs:5390]

### D. Programador y cumplimiento de cuotas

| Tarefa | Detalles |
|------|---------|
| Cuota de discoteca | Rastrear bytes en discoteca; rejeitar novos pins ao exceder `max_capacity_bytes`. Fornecer ganchos de desalojo para políticas futuras. |
| Concordancia de fetch | Semaforo global (`max_parallel_fetches`) mais orcamentos por provedor oriundos de caps de range SF-2d. |
| Fila de alfileres | Limitar trabajos de ingestao pendientes; export endpoints Norito de estado para profundidad de fila. |
| Cadencia PoR | Trabajador de fondo dirigido por `por_sample_interval_secs`. |

### E. Telemetría y registro

Métricas (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histograma con etiquetas `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Registros/eventos:- Telemetria Norito estructurada para ingesta de gobierno (`StorageTelemetryV1`).
- Alertas cuando la utilización es > 90% o la racha de faltas PoR excede el umbral.

### F. Estrategia de testículos

1. **Pruebas unitarias.** Persistencia del almacén de fragmentos, cálculos de cuota, invariantes del planificador (ver `crates/sorafs_node/src/scheduler.rs`).
2. **Testículos de integración** (`crates/sorafs_node/tests`). Pin -> buscar viaje de ida y vuelta, reinicio de recuperación, rechazo por cuota, verificación de pruebas de amostragem PoR.
3. **Tests de integracao Torii.** Rodar Torii con almacenamiento habilitado, ejercitar puntos finales HTTP a través de `assert_cmd`.
4. **Roadmap de caos.** Drills futuros simulam exaustao de disco, IO lento, remocao de provedores.

## Dependencias

- Política de admisión SF-2b - garantizar que los nodos verifiquen los sobres de admisión antes de anunciar.
- Marketplace de capacidade SF-2c - ligar telemetria de volta a declaracoes de capacidade.
- Extensoes de advert SF-2d - consume capacidade de range + orcamentos de stream quando disponiveis.

## Criterios de saya do marco- `cargo run -p sorafs_node --example pin_fetch` funciona contra accesorios ubicados.
- Torii compila con `--features sorafs-storage` y pasa pruebas de integración.
- Documentación ([guía de almacenamiento del nodo](node-storage.md)) actualizada con valores predeterminados de configuración + ejemplos de CLI; runbook de operador disponible.
- Telemetría visual en paneles de puesta en escena; alertas configuradas para saturacao de capacidade e falhas PoR.

## Entregaveis de documentacao e ops

- Actualizar a [referencia de almacenamiento del nodo](node-storage.md) con los valores predeterminados de configuración, uso de CLI y pasos de solución de problemas.
- Manter o [runbook de operacoes de nodo](node-operations.md) alinhado com a implementacao conforme SF-3 evolui.
- Publicar referencias de API para endpoints `/sorafs/*` dentro del portal del desarrollador y conectar al manifiesto OpenAPI así como los controladores de Torii estiverem pronto.