---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-default-lane-quickstart
Название: Быстрый старт по умолчанию (NX-5)
Sidebar_label: Полоса быстрого старта по умолчанию
описание: Настройте и проверьте резервную полосу по умолчанию в Nexus, чтобы Torii и SDK могли опускать line_id на общедоступных полосах.
---

:::note Канонический источник
На этой странице отражено `docs/source/quickstart/default_lane.md`. Держите копии синхронизированными, пока локальный прогон не попадет на портал.
:::

# Полоса быстрого старта по умолчанию (NX-5)

> **Дорожная карта контекста:** NX-5 — общедоступная полоса интеграции по умолчанию. Рантайм теперь обеспечивает резервный вариант `nexus.routing_policy.default_lane`, чтобы REST/gRPC эндпоинты Torii и каждый SDK могли безопасно опускать `lane_id`, когда трафик соответствует стандартной полосе общего пользования. Это руководство осуществляется операторами через каталог каталога, резервную проверку в `/status` и проверку поведения клиента с начала до конца.

## Предварительные требования

- Сборка Sora/Nexus для `irohad` (запуск `irohad --sora --config ...`).
- Доступ к репозиторию конфигураций для редактирования раздела `nexus.*`.
- `iroha_cli`, настроенный на открытый кластер.
- `curl`/`jq` (или эквивалент) для просмотра полезной нагрузки `/status` в Torii.

## 1. Описать каталог Lane и Dataspace

Определите каналы и пространства данных, которые должны существовать в сети. Фрагмент ниже (вырезанный из `defaults/nexus/config.toml`) регистрирует три общедоступных канала и соответствует псевдониму для пространства данных:

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

Каждый `index` должен быть прогрессивным и непрерывным. Id dataspaces — это 64-битные значения; в примерах, использованных выше, те же значения чисел, что и индексы, для наглядности.

## 2. Задать дефолты маршрутизации и опциональные переопределения

Секция `nexus.routing_policy` резервная полоса управления и позволяет переопределять маршрутизацию для конкретных инструкций или префиксов аккаунтов. Если ни одно правило не подходит, планировщик направляет транзакцию в `default_lane` и `default_dataspace`. Логика маршрутизатора находится в `crates/iroha_core/src/queue/router.rs`, и прозрачность применяется к поверхностям Torii REST/gRPC.

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


## 3. Запустить ноду с прикладной политикой

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Нода логирует вычисленную политику маршрутизации при старте. Любые ошибки валидации (отсутствующие индексы, дублирующиеся псевдонимы, некорректированные идентификаторы пространств данных) всплывают до начала сплетен.

## 4. Подтвердить состояние управления полосой движения

После того как нода онлайн воспользуйтесь помощником CLI, чтобы убедиться, что полоса по умолчанию запечатана (манифест загружен) и готова к трафику. Сводный вид выводит по одной строке на полосу:

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

Если полоса по умолчанию показывает `sealed`, следуйте runbook по управлению полосами перед темой, как получить внешний трафик. Флаг `--fail-on-sealed` удобен для CI.

## 5. Проверьте полезную нагрузку статуса ToriiОтвет `/status` раскрывает и политику маршрутизации, и снимок scheduler по lanes. Используйте `curl`/`jq`, чтобы проверить значения настроений по умолчанию и проверить, что резервная полоса публикует телеметрию:

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

Чтобы посмотреть живые счетчики scheduler для lane `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Это подтверждение, что моментальный снимок TEU, метаданный псевдоним и флаги-манифест соответствуют конфигурации. Тот же payload используется панелями Grafana для dashboard lane-ingest.

## 6. Проверить дефолтное поведение клиента

- **Rust/CLI.** `iroha_cli` и клиентский ящик Rust опускают поле `lane_id`, когда вы не передаете `--lane-id` / `LaneSelector`. Queue router в этом случае падает в `default_lane`. Используйте явные флаги `--lane-id`/`--dataspace-id` только при работе с не-default lane.
- **JS/Swift/Android.** Последние релизы SDK считаются `laneId`/`lane_id` опциональными и резервными вариантами, объявленными в `/status`. Держите политику синхронизации между постановкой и производством, чтобы мобильным приложениям не требовались неожиданные перенастройки.
- **Тесты Pipeline/SSE.** Фильтры событий транзакций принимают предикаты `tx_lane_id == <u32>` (см. `docs/source/pipeline.md`). Подпишитесь на `/v2/pipeline/events/transactions` с этим фильтром, чтобы подтвердить, что записи, отправленные без явного переулка, приходят под резервным идентификатором переулка.

## 7. Хуки наблюдаемости и управления

- `/status` также публикует `nexus_lane_governance_sealed_total` и `nexus_lane_governance_sealed_aliases`, чтобы Alertmanager мог предупреждать, когда полоса движения манифестируется. Держите эти алерты включенными даже на devnet.
- Планировщик графиков телеметрии и панель управления для полос (`dashboards/grafana/nexus_lanes.json`) определяют псевдоним/слаг поля из каталога. Если вы переименовываете псевдоним, переименуйте и соответствующий каталог Kura, чтобы аудиторы сохраняли определенный путь (отслеживается по NX-1).
- Парламентские одобрения для default lanes должны включать план rollback. Зафиксируйте хеш-манифест и докажите управление рядом с этим быстрым запуском в вашем операторском справочнике, чтобы будущие ротации не гадали требуемое состояние.

Когда эти проверки пройдены, можно считать `nexus.routing_policy.default_lane`, что стало основанием для конфигурации SDK, и начать отключать унаследованные однополосные кодовые пути в сети.