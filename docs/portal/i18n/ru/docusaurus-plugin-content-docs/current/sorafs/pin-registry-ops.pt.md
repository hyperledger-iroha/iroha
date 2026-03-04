---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pin-registry-ops
Название: Operacoes do Pin Registry
Sidebar_label: Operacos делает реестр контактов
описание: Мониторинг и триагем регистрации контактов SoraFS и показателей репликации SLA.
---

:::примечание Fonte canonica
Эспелья `docs/source/sorafs/runbooks/pin_registry_ops.md`. Мантенья посол в качестве синхронизированных версий съел aposentadoria da documentacao herdada do Sphinx.
:::

## Визао гераль

Этот документ Runbook представляет собой мониторинг и триагирование в реестре контактов SoraFS и своих соглашениях об уровне обслуживания (SLA) репликации. В качестве исходных метрик `iroha_torii` и их экспортируют через Prometheus из пространства имен `torii_sorafs_*`. Torii находится в реестре с интервалом в 30 секунд в втором плане, поэтому панели мониторинга постоянно настраиваются, когда оператор не проводит консультации с конечными точками `/v1/sorafs/pin/*`. Импортируйте Курадо приборной панели (`docs/source/grafana_sorafs_pin_registry.json`) для макета Grafana, чтобы использовать его непосредственно для безопасного использования.

## Ссылка на метрики

| Метрика | Этикетки | Описание |
| ------- | ------ | --------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Инвентаризация манифестов в сети для режима циклической жизни. |
| `torii_sorafs_registry_aliases_total` | - | Заражение псевдонимов манифестных действий, зарегистрированных без регистрации. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Отставание сегментированных заказов на репликацию по статусу. |
| `torii_sorafs_replication_backlog_total` | - | Удобный датчик, соответствующий заказу `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Уведомление об SLA: `met` содержит приказы, заключенные в срок, `missed` совокупные выводы с опозданием + истечение срока действия, `pending` ожидаются сроки. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latencia agregada de conclusao (epocas entre emissao e conclusao). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Janelas de folga de ordens pendentes (крайний срок menos epoca de emissao). |

Все датчики создаются в виде ежедневного снимка, а приборные панели становятся автоматическими в каденции `1m` или более быстро.

## Панель управления do Grafana

В формате JSON панель мониторинга включает в себя набор данных, которые могут привести к потокам рабочих операций. В качестве консультаций abaixo служит как референсия Rapida Caso voce Prefira Montar Graficos Sob Medida.

1. **Цикл просмотра манифестов** — `torii_sorafs_registry_manifests_total` (агрегирован для `status`).
2. **Тенденция к каталогу псевдонимов** — `torii_sorafs_registry_aliases_total`.
3. **Состояние заказов** — `torii_sorafs_registry_orders_total` (агрегировано по `status`).
4. **Невыполненные заказы и срок действия** — комбинация `torii_sorafs_replication_backlog_total` и `torii_sorafs_registry_orders_total{status="expired"}` для насыщения.
5. **Успех SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Latencia против крайнего срока для folga do** — sobrepoe `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` и `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Используйте преобразования Grafana для дополнительных просмотров `min_over_time`, когда требуется абсолютная точность, например:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordens perdidas (таксон 1h)** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Ограничения оповещения

- **Успех SLA  0**
  - Имя: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Акао: Инспекция государственных деклараций для подтверждения оттока поставщиков.
- **p95 заключения > указанный срок**
  - Имя: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acao: Убедитесь, что поставщики услуг достигли установленных сроков; рассмотреть реатрибуции.

### Проверка примера Prometheus

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
          summary: "SLA de replicacao SoraFS abaixo da meta"
          description: "A razao de sucesso do SLA ficou abaixo de 95% por 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicacao SoraFS acima do limiar"
          description: "Ordens de replicacao pendentes excederam o budget configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordens de replicacao SoraFS expiradas"
          description: "Pelo menos uma ordem de replicacao expirou nos ultimos cinco minutos."
