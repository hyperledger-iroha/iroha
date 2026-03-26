---
lang: ru
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی
В качестве примера можно привести SN-3b, предназначенный для установки на СН-3Б.
:::

# Набор инструментов для массовой адаптации SNS (SN-3b)

**Добавлено:** SN-3b «Инструменты для массовой адаптации»  
**Артефакты:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Регистраторы `.sora` и регистрации `.nexus` и управление
разрешения для расчетных рельсов
ہیں۔ Использование полезных данных JSON и интерфейс командной строки, масштабирование и настройка.
Для SN-3b используется детерминированный построитель CSV-to-Norito, который работает с Torii и CLI.
Структуры `RegisterNameRequestV1` تیار کرتا ہے۔ помощник ہر row کو پہلے
проверить агрегированный манифест или необязательный JSON с разделителями-новой строкой.
Проведите аудит и структурированные поступления, а также полезные нагрузки
خودکار طور پر submit کر سکتا ہے۔

## 1. CSV-файл

Парсер может отобразить строку заголовка или изменить порядок (гибкий порядок):

| Столбец | Требуется | Описание |
|--------|----------|-------------|
| `label` | Да | Запрошенная метка (принимается смешанный регистр; инструмент Norm v1 اور UTS-46 обеспечивает нормализацию). |
| `suffix_id` | Да | Числовой идентификатор суффикса (десятеричный یا `0x`, шестнадцатеричный). |
| `owner` | Да | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Да | Целое число `1..=255`. |
| `payment_asset_id` | Да | Расчетный актив (مثال `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Да | Целые числа без знака и собственные единицы актива представляют собой کریں۔. |
| `settlement_tx` | Да | Значение JSON, литеральная строка, платежная транзакция, хеш и т. д. |
| `payment_payer` | Да | AccountId и авторизация платежа. |
| `payment_signature` | Да | JSON یا литеральная строка جس میں доказательство подписи стюарда/казначейства ہو۔ |
| `controllers` | Необязательно | Адреса учетных записей контроллера — список, разделенный точкой с запятой или запятой. خالی ہونے پر `[owner]`. |
| `metadata` | Необязательно | Встроенный JSON `@path/to/file.json`, подсказки преобразователя, записи TXT и т. д. По умолчанию `{}`۔ |
| `governance` | Необязательно | Встроенный JSON от `@path` до `GovernanceHookV1` или от `GovernanceHookV1`. `--require-governance` в столбце کو لازمی کرتا ہے۔ |

Если вы хотите использовать внешний файл `@`, обратитесь к соответствующему столбцу.
CSV-файл путей относительного разрешения ہوتے ہیں۔

## 2. Помощник چلانا

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Ключевые параметры:

- `--require-governance` перехватчик управления, если строки отклоняются (премиум-класс).
  аукционы или зарезервированные задания (например, مفید).
- `--default-controllers {owner,none}` طے کرتا ہے کہ خالی владелец ячеек контроллера
  аккаунт پر واپس جائیں یا نہیں.
- `--controllers-column`, `--metadata-column`, или `--governance-column` восходящий поток
  экспорт کے ساتھ کام کرتے ہوئے необязательных столбцов, которые можно использовать для экспорта.

Агрегированный манифест сценария:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<i105-account-id>",
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

Например, `--ndjson` и `RegisterNameRequestV1` — однострочный JSON.
Запросы на автоматизацию запроса Torii
Видео трансляции:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```## 3. Автоматическая отправка

### 3.1 Torii Режим REST

`--submit-torii-url` کے ساتھ `--submit-token` یا `--submit-token-file` دیں تاکہ
В манифесте введите запись Torii и нажмите кнопку:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Помощник запросит запрос `POST /v1/sns/names` для HTTP-запроса.
  ошибка или прерывание کرتا ہے۔ Путь к журналу ответов, записи NDJSON и добавление
  ہوتے ہیں۔
- `--poll-status` ہر представление `/v1/sns/names/{namespace}/{literal}` کو
  دوبارہ query کرتا ہے (زیادہ سے زیادہ `--poll-attempts`, по умолчанию 5) تاکہ запись
  видимый ہونے کی تصدیق ہو۔ `--suffix-map` (JSON или `suffix_id` для значений «суффикса»).
  سے карта کرے) فراہم کریں تاکہ инструмент `{label}.{suffix}` литералы получают کر سکے۔
