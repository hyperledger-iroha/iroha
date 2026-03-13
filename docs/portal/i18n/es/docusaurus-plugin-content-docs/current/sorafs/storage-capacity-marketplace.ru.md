---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado-capacidad-de-almacenamiento
título: Маркетплейс емкости хранения SoraFS
sidebar_label: Marcas comerciales
descripción: Plan SF-2c para comercializar herramientas, órdenes de replicación, televisores y ganchos de gobernanza.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/storage_capacity_marketplace.md`. Deje copias sincronizadas y active la documentación actual.
:::

# Маркетплейс емкости хранения SoraFS (черновик SF-2c)

Пункт hoja de ruta SF-2c вводит управляемый mercado, где proveedores хранилища
декларируют коммитнутую емкость, получают órdenes de replicación y зарабатывают tarifas
пропорционально предоставленной доступности. Este documento contiene entregables
для первого релиза и разбивает их на accionables треки.

## Цели

- Фиксировать обязательства proveedores по емкости (общие байты, лимиты по lane, срок действия)
  En forma proporcionada por Gobernanza, transporte SoraNet e Torii.
- Распределять pines между proveedores согласно заявленной емкости, estacas y políticas-ограничениям,
  сохраняя детерминированное поведение.
- Измерять доставку хранения (успех репликации, uptime, pruebas целостности) y
  экспортировать телеметрию для распределения tarifas.
- Предоставлять процессы revocación y disputa, чтобы нечестные proveedores могли быть
  наказаны или удалены.

## Conceptos domésticos| Concepción | Descripción | Первичный entregable |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Carga útil Norito, proveedor de ID detallado, fragmentador de perfiles de datos, GiB comprometido, límites de carril, sugerencias de precios, compromiso de participación y diseño. | Схема + валидатор в `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrucción, gobierno corporativo, manifiesto CID de nombres de proveedores o proveedores no autorizados, estándares de seguridad y métricas SLA. | Norito muestra el contrato inteligente Torii + API. |
| `CapacityLedger` | Registro dentro y fuera de la cadena, declaraciones de acciones activas, órdenes de replicación, tarifas de protección y tarifas de implementación. | Un contrato inteligente de módulo o un servicio de código auxiliar fuera de cadena con una instantánea determinada. |
| `MarketplacePolicy` | Gobernanza política, participación mínima, tres auditorías y estrategias. | Estructura de configuración en `sorafs_manifest` + gobierno del documento. |

### Реализованные схемы (статус)

## Разбиение работ

### 1. Слой схем и реестра| Задача | Propietario(s) | Примечания |
|------|----------|-------|
| Utilice `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipo de Almacenamiento / Gobernanza | Utilice Norito; включить семантическое версионирование и ссылки на capacidades. |
| Realice el módulo analizador + validador en `sorafs_manifest`. | Equipo de almacenamiento | Tenga en cuenta las identificaciones monotonales, las etiquetas corporativas y las apuestas por juego. |
| Puede eliminar el fragmentador de metadatos del archivo `min_capacity_gib` para el perfil del archivo. | Grupo de Trabajo sobre Herramientas | El cliente debe establecer un perfil mínimo de hardware en el perfil. |
| Consulte el documento `MarketplacePolicy`, descripción detallada de las barandillas de admisión y gráficos. | Consejo de Gobierno | Publicado en la sección de documentos con valores predeterminados de políticas. |

#### Определения схем (реализованы)- `CapacityDeclarationV1` фиксирует подписанные обязательства емкости для каждого proveedor, включая канонические maneja fragmentador, ссылки на capacidades, opciones opcionales по carril, sugerencias по precios, окна валидности и metadatos. Валидация обеспечивает ненулевой estaca, канонические manijas, дедуплицированные alias, mayúsculas en el carril en пределах заявленного total y monotonnyy учет GiB.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` связывает manifests с назначениями, выпущенными Governance, с целями избыточности, порогами SLA and гарантиями на asignation; Los validadores de canon manejan el fragmentador, los proveedores únicos y las organizaciones dentro de la fecha límite, como Torii o el código de registro. orden.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` describe instantáneas de instantáneas (desbloqueadas frente a GiB implementadas, replicaciones seleccionadas, tiempo de actividad/PoR actuales), которые питают honorarios de распределение. Gran cantidad de proveedores que utilizan la declaración de datos, y resultados - en 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Общие helpers (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de carril/asignación/SLA) para determinar la clave de acceso y los informes, Estos pueden ser útiles para CI y herramientas posteriores.【crates/sorafs_manifest/src/capacity.rs:230】- `PinProviderRegistry` publica una instantánea en cadena con `/v2/sorafs/capacity/state`, muestra las declaraciones de proveedores y registra el libro de tarifas para determinar Norito JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Покрытие валидации проверяет соблюдение канонических handles, обнаружение дубликатов, granisы по lane, guards назначения репликации и проверки диапазонов телеметрии, чтобы регрессии всплывали сразу в CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Herramientas del operador: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` especificaciones de conversión de datos y cargas útiles Norito, blobs base64 y resúmenes JSON, nombres de operadores подготовить accesorios `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` y accesorios de orden de replicación con validación local. 【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Accesorios de referencia живут в `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) y generadores desde `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Plano de control de integración| Задача | Propietario(s) | Примечания |
|------|----------|-------|
| Agregue las cargas útiles JSON Torii, `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` con Norito. | Torii Equipo | Зеркалировать логику валидации; переиспользовать Norito Ayudantes JSON. |
| Протолкнуть instantáneas `CapacityDeclarationV1` en el orquestador del marcador de metadatos y en el plan de recuperación de puerta de enlace. | Equipo de trabajo de herramientas / orquestador | Utilice `provider_metadata` para determinar la capacidad, ya que la puntuación de múltiples características limita los límites del carril. |
| Coloque órdenes de replicación en el orquestador/puerta de enlace de los clientes para mejorar las asignaciones y la conmutación por error de sugerencias. | Equipo de Networking TL / Gateway | Generador de marcadores que permiten replicar órdenes de gobernanza. |
| Herramientas CLI: расширить `sorafs_cli` командами `capacity declare`, `capacity telemetry`, `capacity orders import`. | Grupo de Trabajo sobre Herramientas | Предоставить детерминированный JSON + marcador de salidas. |

