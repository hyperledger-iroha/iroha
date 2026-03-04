---
lang: ru
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
На этой странице отражено `docs/source/sns/bulk_onboarding_toolkit.md`, внешнее
операторы рассмотрели те же рекомендации SN-3b без клонирования репозитория.
:::

# Набор инструментов для массовой адаптации SNS (SN-3b)

**Ссылка:** SN-3b "Инструменты для массового внедрения"  
**Артефакты:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Крупные регистраторы часто заранее подготавливают стоимость регистраций `.sora` или
`.nexus` одинаковыми одобрениями управления и расчетных рельсов. Ручная сборка
Полезные данные JSON или повторный запуск CLI не масштабируются, поэтому используется SN-3b.
определённый конструктор CSV-to-Norito, который готовит структуру
`RegisterNameRequestV1` для Torii или CLI. Хелпер заранее валидирует каждый текст,
выдает агрегированный манифест и опциональный построчный JSON, и может отправить
полезные нагрузки автоматически, запись структурированных квитанций для аудитов.

## 1. Схема CSV

Парсеру требуется такой формат заголовка (порядок гибкий):

| Колонка | Обязательно | Описание |
|---------|-------------|----------|
| `label` | Да | Запрошенная метка (допускается смешанный случай; инструмент нормализует по Норме v1 и УТС-46). |
| `suffix_id` | Да | Суффикс числового идентификатора (десятичный или `0x` hex). |
| `owner` | Да | Строка AccountId (литерал IH58; необязательная подсказка @domain) регистрация владельца. |
| `term_years` | Да | Целое число `1..=255`. |
| `payment_asset_id` | Да | Активный расчет (например, `xor#sora`). |
| `payment_gross` / `payment_net` | Да | Беззнаковые целые, представляющие собой важные активы. |
| `settlement_tx` | Да | JSON-значение или строка, описывающая платежную транзакцию или хеш. |
| `payment_payer` | Да | AccountId, авторизовавший платеж. |
| `payment_signature` | Да | JSON или строка с доказательством предоставляется управляющему или казначейству. |
| `controllers` | Необязательно | Список адресов контроллера, разделенный `;` или `,`. По умолчанию `[owner]`. |
| `metadata` | Необязательно | Встроенный JSON или `@path/to/file.json` с подсказками преобразователя, записями TXT и т. д. д. По умолчанию `{}`. |
| `governance` | Необязательно | Встроенный JSON или `@path` на `GovernanceHookV1`. `--require-governance` делаю колонку обязательно. |

Любая колонка может ссылаться на внешний файл, если добавить `@` в начале значения.
Пути разрешают относительно файла CSV.

## 2. Запуск хелпера

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Ключевые опции:

- `--require-governance` отклоняет строки без крючка управления (полезно для
  премиальные аукционы или зарезервированные уступки).
- `--default-controllers {owner,none}` решает, будут ли пустые ячейки контроллера
  падать обратно на хозяина.
- `--controllers-column`, `--metadata-column`, и `--governance-column` разрешают
  переименовать опциональные колонки при работе с добычей экспорта.

В случае успеха скрипт записывает агрегированный манифест:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "ih58...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"ih58...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"ih58...",
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

Если указано `--ndjson`, каждый `RegisterNameRequestV1` также записывается как
однострочный документ JSON, чтобы автоматизация могла стримить запрос прямо в
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```## 3. Автоматизированные отправки

###3.1 Режим Torii REST

Укажите `--submit-torii-url` и либо `--submit-token`, либо `--submit-token-file`,
чтобы отправить каждую запись манифеста напрямую в Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Помощник делает один `POST /v1/sns/registrations` на запрос и останавливается при
  HTTP первые деньги. Ответы включаются в регистрацию, как записано NDJSON.
- `--poll-status` повторно запрашивает `/v1/sns/registrations/{selector}` после
  каждое отправление (до `--poll-attempts`, по умолчанию 5), чтобы проверить
  видимость записи. Укажите `--suffix-map` (JSON-сопоставление `suffix_id` в значениях
  "суффикс"), чтобы инструмент мог вывести `{label}.{suffix}` для опроса.
- Настройки: `--submit-timeout`, `--poll-attempts`, и `--poll-interval`.

###3.2 Режим ироха CLI

Чтобы прогнать каждую запись манифеста через CLI, задайте путь к бинарнику:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Контроллеры должны быть записи `Account` (`controller_type.kind = "Account"`),
  потому что CLI сейчас поддерживает только контроллеры на основе учетных записей.
- Метаданные и объекты управления создаются во временных файлах при каждом запросе и
  передаются в `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout/stderr и коды вывода CLI регистрируются; ненулевые коды прерывают запуск.

