---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-default-lane-quickstart
название: Guia Rapida Del Lane Predeterminado (NX-5)
Sidebar_label: Предопределенный быстрый переход по полосе движения
описание: Настройка и проверка резервного варианта заранее определенной полосы Nexus для Torii и SDK можно опустить идентификатор полосы движения на общедоступных полосах.
---

:::обратите внимание на Фуэнте каноника
Эта страница отражает `docs/source/quickstart/default_lane.md`. Вам нужно будет скопировать информацию о том, что адрес локализации находится на портале.
:::

# Быстрый переход по заранее заданной полосе движения (NX-5)

> **Контекст дорожной карты:** NX-5 — предопределенная интеграция на общественную дорогу. Среда выполнения теперь отображает резервный `nexus.routing_policy.default_lane` для конечных точек REST/gRPC Torii и каждого SDK, который можно опустить с защитой `lane_id`, когда трафик переносится на каноническую публичную полосу. Это поможет операторам настроить каталог, проверить резервный вариант в `/status` и проверить совместимость клиента в экстремальных и экстремальных ситуациях.

## Предварительные требования

- Сборка Sora/Nexus от `irohad` (выпущена `irohad --sora --config ...`).
- Доступ к репозиторию конфигурации для редактирования разделов `nexus.*`.
- `iroha_cli` настроен для доступа к кластеру объектов.
- `curl`/`jq` (или эквивалент) для проверки полезной нагрузки `/status` от Torii.

## 1. Описать каталог полос и пространств данных.

Объявите полосы и пространства данных, которые должны существовать в красном цвете. Следующий фрагмент (запись `defaults/nexus/config.toml`) регистрирует три публичных полосы с псевдонимом корреспондента пространства данных:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Cada `index` должен быть единым и непрерывным. Идентификаторы пространства данных имеют размер 64 бита; los ejemplos anteriores usan los mismos valores numericos que los indexs de Lane para mayor claridad.

## 2. Настройка предопределенных значений enrutamiento и дополнительных настроек

Раздел `nexus.routing_policy` контролирует резервную полосу и позволяет записать инструкции по конкретным инструкциям или префиксам этой строки. Если какие-то правила совпадают, планировщик перейдет в транзакцию с настроенными `default_lane` и `default_dataspace`. Логика маршрутизатора работает в `crates/iroha_core/src/queue/router.rs` и применяется к прозрачной форме с использованием REST/gRPC Torii.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Когда вы присоединяетесь к новым полосам, актуализируйте первый каталог и расширяйте правила ознакомления. Резервная полоса должна быть тщательно проложена по публичной полосе, на которой сосредоточена часть трафика пользователей, чтобы старые SDK работали.

## 3. Арранка ип-нодо с политическим приложением

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Эль-нодо-регистра-ла-политика де-энрутамиенто производилась во время ожидания. Более частые ошибки проверки (недостоверные индексы, псевдонимы дубликатов, идентификаторы недействительных пространств данных) должны быть связаны с появлением сплетен.

## 4. Подтвердите статус губернатора переулка.В случае, если номер находится на линии, используйте помощник CLI для проверки того, что полоса заранее определена (заявленный груз) и список для трафика. La vista de резюме imprime una fila por road:

```bash
iroha_cli app nexus lane-report --summary
```

Пример вывода:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Если полоса заранее определена по номеру `sealed`, она должна быть указана в справочнике управления полосами до разрешения внешнего трафика. Флаг `--fail-on-sealed` используется для CI.

## 5. Проверка полезной нагрузки на территории Torii

Ответ `/status` разъясняет политическую политику как мгновенный планировщик на полосе движения. Используйте `curl`/`jq`, чтобы подтвердить значения заранее определенных конфигураций и проверить, что резервная линия обеспечивает телеметрию:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Пример вывода:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Для проверки контадоров в реальном времени планировщика на полосе `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Это подтверждение того, что мгновенное значение TEU, метаданные псевдонима и флаги манифеста являются строками конфигурации. Полезная нагрузка неправильного типа — это те, которые используются панелями Grafana для панели управления полосой движения.

## 6. Предоставление предопределенных ценностей клиенту

- **Rust/CLI.** `iroha_cli` и клиентский ящик Rust опускают поле `lane_id`, когда нет ошибок `--lane-id` / `LaneSelector`. Маршрутизатор кола снова повторяет `default_lane`. Используйте явные флаги `--lane-id`/`--dataspace-id` в одиночку, когда вы выбираете заранее не определенную полосу.
- **JS/Swift/Android.** Последние версии SDK содержат `laneId`/`lane_id` как дополнительные и имеют резервный вариант, объявленный для `/status`. Синхронизируйте политику внедрения между постановкой и производством для мобильных приложений, не требуя изменения конфигурации в чрезвычайных ситуациях.
- **Тестирование конвейера/SSE.** Фильтры событий транзакций принимаются согласно прогнозам `tx_lane_id == <u32>` (версия `docs/source/pipeline.md`). Подпишитесь на `/v2/pipeline/events/transactions` с этим фильтром, чтобы продемонстрировать, что записные листы отправляются без явного указания полосы движения, а также идентификатор резервной полосы.

## 7. Observabilidad y ganchos de gobernanza

- `/status` также является общедоступным `nexus_lane_governance_sealed_total` и `nexus_lane_governance_sealed_aliases`, чтобы Alertmanager мог сообщить, когда полоса проложена в манифесте. Manten esas alertas habilitadas incluso en devnets.
- Карта телеметрии планировщика и панель управления полосами (`dashboards/grafana/nexus_lanes.json`) отображаются в псевдонимах/ссылках каталога. Если вы назовете псевдоним, введите этикетку каталогов корреспондентов Kura, чтобы аудиторы поддерживали определенные руты (Seguido bajo NX-1).
- Парламентские апробации по заранее определенным направлениям должны включать план отката. Зарегистрируйте хеш-файл манифеста и свидетельство губернатора с помощью быстрого запуска в вашем рабочем журнале, чтобы не было вращений будущего, которые необходимы для достижения требуемого состояния.Все, что связано с этими сложностями, может быть использовано `nexus.routing_policy.default_lane` как функция проверки подлинности для настройки SDK и дестабилизации рутов кода, находящихся в одном ряду в красном цвете.