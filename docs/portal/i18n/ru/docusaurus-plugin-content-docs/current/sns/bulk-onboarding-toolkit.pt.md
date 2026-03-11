---
lang: ru
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание Fonte canonica
Espelha `docs/source/sns/bulk_onboarding_toolkit.md` для внешних операций
Воспользуйтесь ориентацией SN-3b в своем клоне или репозитории.
:::

# Инструментарий для массовой адаптации в социальных сетях (SN-3b)

**Справочная информация по плану действий:** SN-3b «Инструменты для массового внедрения».  
**Артефатос:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Большая частая подготовка регистраторов `.sora` или
`.nexus` в качестве сообщений об одобрении управления и путях урегулирования. Монтар
полезные данные JSON вручную или повторно выполнить CLI для увеличения, включая SN-3b между собой
Определенный конструктор CSV для Norito, который готовит проекты
`RegisterNameRequestV1` для Torii или для CLI. О помощник, валида када линья
Как раньше, создайте единый манифест в формате JSON, разделенный порами
если дополнительная линия не требуется, и можно автоматически отправить полезную нагрузку
регистрация рецептов estruturados для аудиторий.

## 1. Эскема CSV

Синтаксический анализатор выполнит следующую строку кабекальо (порядок и гибкий порядок):

| Колуна | Обригаторио | Описание |
|--------|-------------|-----------|
| `label` | Сим | Запрос на этикетку (смешанный случай aceita; нормализация нормализации, соответствующая нормам v1 и UTS-46). |
| `suffix_id` | Сим | Суфиксный цифровой идентификатор (десятичный или шестнадцатеричный `0x`). |
| `owner` | Сим | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Сим | Интейро `1..=255`. |
| `payment_asset_id` | Сим | Атива урегулирования (например, `xor#sora`). |
| `payment_gross` / `payment_net` | Сим | Inteiros sem sinal представляют собой unidades nativas do ativo. |
| `settlement_tx` | Сим | Используйте JSON или строковый литерал, расшифрованный для транзакции или хэша. |
| `payment_payer` | Сим | AccountId, который авторизуется или публикуется. |
| `payment_signature` | Сим | JSON или строковый буквальное утверждение, подтверждающее убийство стюарда или тесурарии. |
| `controllers` | Необязательно | Список разделяется на мосты и виртуальные или виртуальные конечные контроллеры. Padrao `[owner]`, когда опускается. |
| `metadata` | Необязательно | Встроенный JSON или `@path/to/file.json` для подсказок по распознавателю, регистрам TXT и т. д. Padrao `{}`. |
| `governance` | Необязательно | Встроенный JSON или `@path` для `GovernanceHookV1`. `--require-governance` exige esta coluna. |

Какой столбец может ссылаться на внешний префикс архива или значение ячейки с `@`.
Наши камины решены и переданы в архив CSV.

## 2. Исполнитель или помощник

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Основные операции:

- `--require-governance` Rejeita Linhas Sem um крючок управления (используется для
  leiloes premium или atribuicoes reservadas).
- `--default-controllers {owner,none}` определяет выбор ячеек контроллеров
  Вольтам для владельца контакта.
- `--controllers-column`, `--metadata-column`, и `--governance-column` разрешение
  переименовывайте варианты выбора или trabalhar com, экспортируя добычу и добычу.

В случае успеха или сценария, состоящего из сводного манифеста:

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
```Se `--ndjson` для fornecido, cada `RegisterNameRequestV1` тамбем и escrito como
уникальный документ JSON de linha для автоматической передачи запросов
прямо на Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. Автоматизация отправки сообщений

### 3.1 Мод Torii REST

Специальные `--submit-torii-url` чаще `--submit-token` или `--submit-token-file`
Чтобы отправить манифест напрямую в Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- О помощник, эмите um `POST /v1/sns/registrations` по запросу и прерыванию не в первую очередь
  ошибка HTTP. В качестве ответов на запросы или журналы регистрации NDJSON.
- `--poll-status` проконсультируйтесь с `/v1/sns/registrations/{selector}` после каждой отправки
  (ate `--poll-attempts`, по умолчанию 5) для подтверждения того, что регистрация является видимой.
  Forneca `--suffix-map` (JSON de `suffix_id` для значений «суффикс»), чтобы
  Ferramenta извлекает буквенные `{label}.{suffix}` для опроса.
- Регулирует: `--submit-timeout`, `--poll-attempts`, e `--poll-interval`.

### 3.2 Режим командной строки ироха

Чтобы повернуть каждый вход в манифест CLI, откройте или откройте бинарный файл:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Контроллеры разработаны с использованием `Account` (`controller_type.kind = "Account"`)
  Сначала нужно использовать CLI, чтобы отображать контроллеры на основе содержимого.
- Большие объемы метаданных и управление важными временными архивами по запросу.
  и вставлены в пункт `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout и stderr da CLI, больше кодов, чем зарегистрированные; кодигос нао
  ноль абортов и экзекуао.

