---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Протокол SoraFS uso ↔ cliente

Este es el protocolo de acuerdo canónico en el que se encuentra
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Utilice las especificaciones upstream para el diseño del archivo Norito y el registro de cambios;
Una copia portátil de las actividades operativas disponibles en el runbook SoraFS.

## Объявления провайдера и валидация

Los controladores SoraFS distribuyen la carga útil `ProviderAdvertV1` (см.
`crates/sorafs_manifest::provider_advert`), подписанные управляемым оператором.
Объявления фиксируют метаданные обнаружения и guardrails, которые
мульти-источниковый оркестратор применяет на рантайме.- **Срок действия** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Провайдеры
  должны обновлять каждые 12 часов.
- **Capacidad TLV** — TLV-список рекламирует транспортные возможности (Torii,
  QUIC+Noise, relés SoraNet, extensiones de proveedores). Неизвестные коды можно
  Solicite `allow_unknown_capabilities = true`, se recomienda encarecidamente
  GRASA.
- **Sugerencias de QoS** — nivel `availability` (Caliente/Tibio/Frío), максимальная задержка
  извлечения, limит конкурентности y опциональный stream presupuesto. Calidad de servicio completa
  совпадать с наблюдаемой телеметрией и проверяется при admisión.
- **Puntos finales y temas de encuentro**: URL de servicio confidencial mediante TLS/ALPN
  метаданными плюс temas de descubrimiento, на которые клиенты подписываются при
  построении conjuntos de guardia.
- **Политика разнообразия путей** — `min_guard_weight`, límites de distribución para el usuario
  AS/пулов и `provider_failure_threshold` обеспечивают детерминированные
  búsqueda multi-peer'и.
- **Perfil de identificación** — провайдеры обязаны публиковать канонический
  manejar (например, `sorafs.sf1@1.0.0`); opción opcional `profile_aliases`
  миграции старых клиентов.

Правила валидации отвергают нулевой нулевой, пустые списки capacidades/puntos finales/temas,
перепутанные сроки, либо отсутствующие objetivos de QoS. Sobres de admisión
сравнивают тела объявления и предложения (`compare_core_fields`) antes
распространением обновлений.

### Búsqueda de rango de Расширения

Los proveedores de rango de rango deben seleccionar los siguientes metadatos:| Polo | Назначение |
|------|------------|
| `CapabilityType::ChunkRangeFetch` | Tenga en cuenta `max_chunk_span`, `min_granularity` y banderas de configuración/configuración. |
| `StreamBudgetV1` | Configuración/rendimiento del sobre opcional (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). Capacidad de alcance de Требует. |
| `TransportHintV1` | Transporte previo previo (por ejemplo, `torii_http_range`, `quic_stream`, `soranet_relay`). Prioridades `0–15`, doblemente desactivadas. |

Поддержка herramientas:

- Пайплайны anuncio del proveedor должны валидировать capacidad de rango, presupuesto de flujo
  y sugerencias de transporte para la carga útil determinada por las auditorías.
- `cargo xtask sorafs-admission-fixtures` paquete de múltiples fuentes
  anuncios вместе с accesorios de degradación в
  `fixtures/sorafs_manifest/provider_admission/`.
- Anuncios de gama без `stream_budget` или `transport_hints` отклоняются загрузчиками
  CLI/SDK para la planificación, arnés multifuente integrado en soportes
  ожиданиями admisión Torii.

## Puerta de enlace de puntos finales de rango

Las puertas de enlace controlan las conexiones HTTP determinadas y se utilizan metadatos.

