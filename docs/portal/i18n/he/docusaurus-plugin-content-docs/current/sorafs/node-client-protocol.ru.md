---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Протокол SoraFS узел ↔ клиент

Этот гид резюмирует каноническое определение протокола в
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Используйте upstream-спецификацию для байтовых Norito פריסה и changelog'ов;
портальная копия держит операционные акценты рядом с остальными runbook SoraFS.

## Объявления провайдера и валидация

Провайдеры SoraFS распространяют payload'ы `ProviderAdvertV1` (см.
`crates/sorafs_manifest::provider_advert`), подписанные управляемым оператором.
Объявления фиксируют метаданные обнаружения и מעקות בטיחות, которые
мульти-источниковый оркестратор применяет на рантайме.

- **Срок действия** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Провайдеры
  должны обновлять каждые 12 часов.
- **יכולת TLV** — TLV-список рекламирует транспортные возможности (Torii,
  QUIC+Noise, ממסרי SoraNet, הרחבות ספקים). Неизвестные коды можно
  пропускать при `allow_unknown_capabilities = true`, следуя рекомендациям
  גריז.
- **רמזים ל-QoS** — שכבה `availability` (חם/חם/קר), максимальная задержка
  извлечения, лимит конкурентности и опциональный תקציב זרם. QoS должен
  совпадать с наблюдаемой телеметрией и проверяется при כניסה.
- **נקודות קצה ונושאי מפגש** — כתובת URL של שירות конкретные с TLS/ALPN
  метаданными плюс נושאי גילוי, на которые клиенты подписываются при
  построении ערכות שמירה.
- ** Политика разнообразия путей** — `min_guard_weight`, לולימיטים מאווררים למען
  AS/пулов ו-`provider_failure_threshold` обеспечивают детерминированные
  multi-peer fetch'и.
- **Идентификаторы профилей** — провайдеры обязаны публиковать канонический
  ידית (לדוגמה, `sorafs.sf1@1.0.0`); опциональные `profile_aliases` помогают
  миграции старых клиентов.

Правила валидации отвергают нулевой הימור, пустые списки יכולות/נקודות קצה/נושאים,
перепутанные сроки, либо отсутствующие יעדי QoS. מעטפות כניסה
сравнивают тела объявления и предложения (`compare_core_fields`) перед
распространением обновлений.

### אחזור טווח Расширения

Range-совместимые провайдеры включают следующую метадату:

| Поле | Назначение |
|------|--------|
| `CapabilityType::ChunkRangeFetch` | Объявляет `max_chunk_span`, `min_granularity` ו- флаги выравнивания/доказательств. |
| `StreamBudgetV1` | Опциональный envelope конкурентности/תפוקה (`max_in_flight`, `max_bytes_per_sec`, опциональный `burst`). יכולת טווח Требует. |
| `TransportHintV1` | Упорядоченные предпочтения транспорта (например, `torii_http_range`, `quic_stream`, `soranet_relay`). Приоритеты `0–15`, дубли отклоняются. |

כלי עזר:

- מודעה של ספק Пайплайны должны валидировать יכולת טווח, תקציב זרימה
  и רמזים לתחבורה до выпуска детерминированных payload'ов для аудитов.
- `cargo xtask sorafs-admission-fixtures` пакует канонические ריבוי מקורות
  פרסומות вместе с מתקני שדרוג לאחור в
  `fixtures/sorafs_manifest/provider_admission/`.
- טווח פרסומות ללא `stream_budget` או `transport_hints` отклоняются загрузчиками
  CLI/SDK לרשות הפלסטינית, сохраняя רתמה מרובת מקורות в соответствии с
  ожиданиями כניסה Torii.

## שער נקודות קצה טווחשערים מנוהלים HTTP-זאפרוסים, אופטימיזציית אמצעים.

### `GET /v2/sorafs/storage/car/{manifest_id}`

