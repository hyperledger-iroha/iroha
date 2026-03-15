---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-default-lane-quickstart
Название: Guia Rapida do Lane Padrao (NX-5)
Sidebar_label: Быстрый переход по полосе движения
описание: Настройте проверку или резервный вариант полосы движения для Nexus для Torii и SDK, если вы можете опустить line_id в общедоступных полосах.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/quickstart/default_lane.md`. Мантенья представился как копиас alinhadas ел, что ревизия локализации была сделана на портале.
:::

# Guia Rapida Do Lane Padrao (NX-5)

> **Контекст дорожной карты:** NX-5 — интеграция в общественную жизнь. Во время выполнения вы можете использовать резервный вариант `nexus.routing_policy.default_lane` для конечных точек REST/gRPC для Torii и каждый SDK, который можно оставить в безопасности с `lane_id`, когда он будет доступен для трафика на публичной канонической полосе. Это руководство по настройке каталога, проверке или резервному использованию `/status` и выполнению действий по настройке клиента по мосту и понту.

## Предварительные требования

- Эм, сборка Sora/Nexus de `irohad` (выполните `irohad --sora --config ...`).
- Доступ к хранилищу настроек для редактирования файлов `nexus.*`.
- `iroha_cli` настроен для работы с каждым кластером.
- `curl`/`jq` (или эквивалент) для проверки полезной нагрузки `/status` до Torii.

## 1. Описание каталога дорожек и пространств данных

Объявите полосы и пространства данных, которые будут существовать заново. В трех случаях (запись `defaults/nexus/config.toml`) зарегистрируйте три публичных полосы, которые являются псевдонимами корреспондентов пространства данных:

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

Каждый `index` был единым и непрерывным. Идентификаторы пространства данных имеют размер 64 бита; мы используем примеры, которые мы используем, и числовые значения, которые являются индексами полосы движения для большей ясности.

## 2. Определите подходящие места и возможные варианты

В течение секунды `nexus.routing_policy` контролирует резервную полосу и позволяет обнаруживать или вращать специальные инструкции или префиксы информации. Если нет соответствующей проверки, или планировщик автоматически выполняет транзакцию для конфигураций `default_lane` и `default_dataspace`. Логика маршрутизатора в `crates/iroha_core/src/queue/router.rs` и политика прозрачности формы, как поверхности REST/gRPC, для Torii.

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

Когда вы проголосуете за новые полосы будущего, начните сначала или каталог и отложите его, как regras de roteamento. Резервный вариант следует продолжать использовать для публичного движения, на котором сосредоточена большая часть трафика пользователей, чтобы SDK были альтернативными постоянными конкурентами.

## 3. Начало узла с политическим приложением

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Узел регистрации политической политики во время запуска. Quaisquer erros de validacao (indexes ausentes, alias duplicados, id de dataspace validos) появляются перед началом сплетен.

## 4. Подтвердите статус правительства на полосе движения.

Если узел подключен к сети, используйте помощник CLI для проверки того, что полоса движения находится в незанятом состоянии (манифест загрузки) и быстро для перемещения. Виза о резюме импрайм на линии:

```bash
iroha_cli app nexus lane-report --summary
```

Пример вывода:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```Если полоса движения находится под номером `sealed`, вы можете использовать справочник управления полосами движения до разрешения внешнего движения. Флаг `--fail-on-sealed` используется для CI.

## 5. Проверка состояния полезных данных Torii

Ответ `/status` раскрывает политику ротации каждый раз или делает снимок для планировщика по полосе. Используйте `curl`/`jq` для подтверждения конфигураций полей и проверки наличия резервной полосы телеметрии:

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

Для проверки работоспособности планировщика на полосе `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Подтвердите, что снимок TEU, метаданные псевдонима и флаги манифеста включены в конфигурацию. Основная полезная нагрузка и использование пелосов Grafana для панели управления полосой пропускания.

## 6. Упражняйтесь с клиентами

- **Rust/CLI.** `iroha_cli` и ящик клиента Rust опустите или запустите `lane_id`, когда прозвучит `--lane-id` / `LaneSelector`. На маршрутизаторе для файлов, портанто, введите `default_lane`. Используйте явные флаги `--lane-id`/`--dataspace-id` для доступа к mirar um Lane nao Padrao.
- **JS/Swift/Android.** В последних выпусках SDK используются `laneId`/`lane_id` как опции и резервный вариант, чтобы объявить о доблести для `/status`. Mantenha a politica de roteamento sincronizada entre staging e producao para que apps moveis nao precisem de reconfiguracoes de emergencia.
- **Тестирование конвейера/SSE.** Фильтры событий трансакционного обмена предсказываются `tx_lane_id == <u32>` (veja `docs/source/pipeline.md`). Assine `/v2/pipeline/events/transactions` com является фильтром для проверки того, что пишется в поле, указанном в явном виде, как или идентификатор резервной полосы.

## 7. Наблюдение и контроль над правительством

- `/status` также публикуется `nexus_lane_governance_sealed_total` и `nexus_lane_governance_sealed_aliases` для того, чтобы Alertmanager сообщал, когда полоса пропускает его манифест. Мантенья должен быть предупрежден о своих навыках в развитии.
- На карте телеметрии планировщика и панели управления полосами (`dashboards/grafana/nexus_lanes.json`) проверьте псевдонимы/ссылки каталога. Если вы проголосуете за изменение псевдонима, повторно зарегистрируйтесь в директорах Kura, чтобы аудиторы определили ваши предпочтения (растредо от NX-1).
- Парламентские выборы утверждают, что необходимо включить план отката. Зарегистрируйте или хеш-манифест и свидетельство об управлении в качестве быстрого запуска без своей рабочей книги оператора, чтобы будущие повороты не были достигнуты или требуются.

После того, как вы прошли проверку, вы можете использовать `nexus.routing_policy.default_lane` в качестве шрифта для настройки SDK, а также отключить альтернативные коды для единой полосы движения.