### `GET /v2/sorafs/storage/car/{manifest_id}`| Требование | Detalles |
|------------|--------|
| **Encabezados** | `Range` (intervalo de frecuencia, compensaciones de fragmentos), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional y base64 opcional `X-SoraFS-Stream-Token`. |
| **Respuestas** | `206` con `Content-Type: application/vnd.ipld.car`, `Content-Range`, descripciones de intervalos regulares, metadanos `X-Sora-Chunk-Range` y este fragmentador/token de encabezados. |
| **Modos de fallo** | `416` para nunca выровненных дипазонов, `401` para отсутствующих/невалидных токенов, `429` при Presupuesto de flujo/byte previo. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Recuperar fragmentos nuevos con los encabezados más fragmentos de resumen predeterminados.
Para recuperar o descargar descargas forenses, no hay suficientes cortes de CAR.

## Flujo de trabajo de múltiples orquestadores

Когда включен SF-6 búsqueda de múltiples fuentes (Rust CLI через `sorafs_fetch`,
SDK según `sorafs_orchestrator`):1. **Собрать входные данные** — decodificar el manifiesto de trozos del plan, получить
   Publicar anuncios y, opcionalmente, solicitar instantáneas de telemetría.
   (`--telemetry-json` o `TelemetrySnapshot`).
2. **Mostrar marcador** — `Orchestrator::build_scoreboard` оценивает
   пригодность и записывает причины отказа; `sorafs_fetch --scoreboard-out`
   сохраняет JSON.
3. **Планировать fragment'и** — `fetch_with_scoreboard` (o `--plan`) primero
   rango de operación, presupuesto de transmisión, límites de reintento/peer (`--retry-budget`,
   `--max-peers`) y coloca el token de flujo en el manifiesto de alcance del archivo.
4. **Проверить recibos** — результаты включают `chunk_receipts` и
   `provider_reports`; Resumen de CLI basado en `provider_reports`, `chunk_receipts`
   y `ineligible_providers` para paquetes de pruebas.

Распространенные ошибки, возвращаемые оperatoram/SDK:

| Ошибка | Descripción |
|--------|----------|
| `no providers were supplied` | No se pueden filtrar rápidamente las filtraciones. |
| `no compatible providers available for chunk {index}` | No es necesario separar o bloquear el trozo de hormigón. |
| `retry budget exhausted after {attempts}` | Utilice `--retry-budget` o incluya otros pares. |
| `no healthy providers remaining` | Все провайдеры отключены после повторных отказов. |
| `streaming observer failed` | El escritor de COCHE aguas abajo завершился с ошибкой. |
| `orchestrator invariant violated` | Manifiesto actualizado, marcador, instantánea de telemetría y CLI JSON para clasificación. |

## Telemetría y documentación- Métricas, emitiruemos оркестратором:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (с тегами manifiesto/región/proveedor). Instale `telemetry_region` en configuración o
  через CLI флаги, чтобы дашборды разделяли по флоту.
- CLI/SDK buscar resúmenes включают сохраненный marcador JSON, recibos de fragmentos y
  informes de proveedores, которые должны входить в paquetes de implementación для gate'ов SF-6/SF-7.
- Controladores de puerta de enlace publicados `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`,
  чтобы SRE paneles de control коррелировали решения оркестратора с поведением сервера.

## Ayuda CLI y REST

- `iroha app sorafs pin list|show`, `alias list` y `replication list` оборачивают
  Puntos finales REST de registro pin y certificación de bloques Norito JSON
  для аудиторских доказательств.
- `iroha app sorafs storage pin` y `torii /v2/sorafs/pin/register` según el modelo Norito
  y manifiestos JSON además de pruebas de alias y sucesores opcionales; pruebas mal formadas
  возвращают `400`, pruebas obsoletas дают `503` с `Warning: 110`, pruebas caducadas
  возвращают `412`.
- Puntos finales REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`,
  `/v2/sorafs/replication`) включают структуры atestación, чтобы клиенты могли
  pruebe estos encabezados de bloque disponibles antes del diseño.

## Ссылки- Especificaciones canónicas:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito tipos: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ayuda CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caja de control: `crates/sorafs_orchestrator`
- Paquete de datos: `dashboards/grafana/sorafs_fetch_observability.json`