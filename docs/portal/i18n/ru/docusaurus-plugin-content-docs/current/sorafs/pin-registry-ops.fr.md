---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pin-registry-ops
Название: Operations du Pin Registry
Sidebar_label: Операции с реестром Pin
описание: Просмотр и проверка реестра контактов SoraFS и показателей репликации SLA.
---

:::note Источник канонический
Reflète `docs/source/sorafs/runbooks/pin_registry_ops.md`. Обратите внимание на две синхронизированные версии, которые возвращают документацию Sphinx héritée.
:::

## ансамбль

Этот runbook записывает комментарии, наблюдает за реестром контактов SoraFS и соответствует уровню обслуживания (SLA) репликации. Метрики, предоставленные `iroha_torii`, экспортируются через Prometheus в пространстве имен `torii_sorafs_*`. Torii запускается в режиме реестра в течение 30 секунд в соответствии с планом задержки, оставляя панели мониторинга в течение дня, когда вы можете выполнить опрос конечных точек `/v1/sorafs/pin/*`. Импортируйте кураторскую панель мониторинга (`docs/source/grafana_sorafs_pin_registry.json`) для макета Grafana, готового к работе, который соответствует направлению других разделов.

## Ссылка на метрики

| Метрика | Этикетки | Описание |
| ------- | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Изобретение манифестов в сети на уровне жизненного цикла. |
| `torii_sorafs_registry_aliases_total` | — | Псевдонимы манифестных действий, зарегистрированных в реестре. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Невыполненные заказы сегментированы по закону. |
| `torii_sorafs_replication_backlog_total` | — | Манометр, отражающий заказы `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Совместимое соглашение об уровне обслуживания: `met` учитывает завершенные заказы в задержках, `missed` объединяет поздние завершения + сроки действия, `pending` отражает принятые заказы. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Задержка завершения завершения (époques entre émission et complétion). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Fenêtres de marge des ordres en attente (крайний срок moins époque d'émission). |

Все датчики повторно инициализируются для извлечения моментального снимка, а панели мониторинга должны переключаться на каденцию `1m` или более быструю.

## Панель управления Grafana

Содержимое информационной панели в формате JSON доступно для операторов рабочих процессов. Запросы в списках могут быть быстрыми, если вы предпочитаете создавать графические изображения по мере необходимости.

1. **Цикл деклараций** – `torii_sorafs_registry_manifests_total` (группа по параметру `status`).
2. **Тенденция к каталогу псевдонимов** – `torii_sorafs_registry_aliases_total`.
3. **Файл по уставу** – `torii_sorafs_registry_orders_total` (группа по `status`).
4. **Незавершенные заказы и заказы с истекшим сроком действия** — объедините `torii_sorafs_replication_backlog_total` и `torii_sorafs_registry_orders_total{status="expired"}`, чтобы убедиться в насыщенности.
5. **Коэффициент возврата SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. **Задержка и крайний срок** — наложите `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` и `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Используйте преобразования Grafana для улучшения изображений `min_over_time`, чтобы получить возможность планировать абсолютную границу, например:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Манские заказы (всего 1 час)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Сигналы тревоги

- **Успешный SLA  0**
  - Сеуил: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Действие: проверьте манифесты управления для подтверждения оттока поставщиков.
- **завершение стр.95 > превышение срока**
  - Сеуил: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: Проверка достоверности поставщиков до наступления крайних сроков; предусматривающий переназначения.

### Примеры правил Prometheus

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
          summary: "SLA de réplication SoraFS sous la cible"
          description: "Le ratio de succès SLA est resté sous 95% pendant 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de réplication SoraFS au-dessus du seuil"
          description: "Les ordres de réplication en attente ont dépassé le budget de backlog configuré."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordres de réplication SoraFS expirés"
          description: "Au moins un ordre de réplication a expiré au cours des cinq dernières minutes."
