---
lang: ru
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
يعكس `docs/source/sns/bulk_onboarding_toolkit.md` حتى يرى المشغلون الخارجيون
Был создан SN-3b в Колумбийском университете.
:::

# عدة ادوات التهيئة بالجملة لـ SNS (SN-3b)

**Отзыв:** SN-3b «Инструменты для массового внедрения»  
**اثار:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Он был отправлен в службу `.sora` и `.nexus`.
Наслаждайтесь исчезновением. Создание полезных данных в формате JSON и просмотр изображений
CLI для запуска SN-3b builder в формате CSV Norito для загрузки
`RegisterNameRequestV1` для Torii в CLI. يتحقق المساعد من كل صف مسبقا،
Создайте файл манифеста и JSON-файл, созданный в соответствии с требованиями пользователя.
полезная нагрузка находится в зоне действия защитного кожуха.

## 1. Загрузить CSV

Ответ на вопрос:

| عمود | مطلوب | الوصف |
|--------|-------|-------|
| `label` | نعم | Приложение для проверки (в стандартном исполнении; стандартная версия Norm v1 и UTS-46). |
| `suffix_id` | نعم | Введите код (код `0x` hex). |
| `owner` | نعم | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | نعم | عدد صحيح `1..=255`. |
| `payment_asset_id` | نعم | Это приложение (код `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | نعم | Это было сделано для того, чтобы покончить с собой. |
| `settlement_tx` | نعم | Используйте JSON для обработки данных и хэша. |
| `payment_payer` | نعم | AccountId الذي فوض الدفع. |
| `payment_signature` | نعم | JSON используется в качестве стюарда и тренера. |
| `controllers` | ختياري | Для этого необходимо установить контроллер управления. Это `[owner]`. |
| `metadata` | ختياري | Встроенный JSON и `@path/to/file.json` в качестве преобразователя и TXT. Приложение `{}`. |
| `governance` | ختياري | Встроенный JSON — `@path` вместо `GovernanceHookV1`. `--require-governance` находится в разработке. |

Он был главой государства-члена Совета директоров `@`.
Загрузите файл CSV.

## 2. تشغيل المساعد

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Ответ:

- `--require-governance` для крючка حوكمة (для премиум-класса).
  التعيينات المحجوزة).
- `--default-controllers {owner,none}` для контроллеров
  الفارغة تعود الى حساب владелец.
- `--controllers-column`, `--metadata-column`, и `--governance-column`.
  Вы можете экспортировать товары в другие страны.

В манифесте манифеста:

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
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
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

Для `--ndjson` и `RegisterNameRequestV1` используется файл JSON.
Вы можете получить доступ к файлу Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Обратная связь

### 3.1 в Torii REST

`--submit-torii-url` и `--submit-token` и `--submit-token-file`.
В манифесте Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- Установите `POST /v1/sns/names` для подключения к HTTP.
  Позвоните в компанию NDJSON.
- `--poll-status` для `/v1/sns/names/{namespace}/{literal}` для `/v1/sns/names/{namespace}/{literal}`.
  Приложение (حتى `--poll-attempts`, версия 5) было отключено. وفر
  `--suffix-map` (JSON يحول `suffix_id` الى قيم "суффикс") в исходном коде
  Запустите опрос `{label}.{suffix}`.
- Доступные варианты: `--submit-timeout`, `--poll-attempts`, и `--poll-interval`.

### 3.2 в интерфейсе командной строки iroha

Откройте файл манифеста CLI, а затем нажмите кнопку «Сохранить»:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- На контроллерах управления в формате `Account` (`controller_type.kind = "Account"`)
  Интерфейс командной строки обеспечивает доступ к контроллерам, которые можно использовать.
- Создание больших двоичных объектов для метаданных и управления в режиме реального времени.
  Используется `iroha sns register --metadata-json ... --governance-json ...`.
- Выполняется стандартный вывод и стандартный вывод stderr в режиме ожидания. Он сказал, что это не так.

Для запуска программы (Torii и CLI) выполните следующие действия.
Альтернативный вариант - запасной вариант.

### 3.3 Дополнительная информация

Для этого используйте `--submission-log <path>` на сайте NDJSON:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

تتضمن ردود Torii Установите флажок `NameRecordV1` او
`RegisterNameResponseV1` (например, `record_status`, `record_pricing_class`,
И18НИ00000080Х, И18НИ00000081Х, И18НИ00000082Х, И18НИ00000083Х,
`label`). تفتيش
نص حر. ارفق هذا السجل مع تذكرة المسجل بجانب Manifest لاثبات قابل لاعادة
الانتاج.

## 4. اتـمتة اصدار بوابة الوثائق

تستدعي CI والبوابة `docs/portal/scripts/sns_bulk_release.sh` يلف
Ссылка на файл `artifacts/sns/releases/<timestamp>/`:

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

Сообщение:

1. Загрузите `registrations.manifest.json` и `registrations.ndjson` в формате CSV.
   В конце концов.
2. Создайте манифест Torii и CLI (программный интерфейс) и `submissions.log`.
   الايصالات المنظمة اعلاه.
3. Установите `summary.json` в диалоговом окне (запустите Torii, откройте CLI).
   временная метка) لكي تمكن اتـمتة البوابة من رفع الحزمة الى مخزن الاثار.
4. Установите `metrics.prom` (переопределите `--metrics`)
   Prometheus будет отключен от сети и будет отключен от сети.
   Используйте JSON для просмотра.

Рабочие процессы
الاعتمادات للتدقيق.

## 5. Свободный ход

Для получения дополнительной информации о `sns_bulk_release.sh` выполните следующие действия:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

На автомобиле `metrics.prom` с коляской Prometheus (на сайте Promtail).
Он был назначен стюардом Стоуэллом Стюардом и стюардами и капитаном Стивом Стоуардом.
جملة. Grafana `dashboards/grafana/sns_bulk_release.json` تعرض نفس
Он был убит в 1980-х годах в Лос-Анджелесе, где он провел время.
تقوم اللوحة بالتصفية عبر `release` حتى يتمكن المدققون من التعمق في Открыть CSV
واحد.

## 6. التحقق وحالات الفشل- **Обозначенная метка:** يتم تطبيع الادخالات باستخدام Python IDNA с строчными буквами
  Норма v1. Он выступил с речью в газете "Санкт-Петербург".
- **Добавление:** يجب ان تقع, идентификаторы суффиксов, годы семестра и подсказки по ценам.
  `u16` и `u8`. Установите флажок в шестнадцатеричном формате `i64::MAX`.
- **Метаданные управления:** встроенный формат JSON. ويتم حل
  Создайте файл CSV. метаданные доступны для скачивания.
- **Контроллеры:** Установите флажок `--default-controllers`. قدم قوائم
  Контроллер установлен (тип `i105...;i105...`).

Он был показан в 2007 году и в 2007 году в Сейме (Миссисипи).
`error: row 12 term_years must be between 1 and 255`). يخرج السكربت بالكود `1`
Для этого используется файл `2`, созданный в формате CSV.

## 7. Защитное действие

- Введите `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` в формате CSV.
  Создайте NDJSON, подключите CLI и Torii.
- Убийца Питера Уоррена (Бондон Пьер Уотсон) в роли Дэниэла Уайта `python3`.
  Включите интерфейс командной строки для изменения настроек.

Заявление о проведении манифеста, организованного NDJSON, и председателя совета директоров стюардов
Для создания полезных нагрузок необходимо использовать Torii.