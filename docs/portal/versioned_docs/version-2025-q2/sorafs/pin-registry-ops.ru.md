---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T15:38:30.656337+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: pin-registry-ops-ru
slug: /sorafs/pin-registry-ops-ru
---

:::обратите внимание на канонический источник
Зеркала `docs/source/sorafs/runbooks/pin_registry_ops.md`. Обе версии должны быть согласованы между выпусками.
:::

## Обзор

В этом модуле Runbook описано, как отслеживать и сортировать контактный реестр SoraFS и его соглашения об уровне обслуживания репликации (SLA). Метрики берутся из `iroha_torii` и экспортируются через Prometheus в пространстве имен `torii_sorafs_*`. Torii производит выборку состояния реестра с 30-секундным интервалом в фоновом режиме, поэтому информационные панели остаются актуальными, даже если ни один оператор не опрашивает конечные точки `/v2/sorafs/pin/*`. Импортируйте проверенную панель мониторинга (`docs/source/grafana_sorafs_pin_registry.json`) для получения готового к использованию макета Grafana, который напрямую соответствует разделам ниже.

## Ссылка на метрику

| Метрическая | Этикетки | Описание |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Инвентаризация манифестов в цепочке по состоянию жизненного цикла. |
| `torii_sorafs_registry_aliases_total` | — | Количество активных псевдонимов манифеста, записанных в реестре. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Журнал заказов на репликацию, сегментированный по статусу. |
| `torii_sorafs_replication_backlog_total` | — | Комфортный манометр, зеркально отображающий заказы `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Учет SLA: `met` подсчитывает выполненные заказы в срок, `missed` агрегирует просроченные заказы + истечение срока действия, `pending` отражает невыполненные заказы. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Совокупная задержка завершения (периоды между выдачей и завершением). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Временные окна отложенных ордеров (крайний срок минус выданная эпоха). |

Все датчики сбрасываются при каждом получении снимка, поэтому информационные панели должны производить выборку с частотой `1m` или быстрее.

## Grafana Панель мониторинга

Панель мониторинга JSON поставляется с семью панелями, которые отображают рабочие процессы оператора. Запросы перечислены ниже для быстрого ознакомления, если вы предпочитаете создавать индивидуальные диаграммы.

1. **Жизненный цикл манифеста** — `torii_sorafs_registry_manifests_total` (сгруппировано по `status`).
2. **Тенденция каталога псевдонимов** – `torii_sorafs_registry_aliases_total`.
3. **Упорядочить очередь по статусу** — `torii_sorafs_registry_orders_total` (сгруппировано по `status`).
4. **Незавершенные заказы по сравнению с просроченными заказами** – объединяет `torii_sorafs_replication_backlog_total` и `torii_sorafs_registry_orders_total{status="expired"}` для поверхностного насыщения.
5. **Коэффициент успеха SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Задержка и просрочка срока** – наложение `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` и `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Используйте преобразования Grafana для добавления видов `min_over_time`, когда вам нужен абсолютный провис пола, например:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Пропущенные заказы (ставка за 1 час)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Пороги оповещений- **Успех по SLA  0**
  - Порог: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Действие: проверьте манифесты управления, чтобы подтвердить отток поставщиков.
- **Завершение стр.95 > среднее отставание от срока**
  - Порог: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: Проверить, что поставщики берут на себя обязательства до наступления сроков; рассмотреть вопрос о переназначении.

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
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Рабочий процесс сортировки

1. **Определите причину**
   - Если SLA не достигает пика, а отставание остается небольшим, сосредоточьтесь на производительности поставщика (сбои PoR, позднее завершение).
   - Если отставание растет из-за стабильных промахов, проверьте прием (`/v2/sorafs/pin/*`), чтобы подтвердить манифесты, ожидающие одобрения совета.
2. **Подтвердите статус поставщика**
   - Запустите `iroha app sorafs providers list` и убедитесь, что заявленные возможности соответствуют требованиям репликации.
   - Проверьте датчики `torii_sorafs_capacity_*`, чтобы подтвердить успешность предоставления GiB и PoR.