```

## Рабочий процесс сортировки

1. **Идентификатор причины**
   - Если соглашения об уровне обслуживания дополняются тем, что резерв остается невыполненным, концентрируется анализ производительности поставщиков (проверка PoR, позднее завершение).
   - Если задержка увеличивается с помощью конюшен, инспектор по допуску (`/v1/sorafs/pin/*`) для подтверждения деклараций и внимания на одобрении совета.
2. **Действия поставщиков услуг**
   - Exécuter `iroha app sorafs providers list` и проверка того, что емкости сообщают о необходимости репликации.
   - Проверка датчиков `torii_sorafs_capacity_*` для подтверждения предоставления GiB и успешного PoR.
3. **Переназначение репликации**
   - Выполнение новых заказов через `sorafs_manifest_stub capacity replication-order` или в случае невыполненной работы (`stat="avg"`) происходит в течение 5 эпох (l'empaquetage Manifest/CAR использует `iroha app sorafs toolkit pack`).
   - Уведомление об управлении, если псевдонимы не связаны с привязками манифестных действий (без внимания `torii_sorafs_registry_aliases_total`).
4. **Документация результатов**
   - Отправитель заметок об инцидентах в журнале операций SoraFS с временными метками и дайджестами заявленных проблем.
   - В первый день работы с Runbook и новыми режимами проверки или информационными панелями, которые представляют собой введение.

## План развертывания

Выполните эту процедуру для этапов активации или продолжения политического кэша псевдонимов в производстве:1. **Подготовка конфигурации**
   - В течение дня `torii.sorafs_alias_cache` в `iroha_config` (пользователь -> фактический) с TTL и условиями соглашения: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` и `governance_grace`. Les valeurs par défaut корреспондент политической жизни `docs/source/sorafs_alias_policy.md`.
   - Заливайте SDK, распространяйте значения мемов через символы конфигурации (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` в привязках Rust/NAPI/Python) к клиентскому приложению, соответствующему шлюзу.
2. **Прогон и постановка**
   - Внедрение изменений конфигурации в промежуточном кластере для обновления топологии производства.
   - Exécuter `cargo xtask sorafs-pin-fixtures` для подтверждения того, что канонические светильники d'alias decodent и шрифт toujours un round-trip; Всякое расхождение подразумевает дрейф вверх по течению в качестве исправления в премьер-министре.
   - Используйте конечные точки `/v1/sorafs/pin/{digest}` и `/v1/sorafs/aliases` с доступными синтетическими продуктами, свежими, обновленными окнами, истекшими и окончательно истекшими. Проверяйте коды HTTP, заголовки (`Sora-Proof-Status`, `Retry-After`, `Warning`) и поля JSON для корпуса, содержащие Runbook.
3. **Актив в производстве**
   - Разверните новую подвеску конфигурации для стандартного отверстия для изменения. Применив отмену Torii, вы можете изменить SDK шлюзов/сервисов, чтобы подтвердить новую политику в журналах.
   - Импортер `docs/source/grafana_sorafs_pin_registry.json` в Grafana (или в течение дня на существующих информационных панелях) и обновляет панели кэша псевдонимов в пространстве NOC.
4. **Проверка после развертывания**
   - Подвеска Surveiller `torii_sorafs_alias_cache_refresh_total` и `torii_sorafs_alias_cache_age_seconds` 30 минут. Les pics dans les courbes `error`/`expired` doivent corréler avec les fenêtres de update; Невнимательность круассана означает, что операторы проверяют превентивные меры и защиту поставщиков перед продолжением.
   - Подтверждение того, что журналы Côté Client montrent les Mêmes Decisions de Politique (les SDKs remontent des erreurs quand la preuve est périmée ou expirée). Отсутствие предупреждений у клиента зависит от неправильной конфигурации.
5. **Резервный вариант**
   - Если псевдоним будет замедлен и в результате обновления произойдет замедление, временно отключите политику и увеличьте `refresh_window` и `positive_ttl` в конфигурации, можно перераспределить. Гардер `hard_expiry` неповреждён, чтобы предотвратить преждевременные изменения, которые могут возникнуть в будущем.
   - Возврат к предыдущей конфигурации в ресторане с моментальным снимком `iroha_config`, предшествующим телеметрии, продолжит отображение счетов `error`, что может привести к происшествию для отслеживания задержек создания псевдонимов.

## Материалы лжи

- `docs/source/sorafs/pin_registry_plan.md` — дорожная карта внедрения и контекста управления.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — операции по складированию, полный реестр playbook.