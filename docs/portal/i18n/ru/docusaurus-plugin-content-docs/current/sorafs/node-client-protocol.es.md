---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Протокол узла ↔ клиента SoraFS

Это руководство возобновило каноническое определение протокола в
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Используйте вышестоящую спецификацию для макетов Norito на уровне байтов и лос
журналы изменений; копия дель портала, мантьен лос оперативные точки, серка дель ресто
Runbooks SoraFS.

## Объявления о подтверждении и подтверждении

Проверки SoraFS, полученные от полезных нагрузок `ProviderAdvertV1` (версия
`crates/sorafs_manifest::provider_advert`) фирмы для государственного оператора.
Объявления о предоставлении метаданных описания и защиты, которую он
Многофункциональный оркестр вводится во время выброса.

- **Вигенсия** — `issued_at < expires_at ≤ issued_at + 86,400 s`. Лос-проведерес
  Дебен рефрескар каждые 12 часов.
- **TLVs de capacidades** — список TLV, объявляющий о транспортных функциях (Torii,
  QUIC+Noise, ресурсы SoraNet, расширения поставщиков). Los códigos desconocidos
  можно опустить, когда `allow_unknown_capabilities = true`, следуйте указаниям
  СМАЗКА.
- **Pistas de QoS** — уровень `availability` (горячий/теплый/холодный), максимальная задержка
  восстановление, ограничение одновременного использования и предпосылка потока необязательно. Качество обслуживания
  Вы должны быть в линейном режиме с телеметрическим наблюдением и проверять вход.
- **Конечные точки и темы встреч** – URL-адреса конкретных служб.
  метаданные TLS/ALPN больше описанных тем и клиентов
  Вам необходимо подписаться на создание комплектов защиты.
- **Политика разнообразия рутинных дел** — `min_guard_weight`, топы разветвлений
  AS/pool y `provider_failure_threshold` имеет возможность детерминированной выборки
  многоранговый.
- **Identificadores de perfil** — лос-провередорес-дебен-экспонер эль-дескриптора
  canónico (стр. например, `sorafs.sf1@1.0.0`); `profile_aliases` дополнительные возможности
  migrar clientes antiguos.

Правила проверки повторяемости ставок, списки свободных возможностей,
конечные точки или темы, разрозненные сведения или цели неудовлетворительного качества обслуживания. Лос
Собрес-де-адмисион сравнивает лос-куэрпос-дель-анунсио и ла пропуэста
(`compare_core_fields`) до актуализации средств.

### Расширения выборки по рангам

Los provedores con rango включают в себя следующие метаданные:

| Кампо | Пропозито |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | Объявите `max_chunk_span`, `min_granularity` и флаги выравнивания/привязки. |
| `StreamBudgetV1` | Дополнительный конверт одновременного выполнения/пропускной способности (`max_in_flight`, `max_bytes_per_sec`, `burst` необязательно). Требуется возможность рангом. |
| `TransportHintV1` | Предпочтения по транспортировке заказов (стр. например, `torii_http_range`, `quic_stream`, `soranet_relay`). Приоритеты `0–15` и повторяющиеся действия. |

Комплектация инструментов:- Каналы провайдера рекламы должны иметь действующую способность ранго, потоковую передачу.
  бюджет и транспортные подсказки перед полезной нагрузкой, определяемой для аудитории.
- `cargo xtask sorafs-admission-fixtures` многофункциональные объявления
  canónicos junto с приспособлениями для понижения версии ru
  `fixtures/sorafs_manifest/provider_admission/`.
- Los anuncios con rango que omiten `stream_budget` или `transport_hints` сын
  Загрузка загрузчиков CLI/SDK перед программированием, обслуживанием файлов
  многократный доступ к Torii.

## Конечные точки рангом шлюза

Шлюзы принимают запросы HTTP, которые определяют метаданные
лос анонсы.

### `GET /v2/sorafs/storage/car/{manifest_id}`

