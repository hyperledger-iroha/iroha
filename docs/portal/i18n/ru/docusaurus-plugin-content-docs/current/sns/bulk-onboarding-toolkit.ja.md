---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 96dd26a970202077113ffe36670c9060393d76e7d0b15f7364c6387b12f8b972
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: bulk-onboarding-toolkit
lang: ru
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/bulk_onboarding_toolkit.md`, чтобы внешние
операторы видели те же рекомендации SN-3b без клонирования репозитория.
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

**Ссылка roadmap:** SN-3b "Bulk onboarding tooling"  
**Артефакты:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Крупные registrars часто заранее подготавливают сотни регистраций `.sora` или
`.nexus` с одинаковыми одобрениями управления и settlement rails. Ручная сборка
JSON payloads или повторный запуск CLI не масштабируются, поэтому SN-3b поставляет
детерминированный CSV-to-Norito builder, который готовит структуры
`RegisterNameRequestV1` для Torii или CLI. Хелпер заранее валидирует каждую строку,
выдает агрегированный manifest и опциональный построчный JSON, и может отправлять
payloads автоматически, записывая структурированные receipts для аудитов.

## 1. Схема CSV

Парсер требует следующую строку заголовка (порядок гибкий):

| Колонка | Обязательно | Описание |
|---------|-------------|----------|
| `label` | Да | Запрошенная метка (допускается mixed case; инструмент нормализует по Norm v1 и UTS-46). |
| `suffix_id` | Да | Числовой идентификатор суффикса (десятичный или `0x` hex). |
| `owner` | Да | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Да | Целое число `1..=255`. |
| `payment_asset_id` | Да | Актив settlement (например `xor#sora`). |
| `payment_gross` / `payment_net` | Да | Беззнаковые целые, представляющие единицы актива. |
| `settlement_tx` | Да | JSON значение или строка, описывающая платежную транзакцию или hash. |
| `payment_payer` | Да | AccountId, авторизовавший платеж. |
| `payment_signature` | Да | JSON или строка с доказательством подписи steward или treasury. |
| `controllers` | Optional | Список адресов controller, разделенный `;` или `,`. По умолчанию `[owner]`. |
| `metadata` | Optional | Inline JSON или `@path/to/file.json` с resolver hints, TXT записями и т. д. По умолчанию `{}`. |
| `governance` | Optional | Inline JSON или `@path` на `GovernanceHookV1`. `--require-governance` делает колонку обязательной. |

Любая колонка может ссылаться на внешний файл, если добавить `@` в начале значения.
Пути разрешаются относительно файла CSV.

## 2. Запуск хелпера

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Ключевые опции:

- `--require-governance` отклоняет строки без governance hook (полезно для
  premium аукционов или reserved assignments).
- `--default-controllers {owner,none}` решает, будут ли пустые controller ячейки
  падать обратно на owner.
- `--controllers-column`, `--metadata-column`, и `--governance-column` позволяют
  переименовать опциональные колонки при работе с upstream exports.

В случае успеха скрипт пишет агрегированный manifest:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

Если указан `--ndjson`, каждый `RegisterNameRequestV1` также записывается как
однострочный JSON документ, чтобы автоматизация могла стримить запросы прямо в
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. Автоматизированные отправки

### 3.1 Режим Torii REST

Укажите `--submit-torii-url` и либо `--submit-token`, либо `--submit-token-file`,
чтобы отправлять каждую запись manifest напрямую в Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Хелпер делает один `POST /v1/sns/registrations` на запрос и останавливается при
  первой HTTP ошибке. Ответы добавляются в лог как NDJSON записи.
- `--poll-status` повторно запрашивает `/v1/sns/registrations/{selector}` после
  каждой отправки (до `--poll-attempts`, по умолчанию 5), чтобы подтвердить
  видимость записи. Укажите `--suffix-map` (JSON маппинг `suffix_id` в значения
  "suffix"), чтобы инструмент мог вывести `{label}.{suffix}` для polling.
- Настройки: `--submit-timeout`, `--poll-attempts`, и `--poll-interval`.

