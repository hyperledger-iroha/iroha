---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 569f46d041fd052b32f250bc513d5dd50ee1e38eaff24d4875c075f47209d06c
source_last_modified: "2025-11-10T19:43:33.013605+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-default-lane-quickstart
title: Быстрый старт default lane (NX-5)
sidebar_label: Быстрый старт default lane
description: Настройте и проверьте fallback default lane в Nexus, чтобы Torii и SDK могли опускать lane_id в public lanes.
---

:::note Канонический источник
Эта страница отражает `docs/source/quickstart/default_lane.md`. Держите обе копии синхронизированными, пока локализационный прогон не попадет в портал.
:::

# Быстрый старт default lane (NX-5)

> **Контекст roadmap:** NX-5 - интеграция default public lane. Рантайм теперь предоставляет fallback `nexus.routing_policy.default_lane`, чтобы REST/gRPC эндпоинты Torii и каждый SDK могли безопасно опускать `lane_id`, когда трафик относится к канонической public lane. Это руководство проводит операторов через настройку каталога, проверку fallback в `/status` и проверку поведения клиента от начала до конца.

## Предварительные требования

- Сборка Sora/Nexus для `irohad` (запуск `irohad --sora --config ...`).
- Доступ к репозиторию конфигураций, чтобы редактировать секции `nexus.*`.
- `iroha_cli`, настроенный на целевой кластер.
- `curl`/`jq` (или эквивалент) для просмотра payload `/status` в Torii.

## 1. Описать каталог lane и dataspace

Определите lanes и dataspaces, которые должны существовать в сети. Фрагмент ниже (вырезан из `defaults/nexus/config.toml`) регистрирует три public lane и соответствующие alias для dataspace:

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

Каждый `index` должен быть уникальным и непрерывным. Id dataspaces - это 64-битные значения; в примерах выше используются те же числовые значения, что и индексы lane, для наглядности.

## 2. Задать дефолты маршрутизации и опциональные переопределения

Секция `nexus.routing_policy` управляет fallback lane и позволяет переопределять маршрутизацию для конкретных инструкций или префиксов аккаунтов. Если ни одно правило не подходит, scheduler направляет транзакцию в `default_lane` и `default_dataspace`. Логика router находится в `crates/iroha_core/src/queue/router.rs` и прозрачно применяет политику к поверхностям Torii REST/gRPC.

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


## 3. Запустить ноду с примененной политикой

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Нода логирует вычисленную политику маршрутизации при старте. Любые ошибки валидации (отсутствующие индексы, дублирующиеся alias, некорректные ids dataspaces) всплывают до начала gossip.

## 4. Подтвердить состояние governance для lane

После того как нода онлайн, используйте CLI helper, чтобы убедиться, что default lane запечатана (manifest загружен) и готова к трафику. Сводный вид выводит по одной строке на lane:

```bash
iroha_cli app nexus lane-report --summary
```

Example output:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Если default lane показывает `sealed`, следуйте runbook по governance для lanes перед тем, как разрешить внешний трафик. Флаг `--fail-on-sealed` удобен для CI.

## 5. Проверить status payload Torii

Ответ `/status` раскрывает и политику маршрутизации, и снимок scheduler по lanes. Используйте `curl`/`jq`, чтобы подтвердить настроенные значения по умолчанию и проверить, что fallback lane публикует телеметрию:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Sample output:

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

Это подтверждает, что TEU snapshot, метаданные alias и флаги manifest соответствуют конфигурации. Тот же payload используется панелями Grafana для dashboard lane-ingest.

## 6. Проверить дефолтное поведение клиента

- **Rust/CLI.** `iroha_cli` и клиентский crate Rust опускают поле `lane_id`, когда вы не передаете `--lane-id` / `LaneSelector`. Queue router в этом случае падает в `default_lane`. Используйте явные флаги `--lane-id`/`--dataspace-id` только при работе с не-default lane.
- **JS/Swift/Android.** Последние релизы SDK считают `laneId`/`lane_id` опциональными и fallback-ят на значение, объявленное в `/status`. Держите политику маршрутизации синхронизированной между staging и production, чтобы мобильным приложениям не требовались аварийные перенастройки.
- **Pipeline/SSE tests.** Фильтры событий транзакций принимают предикаты `tx_lane_id == <u32>` (см. `docs/source/pipeline.md`). Подпишитесь на `/v1/pipeline/events/transactions` с этим фильтром, чтобы доказать, что записи, отправленные без явного lane, приходят под fallback lane id.

## 7. Observability и governance hooks

- `/status` также публикует `nexus_lane_governance_sealed_total` и `nexus_lane_governance_sealed_aliases`, чтобы Alertmanager мог предупреждать, когда lane теряет manifest. Держите эти алерты включенными даже на devnet.
- Карта телеметрии scheduler и dashboard governance для lanes (`dashboards/grafana/nexus_lanes.json`) ожидают поля alias/slug из каталога. Если вы переименовываете alias, переименуйте и соответствующие директории Kura, чтобы аудиторы сохраняли детерминированные пути (отслеживается по NX-1).
- Парламентские одобрения для default lanes должны включать план rollback. Зафиксируйте hash manifest и доказательства governance рядом с этим quickstart в вашем операторском runbook, чтобы будущие ротации не гадали требуемое состояние.

Когда эти проверки пройдены, можно считать `nexus.routing_policy.default_lane` источником истины для конфигурации SDK и начинать отключать унаследованные single-lane кодовые пути в сети.