### 3. Mercado político y gobernanza| Задача | Propietario(s) | Примечания |
|------|----------|-------|
| Утвердить `MarketplacePolicy` (participación mínima, multiplicadores de estratos, auditoría periódica). | Consejo de Gobierno | Publique en documentos y revise rápidamente la historia. |
| Добавить ganchos de gobernanza, чтобы El Parlamento puede aprobar, renovar y revocar declaraciones. | Consejo de Gobernanza / Equipo de Contratos Inteligentes | Implementar eventos Norito + manifiestos de ingestión. |
| Реализовать график штрафов (снижение honorarios, reducción de fianzas), привязанный к телеметрируемым нарушениям SLA. | Consejo de Gobierno / Tesorería | Согласовать с liquidación de salidas `DealEngine`. |
| Documente el proceso de disputa y la matricialización. | Documentos / Gobernanza | Сослаться на disputa runbook + CLI de ayuda. |

### 4. Tarifas de medición y distribución| Задача | Propietario(s) | Примечания |
|------|----------|-------|
| Utilice la medición de ingesta en Torii para comenzar con `CapacityTelemetryV1`. | Torii Equipo | Валидировать GiB-hora, успех PoR, tiempo de actividad. |
| Desactive la medición de tuberías `sorafs_node` para activar la medición en el pedido + estadística SLA. | Equipo de almacenamiento | Согласовать с órdenes de replicación y maneja fragmentador. |
| Liquidación de tuberías: conversión de telemétricos + réplicas de pagos, nominación en XOR, resúmenes listos para la gestión y contabilidad ficticia del libro mayor. | Equipo de Tesorería / Almacenamiento | Подключить к Deal Engine / Exportaciones de tesorería. |
| Exportar paneles/alertas para la medición de datos (ingesta de trabajos pendientes, configuración de telemetría). | Observabilidad | Utilice el paquete Grafana para conectar el SF-6/SF-7. |- Torii, el archivo publicado `/v2/sorafs/capacity/telemetry` y `/v2/sorafs/capacity/state` (JSON + Norito), estos operadores pueden administrar instantáneas de telemetría эпохам, а инспекторы - получать канонический libro mayor для аудита или упаковки доказательств.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- La integración `PinProviderRegistry` garantiza qué órdenes de replicación están disponibles en el punto final; helpers CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) permite validar/descargar un televisor y ejecutar una automatización con un hash determinado y un alias de eliminación.
- Las instantáneas de medición forman la instantánea `CapacityTelemetrySnapshot`, se descargan de la instantánea `metering`, y las exportaciones Prometheus se exportan directamente a la importación. Placa Grafana en `docs/source/grafana_sorafs_metering.json`, чтобы команды биллинга отслеживали накопление GiB-hour, прогнозируемые nano-SORA fee y соблюдение SLA en реальном времени.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Como suavizado de medición, captura de instantáneas `smoothed_gib_hours` e `smoothed_por_success_bps`, todos los operadores pueden seleccionar la red EMA сырыми счетчиками, которые gobierno использует для pagos.【crates/sorafs_node/src/metering.rs:401】

