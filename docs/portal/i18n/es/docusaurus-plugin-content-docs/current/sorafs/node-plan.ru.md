---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de nodo
título: Plan de realización del uso SoraFS
sidebar_label: Plan de realización del uso
descripción: Pruebe el cartón de la impresora SF-3 en un robot integrado con piezas, accesorios y piezas de repuesto.
---

:::nota Канонический источник
:::

SF-3 поставляет первый исполняемый crate `sorafs-node`, который превращает процесс Iroha/Torii в провайдера хранения SoraFS. Utilice este plan para adaptarse a [gaidom по хранилищу узла](node-storage.md), [politicy допуска провайдеров](provider-admission-policy.md) y [дорожной картой маркетплейса емкости хранения](storage-capacity-marketplace.md) при выстраивании последовательности работ.

## Целевой объем (веха M1)1. **Integración de la tienda de fragmentos.** Introduzca `sorafs_car::ChunkStore` en el backend permanente, donde se encuentran los archivos, los manifiestos y el PoR-деревья en la actualidad. directores de dannyх.
2. **Puerta de enlace.** Abra Norito Dispositivos HTTP para enviar pin, buscar canales, muestreo PoR y canales de datos del proceso Torii.
3. **Прокладка конфигурации.** Добавить структуру конфигурации `SoraFsStorage` (flag включения, емкость, director, limиты конкурентности), consulte `iroha_config`, `iroha_core` y `iroha_torii`.
4. **Квоты/планирование.** Presionar los límites del disco/paralelizar por parte del operador y mantener los dispositivos con contrapresión.
5. **Medición.** Emite métricas/logotipos mediante pin, búsqueda de archivos, archivos de almacenamiento y muestreo de PoR.

## Разбиение работ

### A. Структура крейта и модулей| Задача | Ответственный(е) | Примечания |
|------|------------------|------------|
| Conecte `crates/sorafs_node` con módulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Equipo de almacenamiento | Reduzca los tipos de configuración necesarios para la integración con Torii. |
| Realice `StorageConfig`, compatible con `SoraFsStorage` (usuario → real → valores predeterminados). | Equipo de almacenamiento/WG de configuración | Garantice la configuración de las ranuras Norito/`iroha_config`. |
| Antes de la fase `NodeHandle`, el código Torii se utiliza para pines/fetches. | Equipo de almacenamiento | Incapsulirovat внутренности хранения and async-provodku. |

### B. Tienda de trozos persistentes

| Задача | Ответственный(е) | Примечания |
|------|------------------|------------|
| Actualice el backend del disco `sorafs_car::ChunkStore` con el indicador en disco (`sled`/`sqlite`). | Equipo de almacenamiento | Diseño predeterminado: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Coloque el metadato PoR (64 KiB/4 KiB) según `ChunkStore::sample_leaves`. | Equipo de almacenamiento | Puede reproducir la reproducción después del reinicio; fallar rápido при коррупции. |
| Realice la repetición de integridad al inicio (refrito de манифестов, чистка незавершенных pins). | Equipo de almacenamiento | Bloquee el inicio Torii para guardar la reproducción. |

### C. Эндпоинты puerta de enlace| Punto | Поведение | Задачи |
|---------|-----------|--------|
| `POST /sorafs/pin` | Принимает `PinProposalV1`, валидирует манифесты, ставит ingestion в очередь, отвечает CID manifesta. | Valide el perfil de chunker, introduzca las ventas, strimita esta tienda de trozos. |
| `GET /sorafs/chunks/{cid}` + consulta de rango | Отдает байты чанка с заголовками `Content-Chunker`; соблюдает спецификацию capacidad de alcance. | Использовать planificador + бюджеты стрима (связать с Capacidad de alcance SF-2d). |
| `POST /sorafs/por/sample` | Запускает PoR sampling для манифеста и возвращает paquete de prueba. | Antes de almacenar fragmentos de muestreo, elimine las cargas útiles JSON Norito. |
| `GET /sorafs/telemetry` | Сводки: емкость, успех PoR, счетчики ошибок fetch. | Previamente a los administradores/operadores. |

El programa de instalación de PoR está configurado con `sorafs_node::por`: трекер фиксирует каждый `PorChallengeV1`, `PorProofV1` y `AuditVerdictV1`, чтобы метрики `CapacityMeter` отражали вердикты gobernancia без отдельной логики Torii.【crates/sorafs_node/src/scheduler.rs#L147】

Principales realizaciones:

- Использовать Axum стек Torii с `norito::json` cargas útiles.
- Добавить Norito схемы ответов (`PinResultV1`, `FetchErrorV1`, estructuras de telemetría).- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` muestra la acumulación de trabajos pendientes, la época/fecha límite más recientes y las marcas de tiempo de éxito/fracaso en la lista de verificación. `sorafs_node::NodeHandle::por_ingestion_status`, а Torii métricas ficticias `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para 【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Programador y применение квот

| Задача | Detalles |
|------|--------|
| Дисковая квота | Отслеживать байты на диске; отклонять новые pines при превышении `max_capacity_bytes`. Подготовить хуки эвикции для будущих политик. |
| Конкурентность buscar | Semáforo global (`max_parallel_fetches`) más combinaciones por proveedor en el rango SF-2d. |
| Очередь alfileres | Лимитировать незавершенные trabajos de ingestión; Consulte el estado Norito de los componentes de las glubinas. |
| Каденция PoR | Фоновый воркер, управляемый `por_sample_interval_secs`. |

### E. Telemetría y registro

Métricas (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (гистограмма с метками `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Логи / события:

- Telemetro estructural Norito para la ingestión de gobernanza (`StorageTelemetryV1`).
- Alertas de aplicaciones > 90% o de versiones anteriores de la serie PoR-ошибок.### F. Prueba de estrategia

1. **Юнит-тесты.** Персистентность almacén de fragmentos, вычисления квот, инварианты planificador (см. `crates/sorafs_node/src/scheduler.rs`).
2. **Pruebas integrales** (`crates/sorafs_node/tests`). Pin → buscar viaje de ida y vuelta, восстановление после рестарта, отказ по квоте, проверка PoR prueba de muestreo.
3. **Pruebas integradas Torii.** Introduzca Torii en canales exclusivos y pronuncie archivos HTTP. `assert_cmd`.
4. **Caos en la hoja de ruta.** Будущие simulacros моделируют исчерпание диска, медленный IO, удаление провайдеров.

## Зависимости

- Политика допуска SF-2b — убедиться, что узлы проверяют sobres de admisión para anuncios publicitarios.
- Marcas comerciales SF-2c: conecte el televisor a las etiquetas adhesivas.
- Anuncio de Расширения SF-2d: permite ampliar la capacidad de alcance + бюджеты стримов по мере появления.

## Критерии завершения вехи

- `cargo run -p sorafs_node --example pin_fetch` работает на локальных accesorios.
- Torii se combina con `--features sorafs-storage` y se realizan pruebas integradas.
- Documentación ([гайд по хранилищу узла](node-storage.md)) disponible en la configuración predeterminada + CLI inicial; runbook para el operador.
- Telemetría en el tablero de preparación; алерты настроены на насыщение емкости и PoR ошибки.

## Entregables de documentación y operación- Обновить [справочник по хранилищу узла](node-storage.md) с дефолтами конфигурации, использованием CLI and шагами solución de problemas.
- Utilice [runbook операций узла](node-operations.md) en versiones realizadas con solo versiones SF-3.
- Publicar aplicaciones en API `/sorafs/*` en el portal de desarrolladores y descargar en el manifiesto OpenAPI, junto con los controladores Torii.