| Реквизито | Подробности |
|-----------|----------|
| **Заголовки** | `Range` (отключено для смещения фрагментов), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` необязательно и `X-SoraFS-Stream-Token` base64 обязательно. |
| **Ответы** | `206` с `Content-Type: application/vnd.ipld.car`, `Content-Range`, которые описывают выход из службы, метаданные `X-Sora-Chunk-Range` и заголовки блоков/токенов. |
| **Моды падения** | `416` для обессоленных рангов, `401` для ошибочных или недействительных токенов, `429`, если они превышают предпосылки потока/байтов. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Извлеките отдельный фрагмент с заголовками ошибок, чтобы определить дайджест.
кусок. Для восстановления или удаления остатков, когда не требуются ломтики
АВТОМОБИЛЬ.

## Flujo de trabajo del orquestador multifuente

Когда вы научитесь извлекать многофункциональный SF-6 (CLI Rust через `sorafs_fetch`,
SDK через `sorafs_orchestrator`):

1. **Recopilar entradas** — расшифровка плана фрагментов манифеста, запись
   больше сообщений и, при желании, можно сделать снимок телеметрии
   (`--telemetry-json` или `TelemetrySnapshot`).
2. **Построить табло** — оценка `Orchestrator::build_scoreboard`
   элегибилидад и регистрация преступлений; `sorafs_fetch --scoreboard-out`
   сохраняйте JSON.
3. **Программные фрагменты** — `fetch_with_scoreboard` (или `--plan`)
   ограничения на рангом, пресупуэстос де поток, топес де реинтентос/пэры
   (`--retry-budget`, `--max-peers`) и выдайте токен потока с учетом баланса
   манифест для каждого запроса.
4. **Проверить данные** — las salidas incluyen `chunk_receipts` y
   `provider_reports`; постоянные резюме CLI `provider_reports`,
   `chunk_receipts` и `ineligible_providers` для комплектов доказательств.

Ошибки, связанные с использованием операционных систем/SDK:

| Ошибка | Описание |
|-------|-------------|
| `no providers were supplied` | Никаких проходов через фильтр. |
| `no compatible providers available for chunk {index}` | Откажитесь от ранго или предполагайте, что это конкретный кусок. |
| `retry budget exhausted after {attempts}` | Инкремент `--retry-budget` или изгнание сверстников. |
| `no healthy providers remaining` | Все проверяющие quedaron deshabilitados tras Fallos Repetidos. |
| `streaming observer failed` | Эскритор CAR ниже по течению прерван. |
| `orchestrator invariant violated` | Манифест Captura, табло, снимок телеметрии и JSON-дель-CLI для сортировки. |

## Телеметрия и доказательства- Показатели выбросов для оркестадора:  
  И18НИ00000077Х, И18НИ00000078Х,
  И18НИ00000079Х, И18НИ00000080Х
  (этикет манифеста/региона/поставщика). Конфигурация `telemetry_region` ru
  config или с помощью флагов CLI, чтобы панели мониторинга были разделены по группам.
- Резюме выборки в CLI/SDK, включая табло JSON, сохраняются и возвращаются.
  фрагменты и сведения о поставщиках, которые можно использовать в пакетах развертывания
  для ворот SF-6/SF-7.
- Обработчики шлюза экспонируются `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`.
  для коррелирования информационных панелей SRE с решениями организатора
  comportamiento del servidor.

## Поддержка CLI и REST

- `iroha app sorafs pin list|show`, `alias list` и `replication list` envuelven los
  конечные точки REST для регистрации контактов и импорта Norito JSON Crudo с блоками
  аттестация для аудиторских доказательств.
- `iroha app sorafs storage pin` y `torii /v2/sorafs/pin/register` акцептанные манифесты
  Norito или JSON — дополнительные доказательства псевдонимов и преемников; доказательства
  elevan `400`, доказательства устарели, экспонен `503` с `Warning: 110`, y доказательства
  истекший срок `412`.
- Потеря конечных точек REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`,
  `/v2/sorafs/replication`) включает в себя структуры аттестации для того, чтобы
  Клиенты проверяют данные против заголовков последнего блока перед актом.

## Ссылки

- Каноническая спецификация:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Типы Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Доступ к CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Ящик Оркестадора: `crates/sorafs_orchestrator`
- Пакет приборных панелей: `dashboards/grafana/sorafs_fetch_observability.json`