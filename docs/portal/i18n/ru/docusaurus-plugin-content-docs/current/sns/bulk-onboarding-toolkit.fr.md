---
lang: ru
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Источник канонический
На этой странице отображается `docs/source/sns/bulk_onboarding_toolkit.md`, где находятся эти файлы.
Externes voient la meme наведение SN-3b без клонирования депо.
:::

# Инструментарий для адаптации массива СНС (СН-3б)

**Справочный план действий:** SN-3b «Инструменты для массового внедрения».  
**Артефакты:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Большие регистраторы готовятся к событию по сотням регистраций `.sora` или
`.nexus` с мемами об одобрении управления и правилах урегулирования.
Фабрика полезных данных JSON в основном или в CLI без масштабирования, с использованием SN-3b
открыть строительный файл CSV с версией Norito, который готовит структуры
`RegisterNameRequestV1` для Torii или CLI. L'helper valide chaque ligne en
amont, emet a la fois un manifee agrege et du JSON delimite par nouvelles
дополнительные опции и возможно автоматизация полезной нагрузки
регистратор структур повторной проверки для проведения проверок.

## 1. Схема CSV

Le parseur exige la ligne d'en-tete suivante (l'ordre est гибкий):

| Колонн | Реквизит | Описание |
|---------|--------|-------------|
| `label` | Да | Libelle requiree (касса смешанного приема; l'outil нормализовать selon Norm v1 и UTS-46). |
| `suffix_id` | Да | Идентификатор суффикса (десятичный или шестнадцатеричный `0x`). |
| `owner` | Да | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Да | Энтьер `1..=255`. |
| `payment_asset_id` | Да | Акт урегулирования (пример `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Да | Entiers не является представителем объединения местных жителей в действии. |
| `settlement_tx` | Да | Значение JSON или цепочка слов, определяющая платежную транзакцию или хэш. |
| `payment_payer` | Да | AccountId для авторизации платежа. |
| `payment_signature` | Да | JSON или цепочка букв, содержащая преуве-де-подпись стюарда или казначейства. |
| `controllers` | Опция | Список отдельных адресов или адресов счетчика контроллера. По умолчанию `[owner]` отсутствует. |
| `metadata` | Опция | Встроенный JSON или `@path/to/file.json` для подсказок по разрешению, регистрации TXT и т. д. По умолчанию `{}`. |
| `governance` | Опция | Встроенный JSON или `@path` указывает на версию `GovernanceHookV1`. `--require-governance` наложить эту колонку. |

Вся колонка может указывать на внешнюю ссылку и префикс значения ячейки
пар `@`. Относительные значения в формате CSV.

## 2. Палач-помощник

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Опции:

- `--require-governance` отключить линии без крючка управления (утилита для
  les encheres premium или les affactions Reservees).
- `--default-controllers {owner,none}` определяет типы контроллеров ячеек.
  retombent sur le compte владелец.
- `--controllers-column`, `--metadata-column` и `--governance-column` постоянно.
  де переименовать колонки опций для экспорта amont.

В случае успешного написания сценария в общем манифесте:

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
```Si `--ndjson` - это Fourni, Chaque `RegisterNameRequestV1` - австралийский сертификат, как
Документ JSON по прямой линии, по которой можно использовать стримерные файлы автоматизации
запрашивает направление версии Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Автоматизированные комиссии

### 3.1 Режим Torii REST

Укажите `--submit-torii-url` плюс `--submit-token` или `--submit-token-file` для заливки.
Вы можете ввести вход в манифест версии Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Помощник по запросу `POST /v1/sns/names` по запросу и по прибытии в премьер-министр
  ошибка HTTP. Ответы присылаются в журнале регистрации NDJSON.
- `--poll-status` повторный опрос `/v1/sns/names/{namespace}/{literal}` после перерыва
  soumission (jusqu'a `--poll-attempts`, по умолчанию 5) для подтверждения того, что
  регистрация est видна. Фурнисс `--suffix-map` (JSON de `suffix_id`
  vers des valeurs «суффикс») для того, чтобы получить мусор
  `{label}.{suffix}` для опроса.
- Регулируемые: `--submit-timeout`, `--poll-attempts` и `--poll-interval`.

### 3.2 Режим интерфейса командной строки iroha

Pour faire passer chaque entree du Manife par la CLI, Fournissez le Chemin du
бинар:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Контроллеры делают входы `Account` (`controller_type.kind = "Account"`)
  car la CLI раскрывает уникальность баз контроллеров на уровне конкурентов.
- Метаданные и управление Blobs являются документами в временных документах по номинальной стоимости.
  запросите и передайте `iroha sns register --metadata-json ... --governance-json ...`.
- Стандартный вывод и стандартный вывод CLI для кодов вылетов в журналах;
  ненулевые коды прерывают выполнение.

Два режима душа могут работать в ансамбле (Torii и CLI) для
выполните развертывание регистратора или повторите резервные варианты.

### 3.3 Итоги голосования

Если `--submission-log <path>` есть, сценарий входа в систему NDJSON
захватчик:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Les reponses Torii reussies incluent des Champs Structures Extraits de
`NameRecordV1` или `RegisterNameResponseV1` (например, `record_status`,
И18НИ00000079Х, И18НИ00000080Х, И18НИ00000081Х,
`registry_event_version`, `suffix_id`, `label`) afin que les Dashboards et les
Отчеты об управлении могут анализировать журналы без инспектора свободного текста.
Joignez ce log aux регистратор билетов с манифестом для доказательств
воспроизводимый.

## 4. Автоматизация выпуска порта

Les jobs CI et portail appellent `docs/portal/scripts/sns_bulk_release.sh`, который
инкапсуляция помощников и сохранение артефактов
`artifacts/sns/releases/<timestamp>/`:

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

Сценарий:1. Создайте `registrations.manifest.json`, `registrations.ndjson` и скопируйте файл.
   Оригинальный CSV в репертуаре выпуска.
2. Создайте манифест через Torii и через CLI (Quand Configuration) и запишите его.
   `submissions.log` с повторяющимися структурами ci-dessus.
3. Emet `summary.json` декривант релиза (chemins, URL Torii, chemin CLI,
   временная метка) afin que l'automatization du portail puisse uploader le Bundle vers
   склад артефактов.
4. Produit `metrics.prom` (переопределение через `--metrics`) contenant des compteurs
   в формате Prometheus для общего количества запросов, распределения суффиксов,
   les totaux d'asset и les resultats de soumission. Le JSON возобновить пуанты
   это документ.

Архив рабочих процессов упрощает репертуар выпуска как отдельный артефакт,
qui contient desormais tout ce dont la gouvernance a besoin pour l'audit.

## 5. Телеметрия и информационные панели

Le fichier de meteriques Genere par `sns_bulk_release.sh` раскрывает серии
суивантес:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Injectez `metrics.prom` в вашей коляске Prometheus (например, через Promtail или
импортная партия) для регистраторов, стюардов и пар управления по
массовое продвижение. Таблица Grafana
`dashboards/grafana/sns_bulk_release.json` визуализируйте мемы, сделанные с помощью
panneaux pour les comptes par suffixe, le Volume de Paiement et les Ratios de
повторный сайт/выпуск сумиссий. Табличный фильтр по `release` для этих файлов
аудиторы могут сконцентрироваться на отдельном исполнении CSV.

## 6. Проверка и режимы проверки

- **Нормализация меток:** элементы нормализуются с Python IDNA plus.
  строчные буквы и фильтры символов Norm v1. Недействительные ярлыки echouent vite
  avant tout appel reseau.
– **Понятные цифры:** идентификаторы суффиксов, годы семестра и подсказки по ценам.
  rester dans lesbornes `u16` и `u8`. Платные поля, принятые
  введите десятичное или шестнадцатеричное значение `i64::MAX`.
- **Разбор метаданных или управление:** встроенный JSON — это направление анализа; лес
  ссылки на файлы, которые разрешают соотнесение с размещением CSV.
  Необъективные метаданные приводят к ошибке проверки.
- **Контроллеры:** ячейки имеют соответствующий `--default-controllers`. Фурниссе
  явные списки (пример `<i105-account-id>;<i105-account-id>`), когда вы делегируете их
  актеры не являются владельцами.

Сигнальные сигналы с числом строк контекста (например,
`error: row 12 term_years must be between 1 and 255`). Сортировка сценария с файлом
код `1` при ошибках проверки и `2`, указывающий CSV-файл.

## 7. Тесты и происхождение

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` может анализировать CSV,
  эмиссия NDJSON, управление соблюдением требований и параметры вызова CLI или Torii.
- L'helper est du Python Pur (дополнительная зависимость) и часть турне
  или `python3` доступен. L'historique des commits est suivi aux cotes de la
  CLI в главном хранилище для воспроизводства.Для выполнения производственных операций воспользуйтесь общим манифестом и пакетом NDJSON au.
Билет регистратора, который может предоставить стюардам точные данные о полезной нагрузке
сум — Torii.