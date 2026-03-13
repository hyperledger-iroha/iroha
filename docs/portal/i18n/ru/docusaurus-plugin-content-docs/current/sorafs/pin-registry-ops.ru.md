---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pin-registry-ops
Название: Реестр контактов операций
sidebar_label: Реестр контактов операций
описание: Мониторинг и сортировка реестра контактов SoraFS и метрика репликации SLA.
---

:::note Канонический источник
Отражает `docs/source/sorafs/runbooks/pin_registry_ops.md`. Держите обе версии синхронизированными, пока наследственная документация Sphinx не будет выведена из эксплуатации.
:::

## Обзор

В этом модуле Runbook описывается, как отслеживать и выполнять сортировку реестра контактов SoraFS и его соглашение об уровне сервиса (SLA) для репликации. Метрики появляются из `iroha_torii` и экспортируются через Prometheus в пространство имен `torii_sorafs_*`. Torii запрашивает состояние реестра женщин в течение 30 секунд на фоне, поэтому информационные панели остаются актуальными, даже когда никто из операторов не запрашивает конечные точки `/v2/sorafs/pin/*`. Импортируйте подготовленный дашборд (`docs/source/grafana_sorafs_pin_registry.json`) для готового макета Grafana, который напрямую соответствует разделам ниже.

## Справочник метрики

| Метрика | Этикетки | Описание |
| ------ | ------ | -------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Инвентаризация проявляется в цепочке по состоянию жизненного цикла. |
| `torii_sorafs_registry_aliases_total` | — | Количество активных манифестов псевдонимов, записанных в реестре. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Журнал невыполненных заказов репликации, сегментированный по статусу. |
| `torii_sorafs_replication_backlog_total` | — | Удобный датчик, отражающий `pending` заказал. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Учет SLA: `met` заказы считает, завершенные в срок, `missed` объединяет поздние заключения + истечения, `pending` отражает незавершенные заказы. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Агрегированная латентность заключается (эпохи между выпуском и завершением). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Окна запаса для незавершенных заказов (срок минус эпохи выдачи). |

Все датчики сбрасываются при каждом получении снимка, поэтому информационные панели должны соответствовать уровню `1m` или выше.

## Панель управления Grafana

Панель мониторинга JSON содержит семь панелей, покрывающих рабочие процессы операторов. Запросы, приведенные ниже, для быстрого справочника, если вы предпочитаете строить собственные графики.

1. **Жизненный цикл манифеста** – `torii_sorafs_registry_manifests_total` (группировка по `status`).
2. **Псевдоним каталога трендов** – `torii_sorafs_registry_aliases_total`.
3. **Очередь заказов по статусу** – `torii_sorafs_registry_orders_total` (группировка по `status`).
4. **Backlog vs четкие заказы** – выборы `torii_sorafs_replication_backlog_total` и `torii_sorafs_registry_orders_total{status="expired"}` для определения насыщения.
5. **Коэффициент успеха SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Латентность vs запас до крайнего срока** – наложите `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` и `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Используйте трансформации Grafana, чтобы добавить представление `min_over_time`, когда нужен абсолютный нижний предел запаса, например:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Пропущенные заказы (тариф 1 час)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Пороговые значения оповещений

- **Успех SLA  0**
  - Порог: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Действие: проверка манифестов управления, чтобы предотвратить отток поставщиков.
- **p95 завершает > средний запас до крайнего срока**
  - Порог: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: Убедиться, что поставщики завершают работу в срок; обратите внимание на перераспределение.

### Пример правил Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SLA репликации SoraFS ниже целевого"
          description: "Коэффициент успеха SLA оставался ниже 95% в течение 15 минут."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog репликации SoraFS выше порога"
          description: "Ожидающие заказы репликации превысили настроенный бюджет backlog."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Заказы репликации SoraFS истекли"
          description: "По крайней мере один заказ репликации истек за последние пять минут."
```

## Сортировка рабочего процесса

1. **Определить причину**
   - Если пропуски SLA показывают, что отставание остается низким, сосредоточьтесь на производительности поставщиков (сбои PoR, поздние обязательства).
   - Если отставание растет при стабильных пропусках, проверьте прием (`/v2/sorafs/pin/*`), чтобы подтвердить надежность, ожидая заключения совета.
