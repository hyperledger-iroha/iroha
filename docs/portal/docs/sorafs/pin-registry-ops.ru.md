---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 77e29c2698a07a07dbfc1e90daaeb9c5a9ba4fbf0f07184200dc937f67cf8df8
source_last_modified: "2025-11-06T04:13:03.802876+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: pin-registry-ops
title: Операции Pin Registry
sidebar_label: Операции Pin Registry
description: Мониторинг и triage Pin Registry SoraFS и метрик SLA репликации.
---

:::note Канонический источник
Отражает `docs/source/sorafs/runbooks/pin_registry_ops.md`. Держите обе версии синхронизированными, пока наследственная документация Sphinx не будет выведена из эксплуатации.
:::

## Обзор

Этот runbook описывает, как мониторить и выполнять triage Pin Registry SoraFS и его соглашения об уровне сервиса (SLA) для репликации. Метрики поступают из `iroha_torii` и экспортируются через Prometheus под namespace `torii_sorafs_*`. Torii опрашивает состояние registry каждые 30 секунд в фоне, поэтому dashboards остаются актуальными даже когда никто из операторов не запрашивает endpoints `/v1/sorafs/pin/*`. Импортируйте подготовленный dashboard (`docs/source/grafana_sorafs_pin_registry.json`) для готового layout Grafana, который напрямую соответствует разделам ниже.

## Справочник метрик

| Метрика | Labels | Описание |
| ------ | ------ | -------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Инвентаризация manifests on-chain по состояниям жизненного цикла. |
| `torii_sorafs_registry_aliases_total` | — | Количество активных alias manifests, записанных в registry. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog заказов репликации, сегментированный по статусу. |
| `torii_sorafs_replication_backlog_total` | — | Удобный gauge, отражающий `pending` заказы. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Учет SLA: `met` считает заказы, завершенные в срок, `missed` агрегирует поздние завершения + истечения, `pending` отражает незавершенные заказы. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Агрегированная латентность завершения (эпохи между выпуском и завершением). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Окна запаса для незавершенных заказов (deadline минус эпоха выдачи). |

Все gauges сбрасываются при каждом pull snapshot, поэтому dashboards должны опрашиваться с частотой `1m` или быстрее.

## Dashboard Grafana

JSON dashboard содержит семь панелей, покрывающих рабочие процессы операторов. Запросы перечислены ниже для быстрого справочника, если вы предпочитаете строить собственные графики.

