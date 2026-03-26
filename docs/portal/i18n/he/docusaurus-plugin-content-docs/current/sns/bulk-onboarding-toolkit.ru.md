---
lang: he
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/bulk_onboarding_toolkit.md`, чтобы внешние
אופטימיזציה של SN-3b ללא רפואה של קליפורניה.
:::

# ערכת הכלים לכניסה לכמות גדולה של SNS (SN-3b)

** מפת דרכים:** SN-3b "כלי עבודה בכמות גדולה"  
**ארכיון:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

רשמי Крупные часто заранее подготавливают сотни регистраций `.sora` או
`.nexus` с одинаковыми одобрениями управления и מסילות התיישבות. Ручная сборка
מטענים של JSON או תוכנת CLI ללא שימוש, תמונה של SN-3b.
детерминированный CSV-to-Norito בונה, который готовит структуры
`RegisterNameRequestV1` ל-Torii או CLI. Хелпер заранее валидирует каждую строку,
выдает агрегированный מניפסט и опциональный построчный JSON, и может отправлять
מטענים автоматически, записывая структурированные קבלות для аудитов.

## 1. קובץ CSV

Парсер требует следующую строку заголовка (порядок гибкий):

| Колонка | Обязательно | Описание |
|--------|----------------|--------|
| `label` | Да | Запрошенная метка (מקרה מעורב; התקן נורמאלי על פי Norm v1 ו-UTS-46). |
| `suffix_id` | Да | Числовой идентификатор суффикса (десятичный или `0x` hex). |
| `owner` | Да | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Да | Целое число `1..=255`. |
| `payment_asset_id` | Да | יישוב Актив (например `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Да | Беззнаковые целые, представляющие единицы актива. |
| `settlement_tx` | Да | JSON מצא או שיטות, אופציונליות של רשתות או hash. |
| `payment_payer` | Да | AccountId, авторизовавший платеж. |
| `payment_signature` | Да | JSON או ארגון אוצר אוצר. |
| `controllers` | אופציונלי | בקר Список адресов, разделенный `;` או `,`. По умолчанию `[owner]`. |
| `metadata` | אופציונלי | Inline JSON או `@path/to/file.json` עם רמזים לפתרון, TXT записями и т. д. По умолчанию `{}`. |
| `governance` | אופציונלי | Inline JSON או `@path` על `GovernanceHookV1`. `--require-governance` делает колонку обязательной. |

Любая колонка может ссылаться на внешний файл, если добавить `@` в начале значения.
Пути разрешаются относительно файла CSV.

## 2. Запуск хелпера

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

אפשרויות נוספות:

- `--require-governance` отклоняет строки без הוק ממשל (полезно для
  premium аукционов или משימות שמורות).
- `--default-controllers {owner,none}` решает, будут ли пустые בקר ячейки
  падать обратно на הבעלים.
- `--controllers-column`, `--metadata-column`, ו-`--governance-column` позволяют
  переименовать опциональные колонки при работе с יצוא במעלה הזרם.

В случае успеха скрипт пишет агрегированный מניפסט:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
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
         https://torii.sora.net/v1/sns/names
  done
```## 3. Автоматизированные отправки

### 3.1 Режим Torii REST

Укажите `--submit-torii-url` и либо `--submit-token`, либо `--submit-token-file`,
чтобы отправлять каждую запись מניפסט напрямую в Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Хелпер делает один `POST /v1/sns/names` על ספרוס או שירותים
  первой HTTP ошибке. Ответы добавляются в лог как NDJSON записи.
- `--poll-status` פריט זמין עבור `/v1/sns/names/{namespace}/{literal}`
  каждой отправки (עד `--poll-attempts`, по умолчанию 5), чтобы подтвердить
  видимость записи. Укажите `--suffix-map` (JSON маппинг `suffix_id` в значения
  "סיומת"), чтобы инструмент мог вывести `{label}.{suffix}` לסקר.
- אופציות: `--submit-timeout`, `--poll-attempts`, ו-`--poll-interval`.

### 3.2 Режим iroha CLI

Чтобы прогнать каждую запись מניפסט через CLI, задайте путь к бинарнику:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- בקרים должны быть `Account` ערכים (`controller_type.kind = "Account"`),
  потому что CLI сейчас поддерживает только בקרים מבוססי חשבונות.
- מטא נתונים וכתמי ממשל пишутся во временные файлы на каждый запрос и
  передаются в `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout/stderr и коды выхода CLI логируются; ненулевые коды прерывают запуск.