Оба режима данных можно запускать вместе (Torii и CLI) для перекрестной проверки.
резервные способы развертывания регистратора или повторения.

### 3.3 Квитанции сообщений

При `--submission-log <path>` в скрипте добавления NDJSON записано:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Успешные ответы Torii включают структурные поля из `NameRecordV1` или
`RegisterNameResponseV1` (например, `record_status`, `record_pricing_class`,
И18НИ00000082Х, И18НИ00000083Х, И18НИ00000084Х, И18НИ00000085Х,
`label`), чтобы дашборды и отчеты управления можно было парсить регистрацию бесплатно
текст. Прикрепите этот журнал к билетам регистратора вместе с манифестом для
воспроизводимого доказательства.

## 4. Автоматизация релизов портала

CI и портал вакансий звонит `docs/portal/scripts/sns_bulk_release.sh`, который
оборачивает помощник и сохраняет документы под `artifacts/sns/releases/<timestamp>/`:

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
   исходный CSV в каталоге релиза.
2. Отправляет манифест через Torii и/или CLI (при настройке), записья.
   `submissions.log` со структурированными квитанциями выше.
3. Формирует `summary.json` с описанием релиза (пути, Torii URL, путь CLI,
   временная метка), чтобы портал автоматизации мог загрузить пакет в хранилище.
   артефактов.
4. Генерирует `metrics.prom` (переопределение через `--metrics`) с Prometheus-совместимыми.
   счетчиками для общих чисел запросов, распределением суффиксов, сумм по активам
   и результаты отправки. Сводка JSON ссылается на этот файл.

Рабочие процессы просто архивируют каталог релиза как единый, дублированный документ.
все необходимое для аудита.

##5. Телеметрия и дашборды

Файл метрик, сгенерированный `sns_bulk_release.sh`, содержит следующие серии:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```Передайте `metrics.prom` в коляску Prometheus (например, через Promtail или пакетную обработку).
импортер), чтобы регистраторы, распорядители и коллеги по управлению рассмотрели соглашение
прогресс объемных технологий. Дашборд Grafana
`dashboards/grafana/sns_bulk_release.json` визуализирует те же данные: количество
по суффиксам, объему денежных средств и конвертам денежных/неускоренных отправлений. Дашборд
фильтруется по `release`, чтобы аудиторы могли изучить один CSV-прогон.

## 6. Проверка и отказ в режимах

- **Канонизация меток:** входные данные нормализуются Python IDNA + строчные буквы и
  фильтрами-символами Норма v1. Недействительные метки быстро падают до сетевых вызовов.
– **Цифровые ограничения:** идентификаторы суффиксов, годы семестра и подсказки по ценам должны быть в
  внутри `u16` и `u8`. Поля денежных средств принимают десятичные или шестнадцатеричные числа.
  `i64::MAX`.
- **Разбор метаданных/управления:** встроенный JSON парсится напрямую; ссылки на файлы
  CSV разрешены. Отсутствие метаданных приводит к необходимости проверки.
- **Контроллеры:** пустые ячейки соблюдают `--default-controllers`. Указывайте
  явные назначенные контроллеры (например, `ih58...;ih58...`) при делегировании невладельцем.

Ошибки сообщаются с контекстными номерами строк (например
`error: row 12 term_years must be between 1 and 255`). Скрипт выходит с кодом `1`
при ошибках валидации и `2`, если путь CSV отсутствует.

## 7. Тестирование и регистрация

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` раскрывает синтаксический анализ CSV,
  вывод NDJSON, управление исполнением и путь отправки данных через CLI или Torii.
- Хелпер написан на чистом Python (без дополнительных зависимостей) и работает.
  везде, где доступен `python3`. История коммитов отслеживается рядом с CLI в
  в основном репозитории для воспроизводимости.

Для продакшн-запусков используйте сгенерированный манифест и пакет NDJSON.
регистратор билетов, чтобы стюарды могли воспроизвести точные полезные нагрузки, отправленные
в Torii.