- Настройки: `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### 3.2 режим CLI iroha

Запись манифеста, а также CLI и бинарный путь:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Контроллеры могут иметь записи `Account` (`controller_type.kind = "Account"`)
  Интерфейс командной строки и контроллеры на основе учетных записей предоставляют доступ к данным
- Метаданные и BLOB-объекты управления, запрос и временные файлы, а также временные файлы.
  ہیں اور `iroha sns register --metadata-json ... --governance-json ...` کو پاس
  کئے جاتے ہیں۔
- CLI stdout/stderr и журнал кодов выхода. ненулевые коды запуск کو прерывание کرتے ہیں۔

Режимы отправки и настройки (Torii интерфейс CLI) и регистратор
развертывание перекрестная проверка ہوں یا репетиция резервных копий

### 3.3 Квитанции об отправке

К `--submission-log <path>` можно добавить записи скрипта NDJSON, добавив команду:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Успешные ответы Torii میں `NameRecordV1` یا `RegisterNameResponseV1` سے نکالے
Структурированные поля شامل ہوتے ہیں (مثال `record_status`, `record_pricing_class`,
И18НИ00000080Х, И18НИ00000081Х, И18НИ00000082Х, И18НИ00000083Х,
`label`) Панели мониторинга и журналы отчетов управления, а также текст в произвольной форме.
анализировать کر سکیں۔ В журнале есть манифест и билеты регистратора, а также прикрепленные файлы.
воспроизводимые доказательства رہے۔

## 4. Автоматизация выпуска портала Документов

Портал CI اور вакансий `docs/portal/scripts/sns_bulk_release.sh` можно позвонить в службу поддержки
помощник для обертывания для артефактов `artifacts/sns/releases/<timestamp>/` для
Адрес магазина:

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

1. `registrations.manifest.json`, `registrations.ndjson` в оригинальном исполнении
   CSV-каталог выпуска или каталог выпуска
2. Torii или CLI для отправки манифеста или отправки (настройка конфигурации), или
   `submissions.log` میں اوپر والے структурированные квитанции لکھتا ہے۔
3. `summary.json` выдает сообщение о выпуске и описывает описание (пути, URL-адрес Torii,
   Путь CLI, метка времени) Пакет автоматизации портала и хранилище артефактов
   загрузить کر سکے۔
4. `metrics.prom` Загружено (`--metrics` может быть переопределено), а Prometheus-
   счетчики формата ہوتے ہیں: общее количество запросов, распределение суффиксов, итоговые значения активов,
   Результаты подачи заявок۔ сводный файл JSON и ссылка на него

Рабочие процессы: каталог выпуска, артефакт, архив, архив, а также архив.
اب وہ سب کچھ ہے جو کو Audit کے لئے درکار ہے۔

## 5. Панели мониторинга телеметрии

`sns_bulk_release.sh` کے ذریعہ بننے и метрики файла серии درج ذیل раскрывают کرتی ہے:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
````metrics.prom` کو اپنے Prometheus с коляской для корма (مثال کے طور پر Promtail
یا пакетный импортер کے ذریعے) تاکہ регистраторы, стюарды اور коллеги по управлению массой
прогресс پر выровнен رہیں۔ Плата Grafana
`dashboards/grafana/sns_bulk_release.json` на панелях данных. Значение: для каждого суффикса.
учитывается, объем платежей, а также соотношение успешных/неуспешных заявок. Плата `release`
фильтр выбор аудиторов запуск CSV и сверление данных

## 6. Проверка режимов сбоя

- **Каноникализация меток:** входы Python IDNA или строчные буквы, Norm v1
  фильтры символов سے нормализовать ہوتے ہیں۔ неверные метки сетевые вызовы سے پہلے
  быстро провалиться ہوتے ہیں۔
- **Цифровые ограждения:** идентификаторы суффиксов, годы семестра и подсказки по ценам `u16` или `u8`.
  границы کے اندر ہونے چاہئیں۔ Поля платежа десятичные یا шестнадцатеричные целые числа `i64::MAX`
  تک принять کرتے ہیں۔
- **Разбор метаданных и управление:** встроенный анализ JSON в режиме реального времени. файл
  ссылки CSV местоположение کے относительное разрешение ہوتی ہیں۔ Необъектные метаданные
  ошибка проверки دیتا ہے۔
- **Контроллеры:** Ячейки `--default-controllers` имеют честь быть доступным. невладелец
  актеры и делегаты и явные списки контроллеров (مثال `<i105-account-id>;<i105-account-id>`)۔

Номера контекстных строк ошибок کے ساتھ report ہوتے ہیں (مثال
`error: row 12 term_years must be between 1 and 255`). Ошибки проверки скрипта پر
`1` — Отсутствует путь CSV. `2` — нет выхода из списка.

## 7. Проверка происхождения

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` Анализ CSV, NDJSON
  эмиссия, принудительное управление, пути отправки CLI/Torii и обложка.
- Помощник на чистом Python ہے (с учетом зависимостей) Доступен `python3` دستیاب
  ہو وہاں چلتا ہے۔ История коммитов CLI Доступ к основному репозиторию и дорожке ہوتی ہے
  Воспроизводимость

Производственные запуски, создание манифеста, пакета NDJSON и билета регистратора.
ساتھ Attach کریں تاکہ стюарды Torii کو отправить ہونے والے точный повтор полезной нагрузки
کر سکیں۔