```

## Fluxo de triagem

1. **Идентификатор причины**
   - Если промахи по соглашению об уровне обслуживания увеличиваются или постоянно отстают от других поставщиков, то они не могут быть удалены от поставщиков (falhas de PoR, выводы с опозданием).
   - Если в журнале пропущены задержки, проверьте допуск (`/v1/sorafs/pin/*`) для подтверждения декларации об утверждении согласия.
2. **Поставщики действительных статусов**
   - Выполните команду `iroha app sorafs providers list` и проверьте, есть ли возможность объявлять о необходимости репликации.
   - Проверьте датчики `torii_sorafs_capacity_*` для подтверждения обеспечения GiB и успеха PoR.
3. **Реплика ретрибута**
   - Новые заказы отправляются через `sorafs_manifest_stub capacity replication-order`, когда в списке невыполненной работы (`stat="avg"`) появляются 5 эпох (или empacotamento de Manifest/CAR USA `iroha app sorafs toolkit pack`).
   - Уведомлять об управлении псевдонимами в привязках к активным манифестам (неожиданно в `torii_sorafs_registry_aliases_total`).
4. **Результат документального исследования**
   - Регистрируйте заметки об инцидентах и журналы операций SoraFS с временными метками и дайджестами манифестных событий.
   - Настройте этот Runbook, используя новые моды или информационные панели, прежде чем вводить их.

## План развертывания

Вот этот процесс на этапах, связанных с умением или терпеливым политиком кэша псевдонимов в производстве:1. **Подготовка конфигурации**
   - Атуализировать `torii.sorafs_alias_cache` в `iroha_config` (пользователь -> фактический) с TTL и дополнительными настройками: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` и `governance_grace`. Наши настройки по умолчанию соответствуют политике `docs/source/sorafs_alias_policy.md`.
   - Для SDK распространение важных значений для ваших настроек (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` в привязках Rust/NAPI/Python) для обеспечения соответствия клиента шлюзу.
2. **Пробный прогон**
   - Внедрите измененную конфигурацию в промежуточный кластер, который создаст топологию производства.
   - Выполните `cargo xtask sorafs-pin-fixtures` для подтверждения того, что канонические устройства псевдонимов, декодированные и фазированные туда и обратно; такое несоответствие подразумевает смещение вверх по течению, которое должно быть решено в первую очередь.
   - Конечные точки Exerca os `/v1/sorafs/pin/{digest}` и `/v1/sorafs/aliases` com provas sinteticas cobrindo casos свежие, с обновлением окна, с истекшим сроком действия и с жестким сроком действия. Действуют коды HTTP, заголовки (`Sora-Proof-Status`, `Retry-After`, `Warning`) и кампусы корпоративного JSON против этого Runbook.
3. **Навыки производства**
   - Имплантируйте новую конфигурацию на крыльце муданки. Примените первый вариант Torii и используйте SDK шлюзов/сервисов для подтверждения узла, подтверждающего новую политику в наших журналах.
   - Импортируйте `docs/source/grafana_sorafs_pin_registry.json` без Grafana (или реализуйте существующие информационные панели) и исправьте боль при обновлении кэша псевдонимов без рабочего пространства в NOC.
4. **Проверка после развертывания**
   - Мониторинг `torii_sorafs_alias_cache_refresh_total` и `torii_sorafs_alias_cache_age_seconds` на 30 минут. Изображения кривых `error`/`expired` должны быть коррелированы с возможностью обновления; Crescimento inesperado означает, что операторы должны проверять проверки псевдонимов и других поставщиков перед продолжением.
   - Подтвердите, что журналы отправляются клиентам в соответствии с политическими решениями (SDK излучают ошибки, когда доказывается, что они устарели или истекли). Предупреждения указывают на неверную конфигурацию клиента.
5. **Резервный вариант**
   - При изменении псевдонима и изменении частоты обновления, временно расслабьте политику, увеличив `refresh_window` и `positive_ttl` в конфигурации, и повторно внедрите ее. Mantenha `hard_expiry` нетронутым, чтобы доказать, что он действительно устарел и сейчас находится в безопасности.
   - Вернитесь к конфигурации переднего восстановления или переднему снимку `iroha_config`, если телеметрия продолжится после заражения `error` на возвышениях, которые будут удалены от инцидента для получения изображений с удаленным псевдонимом.

## Связанные материалы

- `docs/source/sorafs/pin_registry_plan.md` - дорожная карта реализации и контекста управления.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - операции работают с хранилищем, дополняют эту книгу игр для реестра.