2. **Проверить статус поставщиков**
   - Запустите `iroha app sorafs providers list` и убедитесь, что заявленные возможности соответствуют требованиям репликации.
   - Проверьте датчики `torii_sorafs_capacity_*`, чтобы обеспечить надежность GiB и успех PoR.
3. **Перераспределить репликацию**
   - Выпустите новые заказы через `sorafs_manifest_stub capacity replication-order`, когда резерв резерва (`stat="avg"`) опустится ниже 5 этапов (упаковка манифеста/CAR использует `iroha app sorafs toolkit pack`).
   - Уведомите управление, если псевдонимы не имеют активных проявлений привязки (неожиданное `torii_sorafs_registry_aliases_total`).
4. **Задокументировать результат**
   - Запишите заметки об инциденте в журнал операций SoraFS с меткой времени и дайджест-манифестами.
   - Обновите этот Runbook, если созданы новые режимы или панели мониторинга.

## План развертывания

Следуйте этому поэтическому процессу при включении или усилении кэша псевдонимов политики в продакшене:1. **Подготовить конфигурацию**
   - Обновите `torii.sorafs_alias_cache` в `iroha_config` (пользователь -> актуальное) с согласованными TTL и оконными Грациями: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. Значения по умолчанию соответствуют заявлению `docs/source/sorafs_alias_policy.md`.
   - Для SDK распространяйте те же значения через их конфигурационные фрагменты (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` в привязках Rust/NAPI/Python), чтобы клиент совпадал со шлюзом.
2. **Прогон в постановке**
   - Развернуть изменение конфигурации в staging-кластере, который отражает топологию продакшена.
   - Запустите `cargo xtask sorafs-pin-fixtures`, чтобы убедиться, что канонические псевдонимы по-прежнему декодируются и передаются туда и обратно; любое несовпадение означает занос вверх по течению, который необходимо устранить первым.
   - Прогоните конечные точки `/v2/sorafs/pin/{digest}` и `/v2/sorafs/aliases` с синтетическими доказательствами, покрывающими случаи свежего, обновления окна, просроченного и окончательно истекшего срока действия. Проверьте HTTP-коды, заголовки (`Sora-Proof-Status`, `Retry-After`, `Warning`) и поля JSON тела относительно этого модуля Runbook.
3. **Включить в продакшене**
   - Развернуть новую конфигурацию в стандартное окно изменений. Сначала замените Torii, а затем перезапустите шлюзы/сервисы SDK после подтверждения новой политики в логах узла.
   - Импортируйте `docs/source/grafana_sorafs_pin_registry.json` в Grafana (или обновите панели мониторинга) и закрепите панель обновления кэша псевдонимов в рабочем пространстве NOC.
4. **Проверка после развертывания**
   - Мониторьте `torii_sorafs_alias_cache_refresh_total` и `torii_sorafs_alias_cache_age_seconds` в течение 30 минут. Пики в кривых `error`/`expired` должны коррелировать с окнами обновления; неожиданный рост означает, что операторы должны проверять псевдонимы и состояния поставщиков перед продолжением.
   - Убедитесь, что клиентские логи отображают те же политики решений (SDK будут выдавать ошибки, когда доказательства устареют или исчезнут). Отсутствие предупреждений клиента означает неверную конфигурацию.
5. **Резервный вариант**
   - Если псевдоним выпуска отстает и окно часто обновляется, временно ослабьте политику, увеличьте `refresh_window` и `positive_ttl` в конфигурации, а затем выполните повторную деплой. Оставьте `hard_expiry` неизменным, чтобы действительное доказательство присутствия продолжали отклоняться.
   - Вернитесь к приведенной конфигурации, создав прежний снимок `iroha_config`, если телеметрия продолжает показывать повышенные значения `error`, а затем инцидент, связанный с инцидентами, вызывающими задержек генерации псевдонима.

## Связанные материалы

- `docs/source/sorafs/pin_registry_plan.md` — дорогая реализация карты и контекстного управления.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — рабочий хранилища операций, выполняет этот реестр playbook.