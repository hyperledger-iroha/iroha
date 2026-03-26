---
lang: ru
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::обратите внимание на Фуэнте каноника
Refleja `docs/source/sns/bulk_onboarding_toolkit.md` для внешних операций
неправильное руководство по SN-3b без клонирования репозитория.
:::

# Инструментарий для подключения к SNS (SN-3b)

**Справочная информация по плану действий:** SN-3b «Инструменты для массового внедрения».  
**Артефакты:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Большие регистраторы в меню предварительной подготовки реестров `.sora` или `.nexus`
con las mismas aprobaciones de gobernanza y Rails de Settlement. Полезные данные Armar в формате JSON
мы вручную или выбираем CLI без необходимости, так как SN-3b входит в состав конструктора
определяется CSV в Norito, который готовит структуры `RegisterNameRequestV1` для
Torii или CLI. El helper valida cada fila de antemano, emite tanto un manifico
объединенный в формате JSON, ограниченный дополнительными линиями, и может быть отправлен
полезные нагрузки автоматически регистрируются при получении estructurados для аудиторий.

## 1. Эскема CSV

Парсеру требуется следующий файл инкабезадо (гибкий порядок):

| Колонна | Требуется | Описание |
|---------|-----------|-------------|
| `label` | Си | Etiqueta solicitada (se acepta mayus/minus; la Herramienta Normaliza Segun Norm v1 и UTS-46). |
| `suffix_id` | Си | Цифровой идентификатор суфии (десятичный или шестнадцатеричный `0x`). |
| `owner` | Си | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Си | Энтеро `1..=255`. |
| `payment_asset_id` | Си | Активация урегулирования (например, `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Си | Enteros sin Signo que представляет собой unidades nativas del activo. |
| `settlement_tx` | Си | Используйте JSON или кадену, описывающую транзакцию паго или хэша. |
| `payment_payer` | Си | AccountId, авторизованный на странице. |
| `payment_signature` | Си | JSON или литерал с запросом фирмы стюарда или tesoreria. |
| `controllers` | Необязательно | Список разделен между точками и точками или направлениями контроллера движения. По дефекту `[owner]`, если его опустить. |
| `metadata` | Необязательно | Встроенный JSON или `@path/to/file.json`, который показывает подсказки преобразователя, регистры TXT и т. д. Из-за дефекта `{}`. |
| `governance` | Необязательно | Встроенный JSON или `@path` добавляется к `GovernanceHookV1`. `--require-governance` Exige Esta Columna. |

Вы можете ссылаться на внешний архив с указанием ценности цели с `@`.
Руты будут восстановлены относительно архива CSV.

## 2. Вызов помощника

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Варианты клавы:

- `--require-governance` rechaza filas без крючка de gobernanza (используется для
  subastas premium или asignaciones reservadas).
- `--default-controllers {owner,none}` решит, какие контроллеры будут удалены
  владелец vuelven a la cuenta.
- `--controllers-column`, `--metadata-column`, y `--governance-column` разрешение
  дополнительные столбцы могут быть изменены, когда вы работаете с экспортом вверх по течению.

В случае выхода из сценария опишите объединенное объединение:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<katakana-i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<katakana-i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<katakana-i105-account-id>",
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
```Если вы соответствуете `--ndjson`, каждый `RegisterNameRequestV1` также будет описан как
Единственная строка документа JSON, позволяющая автоматизировать передачу
просьба адресовать Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Автоматизированные отправки

### 3.1 Мод Torii REST

Специальное `--submit-torii-url` для `--submit-token` или `--submit-token-file` для
Эмпухар каждый раз вводит манифесты прямо на Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Помощник выдает `POST /v1/sns/names` для ходатайства и прерывания беременности
  ошибка праймера HTTP. Ответы будут добавлены в порядок следования журналов регистрации
  НДЖСОН.
- `--poll-status` обратитесь к консультанту `/v1/sns/names/{namespace}/{literal}` после
  cada envio (hasta `--poll-attempts`, по умолчанию 5) для подтверждения регистрации
  это видно. Proporcione `--suffix-map` (JSON de `suffix_id` — «суффикс значений»)
  для того, чтобы получить буквы `{label}.{suffix}` в результате опроса.
- Регулируется: `--submit-timeout`, `--poll-attempts`, y `--poll-interval`.

### 3.2 Режим командной строки ироха

Для входа в манифесты через CLI, укажите путь к бинарному файлу:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Контроллеры должны быть установлены `Account` (`controller_type.kind = "Account"`)
  поэтому CLI фактически отображает только контроллеры на базе компьютеров.
- Лос-капли метаданных и управление записаны в временный архив для
  попросите и сделайте шаг `iroha sns register --metadata-json ... --governance-json ...`.
- Стандартный вывод и поток CLI для зарегистрированных кодов; Лос Кодигос
  нет возможности прервать выброску.

Доступные режимы отправки могут быть отключены (Torii и CLI) для проверки
отключает регистратор или резервные варианты.

### 3.3 Получение отправки

Когда вы используете `--submission-log <path>`, сценарий добавляется к NDJSON, который
каптуран:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Выходные ответы Torii включают в себя экстраструктурированные лагеря
`NameRecordV1` или `RegisterNameResponseV1` (например, `record_status`,
И18НИ00000079Х, И18НИ00000080Х, И18НИ00000081Х,
`registry_event_version`, `suffix_id`, `label`) для панелей мониторинга и отчетов
gobernanza puedan parsear el log sin inspectionar texto free. Дополнительный журнал
Лос-билеты регистратора объединены с манифестом для воспроизводимых доказательств.

## 4. Автоматизация выпуска портала

Работа CI и портала `docs/portal/scripts/sns_bulk_release.sh`,
que envuelve el helper y Guarda Artefactos bajo `artifacts/sns/releases/<timestamp>/`:

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

Эл скрипт:1. Постройте `registrations.manifest.json`, `registrations.ndjson`, и скопируйте эл.
   Исходный файл CSV в каталоге выпуска.
2. Отправьте манифест с использованием Torii и CLI (когда вы настроены), опишите
   `submissions.log` с полученными ответами на поступление.
3. Напишите `summary.json` с описанием выпуска (руты, URL Torii, рута CLI,
   временная метка), чтобы автоматизировать портал, чтобы загрузить пакет a
   альмаценамиенто де артефактос.
4. Создайте `metrics.prom` (переопределите через `--metrics`), который содержит контадоры.
   в формате Prometheus для всех забот, распространения суфий,
   итоговые активы и результаты отправки. Возобновление JSON на этом экране
   архив.

Рабочие процессы просто архивируются в каталоге выпуска как одиночный артефакт,
Что сейчас содержит все, что нужно для аудитории.

## 5. Телеметрия и информационные панели

Архив метрик, созданный для `sns_bulk_release.sh`, выставляется напоказ
серия:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Поставляется `metrics.prom` на коляске Prometheus (например, через Promtail или
партия импортера) для обслуживающих регистраторов, стюардов и парс-де-гобернанса
alineados о большом прогрессе. Эль-табло Grafana
`dashboards/grafana/sns_bulk_release.json` Визуализация данных с помощью панелей
пункт conteos por sufijo, объем pago и соотношение выхода/fallo de envios. Эль
Табличный фильтр для `release`, чтобы аудиторы могли войти в одиночку
Коррида CSV.

## 6. Проверка и модусы падения

- **Нормализация метки:** ввод нормализуется с помощью Python IDNA.
  строчные буквы и фильтры символов Norm v1. Этикетки инвалиды Fallan Rapido Antes
  де куалькье ламада де красный.
– **Guardrails numericos:** идентификаторы суффиксов, годы семестра и подсказки по ценам. deben caer
  в пределах `u16` и `u8`. Los Campos de Pago принимают Enteros decimales o
  шестнадцатеричная хаста `i64::MAX`.
- **Разбор метаданных или управление:** JSON встроен в прямой анализ; лас
  Ссылки на архивы могут быть найдены относительно распространения CSV. Метаданные
  ни один морской объект не может привести к ошибке проверки.
- **Контроллеры:** указаны на белом фоне `--default-controllers`. Пропорционе
  явные списки контроллеров (например, `<katakana-i105-account-id>;<katakana-i105-account-id>`) и делегировать
  актеры без владельца.

Los Fallos se reportan con numeros de fila contextuales (например,
`error: row 12 term_years must be between 1 and 255`). Продажа сценария с кодом
`1` с ошибками проверки и `2`, когда произошел сбой в изменении CSV.

## 7. Тестирование и процедура

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` разбор куба CSV,
  выброс NDJSON, принудительное управление и управление адресами через CLI или Torii.
- El helper es Python Puro (без дополнительных зависимостей) и соответствует другим параметрам.
  lugar donde `python3` это доступно. El historial de commits se rastrea junto
  а-ля CLI в основном репозитории для воспроизводства.Для производства, добавьте созданные манифесты и пакет NDJSON al.
билет регистратора, чтобы стюарды могли точно воспроизвести полезную нагрузку
que se enviaron a Torii.