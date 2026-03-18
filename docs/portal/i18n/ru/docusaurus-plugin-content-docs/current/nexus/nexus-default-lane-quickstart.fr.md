---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-default-lane-quickstart
Название: Guide Rapide de la Lane по умолчанию (NX-5)
Sidebar_label: Руководство по быстрой полосе движения по умолчанию
описание: Конфигуратор и проверка резервного варианта полосы по умолчанию Nexus до того, как Torii, и SDK может использовать идентификатор полосы движения на общественных полосах.
---

:::note Источник канонический
Представление этой страницы `docs/source/quickstart/default_lane.md`. Убедитесь, что две копии совпадают только с тем, что локализация прибывает на порт.
:::

# Руководство по быстрой полосе движения по умолчанию (NX-5)

> **Контекстная дорожная карта:** NX-5 — интеграция по умолчанию в общественную зону. Среда выполнения предоставляет резервный вариант `nexus.routing_policy.default_lane` для конечных точек REST/gRPC Torii, а SDK может обеспечить полную безопасность `lane_id` для передачи трафика по канонической публичной полосе. Это руководство сопровождает операторов для настройки каталога, проверки резервного варианта в `/status` и тестера взаимодействия клиента в режиме «bout en bout».

## Предварительное условие

- Сборка Sora/Nexus от `irohad` (lancer `irohad --sora --config ...`).
- Доступ к хранилищу конфигурации для модификатора разделов `nexus.*`.
- `iroha_cli` настройка для передачи данных по кабелю кластера.
- `curl`/`jq` (или эквивалент) для проверки полезной нагрузки `/status` от Torii.

## 1. Определить каталог дорожек и пространств данных

Объявите дорожки и пространства данных, которые существуют в резерве. L'extrait ci-dessous (шина `defaults/nexus/config.toml`) зарегистрируйте три публичных полосы, а также псевдонимы корреспондентов пространства данных:

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

Chaque `index` doit etre unique et contigu. Идентификаторы пространства данных со значениями 64 бита; примеры ci-dessus, использующие числовые значения мемов, которые являются индексом полосы движения для плюс де ясности.

## 2. Определите значения по умолчанию для маршрутизации и дополнительные сборы

Раздел `nexus.routing_policy` контролирует резервную полосу и разрешает дополнительную маршрутизацию для конкретных инструкций или префиксов счета. Если соответствующие правила соответствуют, планировщик настраивает маршрут транзакции для версий `default_lane` и `default_dataspace`. Логика маршрутизатора с `crates/iroha_core/src/queue/router.rs` и применение политики прозрачности на поверхностях REST/gRPC Torii.

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

Lorsque vous ajoutez plus Tard de Nouvelles Lane, Mettez d'Abord a Jour le Catalogue, puis etendez les Regles de Routage. Резервная полоса является продолжением указателя на общественную полосу, которая является основной для пользователя трафика, который продолжает функционировать SDK.

## 3. Удалить все с политической аппликацией

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Le noeud Journalise la Politique de Routage Demarrage Demarrage. Все ошибки проверки (пропущенные индексы, дубликаты псевдонимов, недействительные идентификаторы пространства данных) устраняются перед дебютом сплетен.

## 4. Подтверждение государственного управления де-ла-лейнЧтобы открыть линию, используйте вспомогательный интерфейс командной строки для проверки того, что полоса по умолчанию является выделенной (явной платой) и предварительной очисткой трафика. Краткое изложение представлено на одной линии:

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

Если полоса соответствует стандартному аффилу `sealed`, следуйте записям управления полосами перед авторизацией внешнего трафика. Флаг `--fail-on-sealed` используется для CI.

## 5. Проверка полезных данных по статусу Torii

Ответ `/status` раскрывает политику маршрутизации, которая заключается в мгновенном использовании планировщика по полосе. Используйте `curl`/`jq` для подтверждения значений по умолчанию в настройках и проверки того, что резервный продукт телеметрии:

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

Залейте инспектора счетчиков в реальном времени в планировщик по полосе `0` :

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Подтвердите, что мгновенный TEU, метадоны псевдонимов и указатели манифеста совпадают с конфигурацией. Полезная нагрузка мема используется для панели управления полосами движения Grafana.

## 6. Тестирование значений по умолчанию для клиентов

- **Rust/CLI.** `iroha_cli` и клиент Rust в контейнере с чемпионом `lane_id`, если вы не прошли `--lane-id` / `LaneSelector`. Маршрутизатор очереди снова находится на `default_lane`. Используйте явные флаги `--lane-id`/`--dataspace-id`, уникальные для вас, на велосипеде по полосе, не по умолчанию.
- **JS/Swift/Android.** Последние версии SDK содержат `laneId`/`lane_id` как опции и возвращаются к значению, указанному в `/status`. Следите за политикой синхронизации маршрутизации между постановкой и производством, чтобы мобильные приложения не нуждались в срочной реконфигурации.
- **Тестирование конвейера/SSE.** Фильтры событий транзакций, принимающие предикаты `tx_lane_id == <u32>` (voir `docs/source/pipeline.md`). Подпишитесь на `/v1/pipeline/events/transactions` с этим фильтром, чтобы убедиться, что письма посланников без явного указания прибыли, поэтому мы имеем резервную полосу.

## 7. Наблюдение и уточнения по управлению

- `/status` опубликуйте aussi `nexus_lane_governance_sealed_total` и `nexus_lane_governance_sealed_aliases` afin qu'Alertmanager puisse avertir lorsqu'une Lane Perd Son Manifest. Gardez ces alertes actives meme sur les devnets.
- Карта телеметрии планировщика и панель управления полосами движения (`dashboards/grafana/nexus_lanes.json`), обслуживающая псевдоним полей/ссылка каталога. Если вы повторно назовете псевдоним, повторно промаркируйте репертуары корреспондентов Kura, чтобы одиторы сохраняли детерминированность (suivi sous NX-1).
- Парламентские одобрения в отношении полос движения по умолчанию включают план отката. Зарегистрируйте хэш манифеста и правила управления в качестве основы для быстрого запуска вашего оператора Runbook, чтобы ротация фьючерсов не была определена необходимым образом.Если эти проверки завершены, вы можете предать `nexus.routing_policy.default_lane` как источник достоверной информации для конфигурации SDK и начать деактивацию однополосных кодов, хранящихся в резерве.