### 5. Обработка disputa y revocación| Задача | Propietario(s) | Примечания |
|------|----------|-------|
| Определить payload `CapacityDisputeV1` (заявитель, evidencia, целевой proveedor). | Consejo de Gobierno | Norito programa + validador. |
| Поддержка CLI для подачи disputas y ответов (con pruebas adjuntas). | Grupo de Trabajo sobre Herramientas | Tenga en cuenta el paquete de pruebas de hash determinado. |
| Добавить автоматические проверки повторяющихся нарушений SLA (escalada automática en disputa). | Observabilidad | Пороги alerta y ganchos de gobernanza. |
| Документировать revocación del libro de jugadas (período de gracia, datos anclados de эвакуация). | Equipo de Documentos/Almacenamiento | Consulte el documento de políticas y el runbook del operador. |

## Требования к тестированию и CI

- Юнит-тесты для всех новых валидаторов схем (`sorafs_manifest`).
- Pruebas integrales y simulaciones de cálculo: declaración → orden de replicación → medición → pago.
- Flujo de trabajo de CI para la regeneración de muestras, dispositivos de sincronización/telemetría y dispositivos de sincronización (descarga `ci/check_sorafs_fixtures.sh`).
- Pruebas de carga de la API de registro (con 10.000 proveedores, 100.000 pedidos).

## Telemetría y tableros- Paneles de tablero:
  - Декларированная vs использованная емкость по proveedor.
  - Órdenes de replicación pendientes y средняя задержка назначения.
  - Соответствие SLA (% de tiempo de actividad), según el PoR).
  - Накопление tarifas y штрафы по эпохам.
- Alertas:
  - Proveedor ниже minимальной заявленной емкости.
  - Orden de replicación завис более чем на SLA.
  - Tubería de medición Сбои.

## Materiales de documentación

- Руководство оператора по декларации емкости, продлению обязательств и мониторингу использования.
- Руководство по gobernancia для утверждения деклараций, выдачи órdenes, обработки disputas.
- Referencia de API para puntos finales y formato de orden de replicación.
- Preguntas frecuentes sobre Marketplace para desarrolladores.

## Чеклист готовности к GA

La hoja de ruta de Punk **SF-2c** bloquea el lanzamiento de la producción para las aplicaciones de construcción sólidas
по учету, обработке disputas и онбордингу. No utilice artefactos ni criterios según los criterios
приемки в синхроне с реализацией.### Ночной учет и сверка XOR
- Exporte archivos de instantáneas y exporte el libro mayor XOR durante este período, de la siguiente manera:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Хелпер завершится с ненулевым кодом при недостающих/переплаченных liquidación o штрафах и
  Resumen del archivo de texto Prometheus.
- Alerta `SoraFSCapacityReconciliationMismatch` (в `dashboards/alerts/sorafs_capacity_rules.yml`)
  срабатывает, когда reconciliation метрики сообщают о расхождениях; tableros de instrumentos лежат в
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archivar resumen JSON y hashes en `docs/examples/sorafs_capacity_marketplace_validation/`
  вместе с paquetes de gobernanza.

### Доказательства disputa y corte
- Подавайте disputas через `sorafs_manifest_stub capacity dispute` (pruebas:
  `cargo test -p sorafs_car --test capacity_cli`), estas cargas útiles están instaladas.
- Запускайте `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` y наборы
  штрафов (`record_capacity_telemetry_penalises_persistent_under_delivery`), чтобы доказать
  детерминированное воспроизведение disputas y barras.
- Utilice `docs/source/sorafs/dispute_revocation_runbook.md` para descargar dispositivos y
  escalas; привязывайте huelga de aprobaciones обратно в informe de validación.

### Proveedores de pruebas de humo y agua
- Regenerar artefactos declarados/telemétricos con `sorafs_manifest_stub capacity ...` y
  прогоняйте CLI tests перед подачей (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Instale el dispositivo Torii (`/v2/sorafs/capacity/declare`)
  `/v2/sorafs/capacity/state` más pantallas Grafana. Следуйте flujo выхода в
  `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Архивируйте подписанные artefactos y salidas de reconciliación внутри
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Зависимости и последовательность1. Завершить SF-2b (política de admisión): operación del mercado para proveedores locales.
2. Realice el registro + registro (este documento) antes de la integración Torii.
3. Завершить tubería de medición до включения выплат.
4. Ejemplo final: incluir tarifas de control de gobernanza después de la puesta en escena de datos de medición.

El progreso debe incluirse en la hoja de ruta según este documento. Hoja de ruta actualizada después de esto,
как каждая основная секция (схемы, plano de control, интеграция, medición, обработка disputas) достигнет
característica completa статуса.