3. **Переназначить репликацию**
   - Выдавайте новые заказы через `sorafs_manifest_stub capacity replication-order`, когда резерв резерва (`stat="avg"`) падает ниже 5 эпох (упаковка манифеста/CAR использует `iroha app sorafs toolkit pack`).
   — Уведомляйте руководство, если у псевдонимов отсутствуют активные привязки манифеста (неожиданно прерывается `torii_sorafs_registry_aliases_total`).
4. **Документируйте результат**
   - Записывайте примечания об инцидентах в журнал операций SoraFS с отметками времени и затронутыми дайджестами манифеста.
   — Обновите этот модуль Runbook, если появятся новые режимы сбоя или панели мониторинга.

## План развертывания

Следуйте этой поэтапной процедуре при включении или ужесточении политики кэширования псевдонимов в рабочей среде:1. **Подготовка конфигурации**
   - Обновите `torii.sorafs_alias_cache` в `iroha_config` (пользователь → фактическое) с согласованными TTL и льготными окнами: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` и `governance_grace`. Значения по умолчанию соответствуют политике в `docs/source/sorafs_alias_policy.md`.
   — Для SDK распространяйте одни и те же значения по уровням конфигурации (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` в привязках Rust/NAPI/Python), чтобы принудительное применение клиента соответствовало шлюзу.
2. **Прогон в стадии подготовки**
   — Разверните изменение конфигурации в промежуточном кластере, отражающем производственную топологию.
   - Запустите `cargo xtask sorafs-pin-fixtures`, чтобы убедиться, что канонические псевдонимы все еще декодируются и проходят туда и обратно; любое несоответствие подразумевает отклонение манифеста восходящего потока, которое необходимо устранить в первую очередь.
   - Испытайте конечные точки `/v2/sorafs/pin/{digest}` и `/v2/sorafs/aliases` с помощью синтетических доказательств, охватывающих свежие случаи, случаи обновления окна, просроченные и окончательно истекшие сроки действия. Проверьте коды состояния HTTP, заголовки (`Sora-Proof-Status`, `Retry-After`, `Warning`) и поля тела JSON на соответствие этому Runbook.
3. **Включить в производство**
   - Разверните новую конфигурацию через стандартное окно изменений. Сначала примените его к Torii, затем перезапустите службы шлюзов/SDK, как только узел подтвердит новую политику в журналах.
   - Импортируйте `docs/source/grafana_sorafs_pin_registry.json` в Grafana (или обновите существующие информационные панели) и закрепите панели обновления кэша псевдонимов в рабочей области NOC.
4. **Проверка после развертывания**
   - Контролируйте `torii_sorafs_alias_cache_refresh_total` и `torii_sorafs_alias_cache_age_seconds` в течение 30 минут. Пики на кривых `error`/`expired` должны коррелировать с периодами обновления политики; неожиданный рост означает, что операторы должны проверить доказательства псевдонимов и работоспособность поставщика, прежде чем продолжить.
   - Убедитесь, что журналы на стороне клиента показывают одни и те же решения политики (SDK выявляет ошибки, когда доказательство устарело или срок его действия истек). Отсутствие предупреждений клиента указывает на неправильную настройку.
5. **Резервный вариант**
   - Если выдача псевдонимов задерживается и окно обновления часто отключается, временно ослабьте политику, увеличив `refresh_window` и `positive_ttl` в конфигурации, а затем повторно разверните. Сохраняйте `hard_expiry` нетронутым, чтобы действительно устаревшие доказательства по-прежнему отклонялись.
   — Вернитесь к предыдущей конфигурации, восстановив предыдущий снимок `iroha_config`, если телеметрия продолжает показывать повышенное количество `error`, а затем откройте инцидент, чтобы отследить задержки создания псевдонимов.

## Сопутствующие материалы

- `docs/source/sorafs/pin_registry_plan.md` — план реализации и контекст управления.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — операции работника хранилища, дополняют этот сборник сценариев реестра.