| Требование | Детали |
|------------|--------|
| **כותרות** | `Range` (один интервал, выровненный по chunk offsets), `dag-scope: block`, `X-SoraFS-Chunker`, опциональный Norito base64 `X-SoraFS-Stream-Token`. |
| **תגובות** | `206` с `Content-Type: application/vnd.ipld.car`, `Content-Range`, описывающим выданный интервал, метаданными Norito метаданными Norito |
| **מצבי כשל** | `416` ל-неверно выровненных диапазонов, `401` ל-отсутствующих/невалидных токен500, I1800NI300, I1800NI30. תקציב זרם/בתים זמין. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

אחזר одного chunk с теми же כותרות плюс детерминированный digest chunk.
Полезно для ретраев или הורדות משפטיות, когда CAR slices не нужны.

## זרימת עבודה мульти-источникового оркестратора

Когда включен SF-6 ריבוי מקורות אחזור (Rust CLI через `sorafs_fetch`,
ערכות SDK של `sorafs_orchestrator`):

1. **Собрать входные данные** — декодировать план chunk'ов manifest, получить
   последние פרסומות и, опционально, передать תמונת מצב של טלמטריה
   (`--telemetry-json` או `TelemetrySnapshot`).
2. **לוח התוצאות Построить** — `Orchestrator::build_scoreboard` оценивает
   пригодность и записывает причины отказа; `sorafs_fetch --scoreboard-out`
   сохраняет JSON.
3. **Планировать chunk'и** — `fetch_with_scoreboard` (או `--plan`) применяет
   range-ограничения, תקציב זרם, лимиты ניסיון חוזר/עמית (`--retry-budget`,
   `--max-peers`) и выпускает אסימון זרם במניפסט scope для каждого запроса.
4. **צריך קבלות** — результаты включают `chunk_receipts` и
   `provider_reports`; סיכום CLI сохраняют `provider_reports`, `chunk_receipts`
   ו `ineligible_providers` עבור חבילות ראיות.

Распространенные ошибки, возвращаемые операторам/SDKs:

| Ошибка | Описание |
|--------|--------|
| `no providers were supplied` | Нет подходящих записей после фильтрации. |
| `no compatible providers available for chunk {index}` | Несовместимость диапазона или бюджета для конкретного chunk. |
| `retry budget exhausted after {attempts}` | Увеличьте `--retry-budget` или исключите сбойные עמיתים. |
| `no healthy providers remaining` | Все провайдеры отключены после повторных отказов. |
| `streaming observer failed` | במורד הזרם CAR סופר завершился с ошибкой. |
| `orchestrator invariant violated` | צור מניפסט, לוח תוצאות, תמונת מצב של טלמטריה ו-CLI JSON לטריאג'. |

## טלמטריה ומסמכים

- Метрики, эмитируемые оркестратором:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (с тегами מניפסט/אזור/ספק). צור `telemetry_region` ב-config или
  через CLI флаги, чтобы дашборды разделяли по флоту.
- סיכומי אחזור של CLI/SDK включают сохраненный לוח תוצאות JSON, קבלות חתיכות и
  דוחות ספקים, קבצים נוספים להפצת חבילות ל-gate'ов SF-6/SF-7.
- מטפלי שערים публикуют `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`,
  чтобы SRE לוחות מחוונים коррелировали решения оркестратора с поведением сервера.

## CLI и REST хелперы- `iroha app sorafs pin list|show`, `alias list` ו-`replication list` оборачивают
  נקודות קצה של REST של pin-registry ואישור Norito JSON עם סמל
  для аудиторских доказательств.
- `iroha app sorafs storage pin` ו-`torii /v2/sorafs/pin/register` פרינט Norito
  или JSON manifests плюс опциональные כינוי הוכחות и יורשים; הוכחות שגויות
  возвращают `400`, הוכחות מעופשות дают `503` с `Warning: 110`, הוכחות שפג תוקפן
  возвращают `412`.
- נקודות קצה REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`,
  `/v2/sorafs/replication`) включают структуры אישור, чтобы клиенты могли
  проверить данные относительно последних לחסום כותרות перед действием.

## Ссылки

- Каноническая спецификация:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito טיפוסים: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- סוללות CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- ארגז оркестратора: `crates/sorafs_orchestrator`
- תקליטורים: `dashboards/grafana/sorafs_fetch_observability.json`