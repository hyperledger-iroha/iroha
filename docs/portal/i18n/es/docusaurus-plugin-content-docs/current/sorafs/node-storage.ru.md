---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: almacenamiento de nodo
título: Дизайн хранения узла SoraFS
sidebar_label: Дизайн хранения узла
descripción: Arquitectura de arquitectura, ganchos y ganchos de ciclo de vida para Torii узлов, хостящих данные SoraFS.
---

:::nota Канонический источник
:::

## Дизайн хранения узла SoraFS (Черновик)

Esta copia automática, que es el uso Iroha (Torii), puede conectarse a una ranura.
disponibilidad según SoraFS y descargue el disco local para la llave en mano y
обслуживания чанков. Она дополняет descubrimiento-especialidades
`sorafs_node_client_protocol.md` y el robot de accesorios SF-1b, descripción del arquitecto
Información sobre la configuración, configuración y configuración del producto
появиться в узле и gateway-коде. Procedimientos operativos prácticos en
[Runbook операций узла](./node-operations).

### Цели- Revise el validador de archivos o el proceso de publicación Iroha
  El disco duro que aparece en el libro mayor SoraFS está conectado al libro mayor.
- Ajuste del módulo de configuración y actualización del módulo Norito: manifiestos,
  планы чанков, корни Prueba de recuperabilidad (PoR) y publicidad провайдеров — это
  источник истины.
- Principales consultas de los operadores, que no utilizan recursos propios
  слишком большого количества запросов pin o fetch.
- Отдавать здоровье/телеметрию (muestreo PoR, recuperación de latencia чанков, давление на
  диск) обратно в gobernabilidad y cliente.

### Arquitecto высокого уровня

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Ключевые módulos:- **Puerta de enlace**: se activan los componentes HTTP Norito para el pin predeterminado, se recupera la búsqueda
  чанков, muestreo PoR y televisores. Валидирует Norito cargas útiles y направляет
  запросы в tienda de trozos. El protocolo HTTP Torii, no disponible
  вводить новый демон.
- **Registro de PIN**: состояние pin манифестов в `iroha_data_model::sorafs` и
  `iroha_core`. При принятии manifesta registro хранит resumen manifesta, resumen
  плана чанков, корень PoR и флаги возможностей провайдера.
- **Almacenamiento de fragmentos**: дисковая реализация `ChunkStore`, которая принимает подписанные
  manifiestos, materiales de los planes чанков через `ChunkProfile::DEFAULT`, y сохраняет
  чанки в детерминированном diseño. Каждый чанк связан с contenido de huellas dactilares y
  PoR метаданными, поэтому sampling может повторно валидировать без перечитывания
  всего файла.
