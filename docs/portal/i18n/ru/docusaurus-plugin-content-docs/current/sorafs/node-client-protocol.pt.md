---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Протокол №  клиента SoraFS

Este guia возобновляет определенный канонический протокол
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Используйте конкретные исходные параметры макетов Norito на уровне байтов и журналах изменений;
копия портала mantem os destaquesoperacionais perto do restante dos runbooks
SoraFS.

## Объявления о проверке и проверке

Поставщики SoraFS для распространения полезных нагрузок `ProviderAdvertV1` (veja
`crates/sorafs_manifest::provider_advert`) Assinados Pelo Operador Governado. Ос
реклама фиксированных метаданных декоберта и ограждений, которые можно использовать
Приложение с несколькими источниками в среде выполнения.

- **Вигенсия** - `issued_at < expires_at <= issued_at + 86,400 s`. Проведоры
  Разработаем ремонт за 12 часов.
- **TLVs de capacidade** — список TLV anuncia recursos de Transporte (Torii,
  QUIC+Noise, реле SoraNet, дополнительные возможности). Codigos desconhecidos
  Podem ser ignorados quando `allow_unknown_capabilities = true`, затем
  СМАЗКА orientacao.
- **Подсказки о QoS** - уровень `availability` (горячий/теплый/холодный), максимальная задержка
  рекуперация, лимит согласования и бюджет потока по желанию. Разработка QoS
  Алинхар с телеметрическим наблюдением и электронной проверкой на приеме.
– **Конечные точки и темы встреч** – URL-адреса конкретных служб с метаданными
  TLS/ALPN чаще всего раскрывает темы, которые могут быть найдены клиентами.
  ao сконструировать наборы охранников.
- **Политика диверсификации каминьо** - `min_guard_weight`, разветвленные крышки
  AS/pool e `provider_failure_threshold`ornam possiveis извлекает детерминированные значения
  многоранговый.
- **Identificadores de perfil** — реквизиты, предназначенные для экспорта или дескриптора canonico (например.
  `sorafs.sf1@1.0.0`); `profile_aliases` вариант ajudam clientes antigos a migrar.

Правила проверки подлинности нулевой ставки, списки возможностей/конечных точек/тем,
забота о порядке или целях повышения качества обслуживания. Сравнение входных конвертов
наши рекламные объявления и предложения (`compare_core_fields`) перед распространением
атуализако.

### Расширения выборки диапазона

В ассортимент поставщиков входят следующие метададо:

| Кампо | Предложение |
|-------|-----------|
| `CapabilityType::ChunkRangeFetch` | Объявите `max_chunk_span`, `min_granularity` и флаги подтверждения/проверки. |
| `StreamBudgetV1` | Дополнительный конверт согласования/пропускной способности (`max_in_flight`, `max_bytes_per_sec`, `burst` опционально). Запросите емкость диапазона. |
| `TransportHintV1` | Предпочтения по транспортировке заказов (например, `torii_http_range`, `quic_stream`, `soranet_relay`). Приоритеты `0-15` и дубликаты обновленных файлов. |

Поддержка инструментов:

- Конвейеры провайдера рекламы определяют диапазон допустимых значений, потоковый бюджет и
  транспортные подсказки перед отправкой полезной нагрузки, определяемой для аудиторий.
- `cargo xtask sorafs-admission-fixtures` agrupa рекламирует canonicos из нескольких источников
  junto com понизить версию светильников em `fixtures/sorafs_manifest/provider_admission/`.
- Объявления в диапазоне, опущенном `stream_budget` или `transport_hints`, в указанных ранее местах.
  Загрузчики pelos CLI/SDK перед повесткой дня, управлением или использованием нескольких источников
  alinhado com в качестве ожидаемого допуска к Torii.

## Конечные точки диапазона до шлюзаШлюзы, требующие HTTP-запросов, определяют, какие метаданные делают
реклама.

### `GET /v2/sorafs/storage/car/{manifest_id}`