### 3.2 Режим iroha CLI

Чтобы прогнать каждую запись manifest через CLI, задайте путь к бинарнику:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Controllers должны быть `Account` entries (`controller_type.kind = "Account"`),
  потому что CLI сейчас поддерживает только account-based controllers.
- Metadata и governance blobs пишутся во временные файлы на каждый запрос и
  передаются в `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout/stderr и коды выхода CLI логируются; ненулевые коды прерывают запуск.

Оба режима отправки можно запускать вместе (Torii и CLI) для перекрестной проверки
registrar deployments или репетиций fallback путей.

### 3.3 Квитанции отправки

При `--submission-log <path>` скрипт добавляет NDJSON записи:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Успешные ответы Torii включают структурированные поля из `NameRecordV1` или
`RegisterNameResponseV1` (например `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`), чтобы дашборды и отчеты управления могли парсить лог без свободного
текста. Прикрепите этот лог к registrar тикетам вместе с manifest для
воспроизводимого доказательства.

## 4. Автоматизация релизов портала

CI и portal jobs вызывают `docs/portal/scripts/sns_bulk_release.sh`, который
оборачивает хелпер и сохраняет артефакты под `artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

Скрипт:

1. Создает `registrations.manifest.json`, `registrations.ndjson` и копирует
   исходный CSV в директорию релиза.
2. Отправляет manifest через Torii и/или CLI (при настройке), записывая
   `submissions.log` со структурированными квитанциями выше.
3. Формирует `summary.json` с описанием релиза (пути, Torii URL, путь CLI,
   timestamp), чтобы автоматизация портала могла загрузить bundle в хранилище
   артефактов.
4. Генерирует `metrics.prom` (override через `--metrics`) с Prometheus-совместимыми
   счетчиками для общего числа запросов, распределения суффиксов, сумм по активам
   и результатов отправки. Summary JSON ссылается на этот файл.

Workflows просто архивируют директорию релиза как единый артефакт, содержащий
все необходимое для аудита.

## 5. Телеметрия и дашборды

Файл метрик, сгенерированный `sns_bulk_release.sh`, содержит следующие серии:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Передайте `metrics.prom` в Prometheus sidecar (например через Promtail или batch
importer), чтобы registrars, stewards и governance peers видели согласованный
прогресс bulk процессов. Дашборд Grafana
`dashboards/grafana/sns_bulk_release.json` визуализирует те же данные: количество
по суффиксам, объем платежей и соотношение успешных/неуспешных отправок. Дашборд
фильтруется по `release`, чтобы аудиторы могли изучить один CSV прогон.

## 6. Валидация и режимы отказа

- **Label canonicalisation:** входы нормализуются Python IDNA + lowercase и
  фильтрами символов Norm v1. Невалидные метки быстро падают до сетевых вызовов.
- **Numeric guardrails:** suffix ids, term years, и pricing hints должны быть в
  пределах `u16` и `u8`. Поля платежей принимают десятичные или hex числа до
  `i64::MAX`.
- **Metadata/governance parsing:** inline JSON парсится напрямую; ссылки на файлы
  разрешаются относительно CSV. Metadata не-объект приводит к ошибке валидации.
- **Controllers:** пустые ячейки соблюдают `--default-controllers`. Указывайте
  явные списки controllers (например `i105...;i105...`) при делегировании не-owner.

Ошибки сообщаются с контекстными номерами строк (например
`error: row 12 term_years must be between 1 and 255`). Скрипт выходит с кодом `1`
при ошибках валидации и `2`, если путь CSV отсутствует.

## 7. Тестирование и происхождение

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` покрывает CSV парсинг,
  вывод NDJSON, enforcement governance и пути отправки через CLI или Torii.
- Хелпер написан на чистом Python (без дополнительных зависимостей) и работает
  везде, где доступен `python3`. История коммитов отслеживается рядом с CLI в
  основном репозитории для воспроизводимости.

Для продакшн запусков прикладывайте сгенерированный manifest и NDJSON bundle к
registrar тикету, чтобы stewards могли воспроизвести точные payloads, отправленные
в Torii.