- **Cuota/Programador**: применяет лимиты оператора (макс. байты диска, макс. pins
  в очереди, макс. búsqueda paralela, TTL (números) y registros IO, чтобы задачи
  libro mayor не были вытеснены. El programador está trabajando en pruebas de PoR y aplicaciones
  muestreo с ограниченным CPU.

### Configuración

Agregue una nueva sección en `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # опциональный человекочитаемый тег
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled`: переключатель участия. Когда false, puerta de enlace возвращает 503 для almacenamiento
  Ендпоинтов и узел не объявляется в discovering.
- `data_dir`: directorio correspondiente para чанков, registros de PoR y búsqueda de televisores.
  Por ejemplo `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: жесткий лимит на pin-данные. Фоновая задача отклоняет
  nuevos pines при достижении limита.
- `max_parallel_fetches`: límite de configuración, programador de datos, чтобы
  Балансировать сеть/IO disk с нагрузкой валидатора.
- `max_pins`: pines de manifiesto máximos, которые принимает узел до применения
  desalojo/contrapresión.
- `por_sample_interval_secs`: trabajos de muestreo PoR automáticos. Каждый
  job incluye listas `N` (según el manifiesto) y emite telemetros.
  Gobernanza может детерминированно масштабировать `N`, задавая ключ
  `profile.sample_multiplier` (целое `1-4`). Значение может быть числом/строкой
  или объектом с anular el perfil, nombre `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: структура, используемая генератором advert для заполнения полей
  `ProviderAdvertV1` (puntero de participación, sugerencias de QoS, temas). Если опущено, узел берет
  valores predeterminados en el registro de gobernanza.

Configuraciones de configuración:- `[sorafs.storage]` se coloca en `iroha_config` y `SorafsStorage` y se descarga
  из конфигурационного файла узла.
- `iroha_core` e `iroha_torii` configuran la configuración de almacenamiento en gateway builder y chunk
  tienda при старте.
- Anulaciones de desarrollo/prueba (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), no
  в producción следует использовать конфиг-файл.

### CLI утилиты

Пока HTTP поверхность Torii еще подключается, crate `sorafs_node` поставляет
тонкий CLI, чтобы операторы могли скриптовать ejercicios de ingestión/exportación против
backend permanente.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` contiene el manifiesto codificado Norito `.to` más bytes de carga útil compatibles.
  Он восстанавливает план чанков из профиля чанкинга манифеста, проверяет паритет
  digest, сохраняет файлы чанков y опционально эмитит JSON `chunk_fetch_specs`, чтобы
  Los instrumentos posteriores pueden mejorar el diseño.
- `export` muestra ID de manifiesto y muestra de manifiesto/carga útil en el disco
  (с опциональным plan JSON), чтобы luminarias оставались воспроизводимыми.

Estos comandos incluyen el resumen JSON Norito en la salida estándar, que contiene todos los scripts. CLI
покрыт интеграционным тестом, который подтверждает корректный manifiestos de ida y vuelta
y cargas útiles de la API Torii.【crates/sorafs_node/tests/cli.rs:1】> Parte HTTP
>
> Torii gateway теперь предоставляет на основе того же ayudantes de solo lectura
> `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — возвращает сохраненный
> Manifiesto Norito (base64) incluido en digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — возвращает детерминированный
> Plantilla JSON (`chunk_fetch_specs`) para herramientas posteriores.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Estos puntos pueden acceder a la CLI, los pagos pueden realizarse desde lugares locales
> скриптов к HTTP sondes без смены парсеров.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Жизненный цикл узла1. **Inicio**:
   - El almacenamiento más completo utilizado para iniciar la tienda de fragmentos en el directorio actual
     и емкостью. Este programa de grabación/configuración de PoR se basa en un pin de reproducción
     манифестов для прогрева кэшей.
   - Registro de la puerta de enlace SoraFS (Norito JSON POST/GET componentes para pin,
     búsqueda, muestreo PoR, televisores).
   - Запустить PoR muestreo trabajador y monitor квот.
2. **Descubrimiento/Anuncios**:
   - Forme `ProviderAdvertV1` en el nuevo teclado electrónico/здоровья, подписать
     ключом, одобренным советом, и опубликовать через descubrimiento canal.
     оставались доступными.
3. **Flujo de trabajo de fijación**:
   - Gateway получает подписанный manifiesto (с планом чанков, корнем PoR and подписями
     совета). Валидирует список alias (`sorafs.sf1@1.0.0` обязателен) y убеждается,
     что план чанков соответствует метаданным манифеста.
   - Проверяет квоты. При превышении capacidad/pin límites отвечает политикой ошибки
     (структурированный Norito).
   - Стримит chunk данные в `ChunkStore`, проверяя digests на ingest. Обновляет PoR
     деревья и хранит metadatos manifestados en el registro.
4. **Obtener flujo de trabajo**:
   - Отдает запросы range чанков с диска. Programador применяет `max_parallel_fetches`
     и возвращает `429` при насыщении.
   - Emite un televisor estructural (Norito JSON) con latencia, bytes servidos yсчетчиками ошибок для aguas abajo monitorización.
5. **Muestreo PoR**:
   - El trabajador выбирает манифесты пропорционально весу (например, байтам хранения) и
     запускает детерминированный muestreo через PoR дерево chunk store.
   - Сохраняет результаты для gobernancia аудита и включает сводки в anuncios de proveedores
     / punto final telem.
6. **Desalojo / квоты**:
   - Los dispositivos instalados se utilizan con nuevos pines. Opcionalmente
     Los operadores pueden implementar políticas de desalojo (por ejemplo, TTL, LRU) después.
     согласования модели gobernanza; пока дизайн предполагает строгие квоты и
     desanclar operaciones, инициируемые оператором.

### Integración de declaraciones y programación- Torii vuelve a conectar la configuración `CapacityDeclarationRecord` o `/v2/sorafs/capacity/declare`
  En la versión `CapacityManager`, esta es la configuración de memoria previa del usuario en memoria.
  зафиксированных fragmentador/carril аллокаций. Cómo publicar instantáneas de solo lectura en televisores
  (`GET /v2/sorafs/capacity/state`) y presenta reservas por perfil/por carril para nuevas versiones
  заказов.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Эндпоинт `/v2/sorafs/capacity/schedule` принимает emitido por la gobernanza `ReplicationOrderV1`
  cargas útiles. Когда заказ нацелен на локального провайдера, менеджер проверяет дублирование
  расписаний, валидирует емкость chunker/lane, резервирует слот and возвращает `ReplicationPlan`
  с описанием оставшейся емкости, чтобы оркестрация могла продолжить la ingestión. Заказы для
  Los fabricantes de archivos adjuntos `ignored`, que mejoran flujos de trabajo multioperador. 【crates/iroha_torii/src/routing.rs:4845】
- Ganchos de finalización (например, после успешной ingerir) вызывают
  `POST /v2/sorafs/capacity/complete` para la reserva de agua
  `CapacityManager::complete_order`. Ответ включает instantánea `ReplicationRelease`
  (totales de остаточные, остатки fragmentador/carril), чтобы herramientas de orquestación могло
  ставить следующий заказ без encuestas. Дальнейшая работа подключит это к oleoducto
  almacén de fragmentos, que es una lógica de ingestión de datos.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】- Встроенный `TelemetryAccumulator` можно обновлять через `NodeHandle::update_telemetry`,
  что позволяет trabajadores en segundo plano фиксировать PoR/uptime samples y со временем
  выводить канонические `CapacityTelemetryV1` cargas útiles без изменения внутренних
  частей planificador.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integración y robot de construcción

- **Gobernanza**: расширить `sorafs_pin_registry_tracker.md` телеметрией хранения
  (tasa de éxito de PoR, utilización del disco). Политики допуска pueden ser tres minimos
  емкость или minимальный Tasa de éxito de PoR до принятия anuncios.
- **SDK de cliente**: раскрыть новую configuración de almacenamiento (лимиты диска, alias), чтобы
  herramientas de gestión могло bootstrapper узлы программно.
- **Telemetría**: integración con la pila de métricas sucesivas (Prometheus /
  OpenTelemetry), varias métricas de datos en paneles de observabilidad.
- **Seguridad**: desactivar el módulo de configuración en el grupo de tareas asíncrono externo con contrapresión
  y рассмотреть sandboxing чтения чанков через io_uring y ограниченные tokio pools,
  чтобы злоумышленники не исчерпали ресурсы.

Este diseño del módulo de configuración opcional y determinista, davaya
Las perillas del operador no están disponibles en la única fuente de alimentación SoraFS. Realización
потребует изменений в `iroha_config`, `iroha_core`, `iroha_torii` y Norito gateway,
а также herramientas для anuncio del proveedor.