Используйте моды отправки подключаемых устройств (Torii и CLI) для проверки
развертывания выполняют регистратор или резервные варианты.

### 3.3 Ответы на подчинение

Когда `--submission-log <path>` и fornecido, или скрипт, входящий в NDJSON, который
каптурам:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Ответы Torii bem-sucedidas incluem Campos estruturados extraidos de
`NameRecordV1` или `RegisterNameResponseV1` (например, `record_status`,
И18НИ00000079Х, И18НИ00000080Х, И18НИ00000081Х,
`registry_event_version`, `suffix_id`, `label`) для панелей мониторинга и связей
degovanca possam parsear или log sem inspectionar texto livre. Приложение этого журнала
в качестве билетов регистратора или манифеста для воспроизводства доказательств.

## 4. Автоматический выпуск портала

Вакансии CI и портала `docs/portal/scripts/sns_bulk_release.sh`, которые
инкапсула о помощнике и армазена артефатос рыдание
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

О скрипт:

1. Создание `registrations.manifest.json`, `registrations.ndjson`, электронная копия или CSV.
   оригинал для директории выпуска.
2. Подметка манифеста с использованием Torii через CLI (когда настроено), добавлено
   `submissions.log` с полученными данными.
3. Создайте `summary.json`, укажите или выпустите (caminhos, URL-адрес Torii, Caminho da
   CLI, метка времени) для автоматического создания портала, который может быть отправлен или связан с пакетом для
   хранилище артефактов.
4. Produz `metrics.prom` (переопределение через `--metrics`) contendo contadores compativeis
   com Prometheus для общего количества запросов, распределения суфиксов, общего количества активов
   и результаты подчинения. O JSON резюме для этого архива.Простые рабочие процессы в архиве или каталоге выпуска как единый арт,
que agora contem tudo или que agovanaca precisa para Audiia.

## 5. Телеметрия и информационные панели

Архив метрик, полученный от `sns_bulk_release.sh`, выставляется в виде следующих серий:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Подача `metrics.prom` без коляски Prometheus (например, через Promtail или
гм, импортная партия) для регистраторов, стюардов и паритетов управления
alinhados sobre или прогресс в массе. О квадро Grafana
`dashboards/grafana/sns_bulk_release.json` Визуализация сообщений с болью
для заражения суфиксом, объемом страниц и соотношением успеха/falha de
сабмисо. O Quadro filtra por `release`, чтобы аудиторы могли видеть вас
уникальное исполнение CSV.

## 6. Проверка и модификация ошибок

- **Канонизация метки:** вводится как нормализованная с помощью Python IDNA больше всего
  строчные буквы и фильтры символов Norm v1. Этикетки недействительны.
  перед тем, как сделать это.
- **Guardrails numericos:** идентификаторы суффиксов, годы семестра и подсказки по ценам devem ficar
  в пределах `u16` и `u8`. Campos de pagamento aceitam inteiros decimais
  или шестигранник съел `i64::MAX`.
- **Разбор метаданных или управление:** Встроенный и прямой анализ JSON;
  ссылки в архивах, соответствующие разрешению и локализации в формате CSV. Метаданные
  нет объекта производства с ошибкой валидации.
- **Контроллеры:** указаны в официальном порядке `--default-controllers`. Форнека
  Явные списки (например, `i105...;i105...`) делегируются владельцам.

Сообщите о нескольких контекстных сообщениях (например,
`error: row 12 term_years must be between 1 and 255`). O script sai com codigo
`1` при ошибках валидации и `2`, когда файл CSV устарел.

## 7. Яички и процесс

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` анализирует CSV,
  эмисса NDJSON, принудительное управление и управление отправкой через CLI или Torii.
- О помощник и Python Puro (sem Dependencias Adicionais) и роды, которые можно получить
  Onde `python3` был отключен. O исторические совершения и растредо-юнто
  CLI не имеет основного хранилища для воспроизводства.

Для запуска производства приложение или созданный манифест и пакет NDJSON к билету
регистратор для того, чтобы стюарды могли воспроизвести полезную нагрузку exatos que foram
субметидос или Torii.