Оба режима отправки можно запускать вместе (Torii ו-CLI) для перекрестной проверки
פריסות רשם или репетиций fallback путей.

### 3.3 Квитанции отправки

פריט `--submission-log <path>` צור קשר עם קוד NDJSON:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

שימוש במכשירים Torii включают структурированные поля из `NameRecordV1` או
`RegisterNameResponseV1` (נקרא `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`), чтобы дашборды и отчеты управления могли парсить лог без свободного
текста. Прикрепите этот лог к הרשם тикетам вместе с Manifest для
воспроизводимого доказательства.

## 4. Автоматизация релизов портала

משרות CI ופורטל вызывают `docs/portal/scripts/sns_bulk_release.sh`, который
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

סקריפט:

1. Создает `registrations.manifest.json`, `registrations.ndjson` וקופי
   исходный CSV в директорию релиза.
2. Отправляет מניפסט через Torii и/или CLI (при настройке), записывая
   `submissions.log` со структурированными квитанциями выше.
3. פורמט `summary.json` עם אופטימיזציה של רישום (פרטים, כתובת אתר Torii, צור CLI,
   חותמת זמן), чтобы автоматизация портала могла загрузить חבילה в хранилище
   артефактов.
4. Генерирует `metrics.prom` (עקוף את через `--metrics`) с Prometheus-совместимыми
   счетчиками для общего числа запросов, распределения суффиксов, сумм по активам
   и результатов отправки. תקציר JSON ссылается на этот файл.

זרימות עבודה просто архивируют директорию релиза как единый артефакт, содержащий
все необходимое для аудита.

## 5. Телеметрия и дашборды

Файл метрик, сгенерированный `sns_bulk_release.sh`, содержит следующие серии:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```הצג את `metrics.prom` ב-Prometheus קרונית צד (לדוגמה, Promtail או batch
יבואן), чтобы רשמים, דיילים ועמיתים לממשל видели согласованный
прогресс בכמות גדולה процессов. Дашборд Grafana
`dashboards/grafana/sns_bulk_release.json` визуализирует те же данные: количество
по суффиксам, объем платежей и соотношение успешных/неуспешных отправок. Дашборд
фильтруется по `release`, чтобы аудиторы могли изучить один CSV прогон.

## 6. Валидация и режимы отказа

- **קנוניזציה של תווית:** входы нормализуются Python IDNA + אותיות קטנות и
  фильтрами символов Norm v1. Невалидные метки быстро падают до сетевых вызовов.
- **מעקות בטיחות מספריים:** מזהי סיומת, שנות טווח, и רמזים לתמחור должны быть в
  פריטים `u16` ו-`u8`. Поля платежей принимают десятичные или hex числа до
  `i64::MAX`.
- **ניתוח מטא נתונים/ממשל:** JSON парсится напрямую מוטבע; ссылки на файлы
  разрешаются относительно CSV. Metadata не-объект приводит к ошибке валидации.
- **בקרים:** пустые ячейки соблюдают `--default-controllers`. Указывайте
  явные списки בקרים (например `soraカタカナ...;soraカタカナ...`) при делегировании не-בעלים.

Ошибки сообщаются с контекстными номерами строк (например
`error: row 12 term_years must be between 1 and 255`). סקריפט выходит с кодом `1`
при ошибках валидации и `2`, если путь CSV отсутствует.

## 7. Тестирование и происхождение

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` покрывает CSV парсинг,
  вывод NDJSON, ממשל אכיפה и пути отправки через CLI או Torii.
- Хелпер написан на чистом Python (ללא дополнительных зависимостей) и работает
  везде, где доступен `python3`. История коммитов отслеживается рядом с CLI в
  основном репозитории для воспроизводимости.

Для продакшн запусков прикладывайте сгенерированный manifest и NDJSON bundle к
רשם тикету, чтобы דיילים могли воспроизвести точные מטענים, отправленные
• Torii.