---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Протокол SoraFS узел ↔ клиент

Этот гид резюмирует каноническое определение протокола в
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Используйте upstream-спецификацию для байтовых Norito layout и changelog'ов;
портальная копия держит операционные акценты рядом с остальными runbook SoraFS.

## Объявления провайдера и валидация

Провайдеры SoraFS распространяют payload'ы `ProviderAdvertV1` (см.
`crates/sorafs_manifest::provider_advert`), подписанные управляемым оператором.
Объявления фиксируют метаданные обнаружения и guardrails, которые
мульти-источниковый оркестратор применяет на рантайме.

- **Срок действия** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Провайдеры
  должны обновлять каждые 12 часов.
- **Capability TLV** — TLV-список рекламирует транспортные возможности (Torii,
  QUIC+Noise, SoraNet relays, vendor extensions). Неизвестные коды можно
  пропускать при `allow_unknown_capabilities = true`, следуя рекомендациям
  GREASE.
- **QoS hints** — tier `availability` (Hot/Warm/Cold), максимальная задержка
  извлечения, лимит конкурентности и опциональный stream budget. QoS должен
  совпадать с наблюдаемой телеметрией и проверяется при admission.
- **Endpoints и rendezvous topics** — конкретные service URL с TLS/ALPN
  метаданными плюс discovery topics, на которые клиенты подписываются при
  построении guard sets.
- **Политика разнообразия путей** — `min_guard_weight`, лимиты fan-out для
  AS/пулов и `provider_failure_threshold` обеспечивают детерминированные
  multi-peer fetch'и.
- **Идентификаторы профилей** — провайдеры обязаны публиковать канонический
  handle (например, `sorafs.sf1@1.0.0`); опциональные `profile_aliases` помогают
  миграции старых клиентов.

Правила валидации отвергают нулевой stake, пустые списки capabilities/endpoints/topics,
перепутанные сроки, либо отсутствующие QoS targets. Admission envelopes
сравнивают тела объявления и предложения (`compare_core_fields`) перед
распространением обновлений.

### Расширения range fetch

Range-совместимые провайдеры включают следующую метадату:

| Поле | Назначение |
|------|------------|
| `CapabilityType::ChunkRangeFetch` | Объявляет `max_chunk_span`, `min_granularity` и флаги выравнивания/доказательств. |
| `StreamBudgetV1` | Опциональный envelope конкурентности/throughput (`max_in_flight`, `max_bytes_per_sec`, опциональный `burst`). Требует range capability. |
| `TransportHintV1` | Упорядоченные предпочтения транспорта (например, `torii_http_range`, `quic_stream`, `soranet_relay`). Приоритеты `0–15`, дубли отклоняются. |

Поддержка tooling:

- Пайплайны provider advert должны валидировать range capability, stream budget
  и transport hints до выпуска детерминированных payload'ов для аудитов.
- `cargo xtask sorafs-admission-fixtures` пакует канонические multi-source
  adverts вместе с downgrade fixtures в
  `fixtures/sorafs_manifest/provider_admission/`.
- Range adverts без `stream_budget` или `transport_hints` отклоняются загрузчиками
  CLI/SDK до планирования, сохраняя multi-source harness в соответствии с
  ожиданиями admission Torii.

## Range endpoints gateway

Gateways принимают детерминированные HTTP-запросы, отражающие метаданные объявлений.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Требование | Детали |
|------------|--------|
| **Headers** | `Range` (один интервал, выровненный по chunk offsets), `dag-scope: block`, `X-SoraFS-Chunker`, опциональный `X-SoraFS-Nonce` и обязательный base64 `X-SoraFS-Stream-Token`. |
| **Responses** | `206` с `Content-Type: application/vnd.ipld.car`, `Content-Range`, описывающим выданный интервал, метаданными `X-Sora-Chunk-Range` и эхо headers chunker/token. |
| **Failure modes** | `416` для неверно выровненных диапазонов, `401` для отсутствующих/невалидных токенов, `429` при превышении stream/byte budget. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch одного chunk с теми же headers плюс детерминированный digest chunk.
Полезно для ретраев или forensic downloads, когда CAR slices не нужны.

## Workflow мульти-источникового оркестратора

Когда включен SF-6 multi-source fetch (Rust CLI через `sorafs_fetch`,
SDKs через `sorafs_orchestrator`):

1. **Собрать входные данные** — декодировать план chunk'ов manifest, получить
   последние adverts и, опционально, передать telemetry snapshot
   (`--telemetry-json` или `TelemetrySnapshot`).
2. **Построить scoreboard** — `Orchestrator::build_scoreboard` оценивает
   пригодность и записывает причины отказа; `sorafs_fetch --scoreboard-out`
   сохраняет JSON.
3. **Планировать chunk'и** — `fetch_with_scoreboard` (или `--plan`) применяет
   range-ограничения, stream budget, лимиты retry/peer (`--retry-budget`,
   `--max-peers`) и выпускает stream token в scope manifest для каждого запроса.
4. **Проверить receipts** — результаты включают `chunk_receipts` и
   `provider_reports`; CLI summary сохраняют `provider_reports`, `chunk_receipts`
   и `ineligible_providers` для evidence bundles.

Распространенные ошибки, возвращаемые операторам/SDKs:

| Ошибка | Описание |
|--------|----------|
| `no providers were supplied` | Нет подходящих записей после фильтрации. |
| `no compatible providers available for chunk {index}` | Несовместимость диапазона или бюджета для конкретного chunk. |
| `retry budget exhausted after {attempts}` | Увеличьте `--retry-budget` или исключите сбойные peers. |
| `no healthy providers remaining` | Все провайдеры отключены после повторных отказов. |
| `streaming observer failed` | Downstream CAR writer завершился с ошибкой. |
| `orchestrator invariant violated` | Соберите manifest, scoreboard, telemetry snapshot и CLI JSON для triage. |

## Телеметрия и доказательства

- Метрики, эмитируемые оркестратором:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (с тегами manifest/region/provider). Укажите `telemetry_region` в config или
  через CLI флаги, чтобы дашборды разделяли по флоту.
- CLI/SDK fetch summaries включают сохраненный scoreboard JSON, chunk receipts и
  provider reports, которые должны входить в rollout bundles для gate'ов SF-6/SF-7.
- Gateway handlers публикуют `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`,
  чтобы SRE dashboards коррелировали решения оркестратора с поведением сервера.

## CLI и REST хелперы

- `iroha app sorafs pin list|show`, `alias list` и `replication list` оборачивают
  pin-registry REST endpoints и печатают сырой Norito JSON с блоками attestation
  для аудиторских доказательств.
- `iroha app sorafs storage pin` и `torii /v1/sorafs/pin/register` принимают Norito
  или JSON manifests плюс опциональные alias proofs и successors; malformed proofs
  возвращают `400`, stale proofs дают `503` с `Warning: 110`, а hard-expired proofs
  возвращают `412`.
- REST endpoints (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) включают структуры attestation, чтобы клиенты могли
  проверить данные относительно последних block headers перед действием.

## Ссылки

- Каноническая спецификация:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito типы: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI хелперы: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Crate оркестратора: `crates/sorafs_orchestrator`
- Пак дашбордов: `dashboards/grafana/sorafs_fetch_observability.json`