| Реквизито | Подробности |
|-----------|----------|
| **Заголовки** | `Range` (уникальное значение смещения фрагмента), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` необязательно и `X-SoraFS-Stream-Token` base64 обязательно. |
| **Ответы** | `206` с `Content-Type: application/vnd.ipld.car`, `Content-Range` описаны на сервере, метаданные `X-Sora-Chunk-Range` и заголовки блоков/токенов. |
| **Фальхас** | `416` для удаленных диапазонов, `401` для устаревших/недействительных токенов, `429` для превышения бюджетов потока/байтов. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Извлеките единый фрагмент из заголовков сообщений или выберите детерминированный фрагмент.
Используется для повторных попыток или загрузок, когда нужны фрагменты CAR.

## Рабочий процесс для orquestrador с несколькими источниками

Когда вы можете получить доступный SF-6 из нескольких источников (CLI Rust через `sorafs_fetch`,
SDK через `sorafs_orchestrator`):

1. **Coletar entradas** — расшифровка плана кусков, которые проявляются, puxar os
   самые последние рекламные объявления, опционально, передайте снимки телеметрии
   (`--telemetry-json` или `TelemetrySnapshot`).
2. **Построить табло** - `Orchestrator::build_scoreboard` avalia a
   легитимность и регистрация граждан; `sorafs_fetch --scoreboard-out`
   сохраняйте JSON.
3. **Часки повестки дня** — `fetch_with_scoreboard` (или `--plan`) для ограничения ограничений.
   диапазон, бюджеты потока, количество повторов/одноранговых узлов (`--retry-budget`, `--max-peers`)
   Выпускаете токен потока с помощью манифеста для каждого запроса.
4. **Проверка данных** - как указано, включая `chunk_receipts` и `provider_reports`;
   итоги сохранения CLI `provider_reports`, `chunk_receipts` e
   `ineligible_providers` для пакетов доказательств.

Ошибки сообщаются с операторами/SDK:

| Эрро | Описание |
|------|-----------|
| `no providers were supplied` | Не используйте элегантный вход в фильтр. |
| `no compatible providers available for chunk {index}` | Несоответствие диапазона или бюджета конкретному фрагменту. |
| `retry budget exhausted after {attempts}` | Добавьте `--retry-budget` или удалите одноранговые узлы с помощью Falha. |
| `no healthy providers remaining` | Все, что вам нужно сделать, это сделать, чтобы избежать повторных ошибок. |
| `streaming observer failed` | О, писатель, CAR, прерванный по течению. |
| `orchestrator invariant violated` | Захват манифеста, табло, моментального снимка телеметрии и CLI JSON для сортировки. |

## Телеметрия и доказательства

- Метрики выбросов пело оркестратора:  
  И18НИ00000077Х, И18НИ00000078Х,
  И18НИ00000079Х, И18НИ00000080Х
  (тагеадас по манифесту/регион/поставщик). Defina `telemetry_region` в конфигурации
  или с помощью флагов CLI для отдельных панелей мониторинга.
- Сохраняющиеся отчеты по выборке CLI/SDK, включая табло JSON, квитанции фрагментов.
  Поставщик сообщает, что были разработаны пакеты развертывания для шлюзов SF-6/SF-7.
- Обработчики шлюза, описание `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  для того, чтобы информационные панели SRE коррелировали с решениями оркестратора в соответствии со стандартами
  делаю сервидор.

## Помощники CLI и REST- `iroha app sorafs pin list|show`, `alias list` и `replication list` связаны с операционной системой
  конечные точки REST выполняют регистрацию контактов и импортируют Norito JSON грубо с блоками
  аттестация для аудиторских доказательств.
- `iroha app sorafs storage pin` и `torii /v2/sorafs/pin/register` aceitam манифестирует
  Norito или JSON с доказательствами псевдонимов и дополнительными преемниками; доказательства
  geram `400`, доказательства устарели, retornam `503` com `Warning: 110`, электронные доказательства истекли
  реторнам `412`.
- Конечные точки REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`, `/v2/sorafs/replication`)
  включить estruturas de attestation для клиентов, проверяющих данные против них
  последние заголовки блоков до начала игры.

## Ссылки

- Спецификация каноника:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Типы Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Помощники CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Ящик для заказа: `crates/sorafs_orchestrator`
- Пакет информационных панелей: `dashboards/grafana/sorafs_fetch_observability.json`