1. **Жизненный цикл manifests** – `torii_sorafs_registry_manifests_total` (группировка по `status`).
2. **Тренд каталога alias** – `torii_sorafs_registry_aliases_total`.
3. **Очередь заказов по статусу** – `torii_sorafs_registry_orders_total` (группировка по `status`).
4. **Backlog vs истекшие заказы** – объединяет `torii_sorafs_replication_backlog_total` и `torii_sorafs_registry_orders_total{status="expired"}` для выявления насыщения.
5. **Коэффициент успеха SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Латентность vs запас до deadline** – наложите `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` и `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Используйте трансформации Grafana, чтобы добавить представления `min_over_time`, когда нужен абсолютный нижний предел запаса, например:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Пропущенные заказы (rate 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Пороговые значения алертов

- **Успех SLA < 0.95 в течение 15 мин**
  - Порог: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Действие: Пейджинг SRE; начать triage backlog репликации.
- **Pending backlog выше 10**
  - Порог: `torii_sorafs_replication_backlog_total > 10` сохраняется 10 мин
  - Действие: Проверить доступность providers и планировщик емкости Torii.
- **Истекшие заказы > 0**
  - Порог: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Действие: Проверить governance manifests, чтобы подтвердить churn providers.
- **p95 завершения > средний запас до deadline**
  - Порог: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: Убедиться, что providers завершают до deadline; рассмотреть перераспределение.

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

## Workflow triage

1. **Определить причину**
   - Если пропуски SLA растут, а backlog остается низким, сосредоточьтесь на производительности providers (сбои PoR, поздние завершения).
   - Если backlog растет при стабильных пропусках, проверьте admission (`/v1/sorafs/pin/*`), чтобы подтвердить manifests, ожидающие утверждения совета.
2. **Проверить статус providers**
   - Запустите `iroha app sorafs providers list` и убедитесь, что заявленные возможности соответствуют требованиям репликации.
   - Проверьте gauges `torii_sorafs_capacity_*`, чтобы подтвердить provisioned GiB и успех PoR.
3. **Перераспределить репликацию**
   - Выпустите новые заказы через `sorafs_manifest_stub capacity replication-order`, когда запас backlog (`stat="avg"`) опустится ниже 5 эпох (упаковка manifest/CAR использует `iroha app sorafs toolkit pack`).
   - Уведомите governance, если aliases не имеют активных binding manifests (неожиданное падение `torii_sorafs_registry_aliases_total`).
4. **Задокументировать результат**
   - Запишите заметки инцидента в журнал операций SoraFS с timestamp и digest manifests.
   - Обновите этот runbook, если появляются новые режимы отказа или dashboards.

## План развертывания

Следуйте этому поэтапному процессу при включении или усилении политики alias cache в продакшене:

1. **Подготовить конфигурацию**
   - Обновите `torii.sorafs_alias_cache` в `iroha_config` (user -> actual) с согласованными TTL и окнами грации: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. Значения по умолчанию соответствуют политике в `docs/source/sorafs_alias_policy.md`.
   - Для SDKs распространите те же значения через их конфигурационные слои (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` в bindings Rust / NAPI / Python), чтобы enforcement клиента совпадал с gateway.
2. **Dry-run в staging**
   - Разверните изменение конфигурации в staging-кластере, который отражает топологию продакшена.
   - Запустите `cargo xtask sorafs-pin-fixtures`, чтобы убедиться, что канонические alias fixtures по-прежнему декодируются и проходят round-trip; любое несовпадение означает upstream drift, который нужно устранить первым.
   - Прогоните endpoints `/v1/sorafs/pin/{digest}` и `/v1/sorafs/aliases` с синтетическими proof, покрывающими случаи fresh, refresh-window, expired и hard-expired. Проверьте HTTP коды, headers (`Sora-Proof-Status`, `Retry-After`, `Warning`) и поля JSON тела относительно этого runbook.
3. **Включить в продакшене**
   - Разверните новую конфигурацию в стандартное окно изменений. Сначала примените к Torii, затем перезапустите gateways/SDK сервисы после подтверждения новой политики в логах узла.
   - Импортируйте `docs/source/grafana_sorafs_pin_registry.json` в Grafana (или обновите существующие dashboards) и закрепите панели refresh alias cache в рабочем пространстве NOC.
4. **Проверка после развертывания**
   - Мониторьте `torii_sorafs_alias_cache_refresh_total` и `torii_sorafs_alias_cache_age_seconds` в течение 30 минут. Пики в кривых `error`/`expired` должны коррелировать с окнами refresh; неожиданный рост означает, что операторы должны проверить proof aliases и состояние providers перед продолжением.
   - Убедитесь, что клиентские логи показывают те же решения политики (SDKs будут выдавать ошибки, когда proof устарел или истек). Отсутствие предупреждений клиента означает неверную конфигурацию.
5. **Fallback**
   - Если выпуск alias отстает и окно refresh часто срабатывает, временно ослабьте политику, увеличив `refresh_window` и `positive_ttl` в config, затем выполните повторный деплой. Оставьте `hard_expiry` неизменным, чтобы действительно устаревшие proof продолжали отклоняться.
   - Вернитесь к предыдущей конфигурации, восстановив прежний snapshot `iroha_config`, если телеметрия продолжает показывать повышенные значения `error`, затем откройте инцидент для расследования задержек генерации alias.

## Связанные материалы

- `docs/source/sorafs/pin_registry_plan.md` — дорожная карта реализации и контекст управления.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — операции storage worker, дополняет этот playbook registry.
