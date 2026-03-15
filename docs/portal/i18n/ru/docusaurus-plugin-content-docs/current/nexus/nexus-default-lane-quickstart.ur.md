---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-default-lane-quickstart
Название: полоса по умолчанию کوئیک اسٹارٹ (NX-5)
Sidebar_label: полоса по умолчанию
описание: Nexus — резервная полоса по умолчанию, настройка и проверка, Torii — общедоступные полосы SDK, полоса_id опустить
---

:::обратите внимание на канонический источник
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ Если вам нужна локализация, выровняйте настройки и выровняйте их.
:::

# полоса по умолчанию کوئیک اسٹارٹ (NX-5)

> **Контекст дорожной карты:** NX-5 — интеграция с полосами общего пользования по умолчанию. резервная среда выполнения `nexus.routing_policy.default_lane`, резервная версия Torii Конечные точки REST/gRPC, поддержка SDK и `lane_id` محفوظ طریقے سے опустить کر سکیں جب ٹریفک каноническая общественная полоса سے تعلق رکھتا ہو۔ Операторы каталога Настройка `/status` Резервная проверка Проверка сквозного поведения клиента کرتی ہے۔

## Предварительные условия

- `irohad` — сборка Sora/Nexus (`irohad --sora --config ...` — сборка).
- хранилище конфигурации تک رسائی تاکہ `nexus.*` разделы редактировать کیے جا سکیں۔
- `iroha_cli` в целевом кластере, который может быть настроен.
- Torii `/status` Проверка полезной нагрузки или проверка `curl`/`jq` (эквивалент).

## 1. полоса доступа к каталогу пространства данных

сеть, доступ к полосам движения и пространствам данных, которые можно объявить Ниже приведен фрагмент (`defaults/nexus/config.toml` سے) для линий общего пользования и сопоставления псевдонимов пространства данных.

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

ہر `index` منفرد اور ہونا چاہیے۔ Идентификаторы пространства данных, 64-битные значения. Здесь можно найти индексы дорожек и числовые значения.

## 2. Маршрутизация по умолчанию и дополнительные переопределения.

`nexus.routing_policy` Резервная полоса управления Управление инструкциями Префиксы учетных записей Переопределение маршрутизации Переопределение маршрутизации Найдите соответствие правил и планировщик, чтобы настроить `default_lane` и `default_dataspace`, выберите маршрут. Логика маршрутизатора `crates/iroha_core/src/queue/router.rs` Не требуется Torii Поверхности REST/gRPC

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


## 3. Как настроить загрузку узла

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

запуск узла کے دوران производная политика маршрутизации لاگ کرتا ہے۔ Ошибки проверки (отсутствующие индексы, дублированные псевдонимы, неверные идентификаторы пространства данных)

## 4. Состояние управления полосой движения کنفرم کریں

узел в сети доступен вспомогательный интерфейс командной строки настройка полосы по умолчанию запечатана (загружен манифест) нет трафика готов ہو۔ Сводный вид:

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

Полоса по умолчанию `sealed` Разрешение внешнего трафика Разрешить использование полосы управления Runbook `--fail-on-sealed` флаг CI کے لئے مفید ہے۔

## 5. Torii проверяет полезные данные состоянияПолитика маршрутизации ответов `/status`. Снимок планировщика полос. `curl`/`jq` Как настроить настройки по умолчанию, как настроить резервную полосу телеметрии رہا ہے:

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

Lane `0` کے لئے счетчики живого планировщика, например:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Используйте моментальный снимок TEU, метаданные псевдонимов, конфигурацию флагов манифеста и выравнивание. Панели полезной нагрузки Grafana и панель управления полосой движения.

## 6. Упражнение по умолчанию для клиента.

- **Rust/CLI.** `iroha_cli` в крейте клиента Rust `lane_id` поле можно опустить или пропустить `--lane-id` / `LaneSelector` pass نہیں کرتے۔ Маршрутизатор очереди `default_lane` является резервным вариантом. Явные флаги `--lane-id`/`--dataspace-id`.
- **JS/Swift/Android.** В SDK выпускаются `laneId`/`lane_id`, а также дополнительные версии `/status` для `/status`. Значение резервного варианта کرتے ہیں۔ Политика маршрутизации, промежуточный этап, производственная синхронизация, поддержка мобильных приложений, экстренная реконфигурация и многое другое.
- **Тесты конвейера/SSE.** Фильтры событий транзакций `tx_lane_id == <u32>` предикаты قبول کرتے ہیں (دیکھیں `docs/source/pipeline.md`). `/v1/pipeline/events/transactions` Фильтр или подписка Если вы хотите использовать явную полосу, вы можете записать резервную полосу id کے تحت پہنچتی ہیں۔

## 7. Наблюдаемость и крючки управления

- `/status` `nexus_lane_governance_sealed_total` или `nexus_lane_governance_sealed_aliases` Опубликовать сообщение в диспетчере оповещений Предупреждать о необходимости публикации манифеста полосы оповещений کھو دے۔ Если оповещения включены, devnets включен или отключен.
- карта телеметрии планировщика, панель управления полосами движения (`dashboards/grafana/nexus_lanes.json`), каталог, поля псевдонимов/слагов, которые ожидаются, и другие. Переименование псевдонима или переименование каталогов Kura, перемаркировка, детерминированные пути аудиторов, выбор (NX-1, дорожка) ہوتا ہے)۔
- полосы по умолчанию کے لئے парламентские одобрения میں план отката شامل ہونا چاہیے۔ хэш манифеста, доказательства управления, быстрый старт, руководство для оператора, Runbook, запись, будущие ротации, состояние, состояние. لگائیں۔