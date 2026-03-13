---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pin-registry-ops
Название: Операции по регистрации контактов
Sidebar_label: Операции с реестром контактов
описание: Мониторинг и триаже реестра контактов SoraFS и показателей репликации SLA.
---

:::примечание Фуэнте каноника
Рефлея `docs/source/sorafs/runbooks/pin_registry_ops.md`. Многие из синхронизированных версий должны удалить наследственную документацию Сфинкса.
:::

## Резюме

Этот документ Runbook предназначен для мониторинга и создания реестра контактов SoraFS и уровня обслуживания (SLA) репликации. Метрики предоставляются `iroha_torii` и экспортируются через Prometheus в пространство имен `torii_sorafs_*`. Torii показывает состояние реестра в интервале в 30 секунд на втором плане, поэтому панели мониторинга постоянно актуализируются, включая тот момент, когда оператор работает с конечными точками `/v2/sorafs/pin/*`. Импортируйте панель управления (`docs/source/grafana_sorafs_pin_registry.json`) для макета списка Grafana, чтобы использовать его непосредственно в нужных разделах.

## Справочные данные по метрикам

| Метрика | Этикетки | Описание |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Инвентаризация манифестов в сети для состояния цикла жизни. |
| `torii_sorafs_registry_aliases_total` | — | Сведения о псевдонимах манифестов зарегистрированных действий в реестре. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Отставание в сегментированных заказах по репликации. |
| `torii_sorafs_replication_backlog_total` | — | Удобный датчик, отражающий сигналы `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Информация об SLA: `met`, когда заказы завершены в срок, `missed`, совокупность завершенных с опозданием + срок действия, `pending`, отражающие ожидаемые заказы. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latencia agregada de Finalización (эпоха между выбросами и завершением). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Ventanas de holgura de órdenes pendientes (крайний срок периодического выброса). |

Все датчики перезагружаются в каждый момент получения моментального снимка, так как информационные панели должны быть проверены с частотой `1m` или более быстро.

## Панель управления Grafana

Панель управления в формате JSON включает в себя панели, на которых отображаются рабочие процессы операторов. Консультации можно просмотреть для быстрой ссылки, если вы предпочитаете создавать графические изображения на медиуме.

1. **Цикл просмотра манифестов** – `torii_sorafs_registry_manifests_total` (агрегирован для `status`).
2. **Тенденция каталога псевдонимов** – `torii_sorafs_registry_aliases_total`.
3. **Кола де Орденес по эстадо** – `torii_sorafs_registry_orders_total` (агрегат по `status`).
4. **Отставание по сроку действия** – комбинация `torii_sorafs_replication_backlog_total` и `torii_sorafs_registry_orders_total{status="expired"}` для быстрого насыщения.
5. **Коэффициент выхода SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Задержка и задержка крайнего срока** – суперпоне `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` и `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Используйте преобразования Grafana для просмотра изображений `min_over_time`, когда требуется абсолютная высота, например:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Órdenes Fallidas (tasa 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Зонты оповещения

- **Выход SLA  0**
  - Умбрал: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Действие: Проверка государственных деклараций для подтверждения оттока поставщиков.
- **p95 завершения > отсрочка срока**
  - Умбрал: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: Проверка того, что поставщики выполнили план до истечения крайнего срока; рассмотреть возможность повторного подписания.

### Правила примера Prometheus

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
          summary: "SLA de replicación de SoraFS por debajo del objetivo"
          description: "El ratio de éxito del SLA se mantuvo por debajo de 95% durante 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicación de SoraFS por encima del umbral"
          description: "Las órdenes pendientes excedieron el presupuesto de backlog configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Órdenes de replicación de SoraFS expiradas"
          description: "Al menos una orden de replicación expiró en los últimos cinco minutos."
```

## Flujo de triaje

1. **Идентификатор причины**
   - В случае промахов по SLA в период отставания в работе, обратите внимание на данные поставщиков (fallas de PoR, завершенные с опозданием).
   - Если накопилась задолженность по пропущенным объектам, проверьте допуск (`/v2/sorafs/pin/*`) для подтверждения декларации об апробации совета.
2. **Действительный статус поставщиков**
   - Вызовите `iroha app sorafs providers list` и проверьте, что объявленные возможности соответствуют требованиям для репликации.
   - Проверьте датчики `torii_sorafs_capacity_*` для подтверждения подготовки GiB и выхода PoR.
3. **Переназначение репликации**
   - Выполните новые заказы через `sorafs_manifest_stub capacity replication-order`, когда завершится невыполненная работа (`stat="avg"`) в течение 5 периодов (вкладка манифеста/CAR usa `iroha app sorafs toolkit pack`).
   - Уведомление губернатора о псевдонимах привязок манифестных действий (неожиданные сообщения `torii_sorafs_registry_aliases_total`).
4. **Результат документального исследования**
   - Регистрируйте примечания об инцидентах в журнале операций SoraFS с временными метками и дайджестами манифестов поражений.
   - Актуализация этого Runbook и появление новых модов на панелях мониторинга.

## План деспльега

Вот такая процедура для опытных или выносливых политиков кэша псевдонимов в производстве:1. **Подготовка конфигурации**
   - Актуализация `torii.sorafs_alias_cache` и `iroha_config` (пользователь -> факт) с TTL и изящными настройками: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` и `governance_grace`. Настройки по умолчанию совпадают с политикой в ​​`docs/source/sorafs_alias_policy.md`.
   - Для SDK распространяйте неверные значения, установленные в рамках конфигурации (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` в привязках Rust/NAPI/Python), чтобы клиентское приложение совпадало со шлюзом.
2. **Прогон и постановка**
   - Отобразите способ настройки в кластере промежуточной подготовки, который отражает топологию производства.
   - Вызовите `cargo xtask sorafs-pin-fixtures` для подтверждения того, что псевдонимы устройств декодируются и проходят туда и обратно; Более частое несоответствие подразумевает дрейф, когда нужно решить первый вопрос.
   - Извлекайте конечные точки `/v2/sorafs/pin/{digest}` и `/v2/sorafs/aliases` с синтетическими запросами, которые могут быть свежими, с обновлением окна, с истекшим сроком действия или с истекшим сроком действия. Действуют коды HTTP, заголовки (`Sora-Proof-Status`, `Retry-After`, `Warning`) и поля JSON с этим Runbook.
3. **Навыки производства**
   - Установите новую конфигурацию на стоянке вентиляционного отверстия. Первое применение к Torii и новому шлюзу/сервису SDK означает, что этот узел подтверждает новую политику в журналах.
   - Импорт `docs/source/grafana_sorafs_pin_registry.json` и Grafana (или существующие панели мониторинга) и панели обновления кэша псевдонима рабочего пространства NOC.
4. **Проверка после отправки**
   - Мониторинг `torii_sorafs_alias_cache_refresh_total` и `torii_sorafs_alias_cache_age_seconds` в течение 30 минут. Изображения на кривых `error`/`expired` должны коррелировать с вентиляционными отверстиями; Crecimiento inesperado подразумевает, что операторы должны проверять проверки псевдонимов и приветствовать поставщиков перед продолжением.
   - Подтвердите, что в журналах клиента указаны ошибочные политические решения (в SDK обнаружены ошибки, когда проверка устарела или истекла). Появление предупреждений клиента указывает на неправильную конфигурацию.
5. **Резервный вариант**
   - Если излучение псевдонима будет отменено и обновление будет нарушено с частотой, временное изменение политики увеличится `refresh_window` и `positive_ttl` в конфигурации, и вы потеряете его. Мантен `hard_expiry` нетронут, чтобы действительно устаревшие пакеты были заменены.
   - Вернитесь к предыдущей конфигурации, восстановленной перед моментальным снимком `iroha_config`, если телеметрия показывает, что большинство изображений `error` поднято, а затем произошел инцидент, чтобы вернуть изображение в поколение псевдонима.

## Относительные материалы

- `docs/source/sorafs/pin_registry_plan.md` — план реализации и контекст управления.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — операции по работе с хранилищем, дополняющие эту книгу реестра.