---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: pin-registry-ops
title: Реестр контактов عمليات
Sidebar_label: Реестр контактов
описание: SoraFS. Реестр контактов. Репликация SLA.
---

:::примечание
یہ صفحہ `docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانی Sphinx دستاویزات ریٹائر نہ ہوں دونوں ورژنز کو ہم آہنگ رکھیں۔
:::

## جائزہ

Можно использовать Runbook или SoraFS, чтобы использовать реестр контактов и репликацию, а также сертификаты SLA (SLA). نگرانی اور ٹرائج کیسے کیا جائے۔ Пространство имен `iroha_torii` для `iroha_torii` и пространство имен `torii_sorafs_*`. ایکسپورٹ ہوتی ہیں۔ Torii в реестре может быть 30 дней, когда вы хотите получить доступ к реестру. Для того чтобы получить доступ к конечным точкам `/v1/sorafs/pin/*`, выберите нужные конечные точки. ہو۔ Установите флажок (`docs/source/grafana_sorafs_pin_registry.json`) Для создания макета Grafana. جو نیچے کے حصوں سے براہ راست میپ ہوتا ہے۔

## میٹرک حوالہ

| میٹرک | Этикетки | وضاحت |
| ----- | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | آن چین проявляется کا انوینٹری لائف سائیکل اسٹیٹ کے مطابق۔ |
| `torii_sorafs_registry_aliases_total` | — | Регистрация в реестре и псевдонимы манифеста. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | заказы на репликацию и отставание в работе |
| `torii_sorafs_replication_backlog_total` | — | سہولت کے لیے манометр جو `pending` заказы, которые можно заказать |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Соглашение об уровне обслуживания: `met` для получения заказов на `missed` для заказа. Срок действия + истечение срока действия `pending` زیر التواء Orders کو ظاہر کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | задержка завершения مجموعی طور پر (выдача до завершения کے درمیان эпох)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | отложенные ордера или свободные окна (крайний срок минус выпущенная эпоха)۔ |

تمام датчики ہر snapshot pull `1m` یا اس سے تیز cadence и образец کرنا چاہیے۔

## Grafana ڈیش بورڈ

Используйте JSON для создания и редактирования файлов JSON. Если вы хотите, чтобы это произошло, вы можете сделать что-то, что вам нужно. درج ہیں۔

1. **Жизненный цикл манифеста** — `torii_sorafs_registry_manifests_total` (`status` в группе).
2. **Тенденция каталога псевдонимов** – `torii_sorafs_registry_aliases_total`.
3. **Упорядочить очередь по статусу** – `torii_sorafs_registry_orders_total` (`status` в группе).
4. **Незавершенные заказы по сравнению с просроченными заказами** – `torii_sorafs_replication_backlog_total` или `torii_sorafs_registry_orders_total{status="expired"}` کو ملا کر saturation دکھاتا ہے۔
5. **Коэффициент успеха SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Задержка и просрочка срока** – `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` или `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}` کو اوورلے کریں۔ Абсолютное провисание пола в Grafana трансформациях и `min_over_time` просмотров.

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Пропущенные заказы (ставка за 1 час)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Пороги оповещения- **Успех по SLA  0**
  - Порог: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Действия: руководство демонстрирует, как поставщики отбиваются от клиентов
- **Завершение стр.95 > среднее отставание от срока**
  - Порог: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Действие: تصدیق کریں کہ сроки поставщика سے پہلے commit کر رہے ہیں؛ переназначения پر غور کریں۔

### مثال Prometheus قواعد

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
          summary: "SoraFS replication SLA ہدف سے کم"
          description: "SLA کامیابی کا تناسب 15 منٹ تک 95% سے کم رہا۔"

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog حد سے اوپر"
          description: "زیر التواء replication orders مقررہ backlog بجٹ سے بڑھ گئے۔"

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders ختم ہو گئیں"
          description: "پچھلے پانچ منٹ میں کم از کم ایک replication order ختم ہوئی۔"
```

## Рабочий процесс сортировки

1. **Всё в порядке**
   - Отсутствие SLA в случае невыполненной работы по провайдерам и отказов в выполнении PoR (сбои PoR, позднее завершение).
   - اگر отставание بڑھے اور пропускает مستحکم ہوں تو вход (`/v1/sorafs/pin/*`) چیک کریں تاکہ проявляет جو کونسل کی منظوری کے منتظر ہیں واضح ہوں۔
2. **Доступные поставщики**
   - `iroha app sorafs providers list` Функция репликации, которая позволяет использовать репликацию ہیں۔
   - `torii_sorafs_capacity_*` измеряет возможность предоставления GiB и успеха PoR в случае необходимости
3. **Репликация и переназначение**
   - جب резерв невыполненной работы (`stat="avg"`) 5 эпох سے نیچے جائے تو `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے Orders جاری کریں (манифест/упаковка автомобиля `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - Псевдонимы могут быть использованы для привязок манифеста и управления, а также для управления (`torii_sorafs_registry_aliases_total` в случае необходимости). کمی)۔
4. **Отличный выбор**
   - Журнал операций SoraFS, временные метки, дайджесты манифестов, заметки об инцидентах и т. д.
   - Доступны режимы сбоя, информационные панели, а также Runbook и другие функции.

## План развертывания

Используйте политику кэширования псевдонимов, чтобы узнать больше о политике кэширования псевдонимов. Варианты:1. **Конфигурация کریں**
   - `iroha_config` или `torii.sorafs_alias_cache` (пользователь -> фактический) Для получения TTL-файлов в Grace Windows можно использовать `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`۔ значения по умолчанию `docs/source/sorafs_alias_policy.md` کی پالیسی سے ملتے ہیں۔
   - Поддержка SDK и уровней конфигурации, а также возможность использования (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` привязок Rust/NAPI/Python) и шлюза принудительного применения клиента. میل کھائے۔
2. **Постановка пробного прогона**
   - Конфигурация, промежуточная установка и развертывание, а также производственная топология и т. д.
   - `cargo xtask sorafs-pin-fixtures` позволяет использовать канонические псевдонимы для декодирования и двустороннего обхода. Несоответствие дрейфа вверх по течению
   - `/v1/sorafs/pin/{digest}` или `/v1/sorafs/aliases` конечные точки для синтетических доказательств или для свежих, обновленных окон, истекших или с жестким истекшим сроком действия. کریں۔ Коды состояния HTTP, заголовки (`Sora-Proof-Status`, `Retry-After`, `Warning`) и поля тела JSON, а также Runbook и проверка проверки.
3. **Производство в производстве**
   - стандартное окно изменений в настройках конфигурации Torii обеспечивает доступ к узлу, который позволяет использовать шлюзы/SDK службы и перезапуск
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana - Дополнительная информация (например, панель мониторинга) или панели обновления кэша псевдонимов. Рабочее пространство NOC
4. **Проверка после развертывания**
   - 30 минут от `torii_sorafs_alias_cache_refresh_total` до `torii_sorafs_alias_cache_age_seconds`. `error`/`expired` кривые, пики, обновление окон, коррелят, ہونا چاہیے؛ Воспользуйтесь услугами операторов и доказательствами псевдонимов, а также поставщиками услуг, которые помогут вам.
   - Журналы на стороне клиента, а также решения по политике и безопасности (SDK устарели, доказательства с истекшим сроком действия и ошибки, которые могут возникнуть)۔ Предупреждения клиента Как настроить конфигурацию
5. **Резервный вариант**
   - Чтобы выпустить псевдоним, обновите окно и нажмите `refresh_window` или `positive_ttl`. Вы можете использовать повторное развертывание или перераспределение ресурсов. `hard_expiry` کو برقرار رکھیں تاکہ واقعی устаревшие доказательства رد ہوتے رہیں۔
   - Снимок телеметрии `error` подсчитывается и используется для моментального снимка `iroha_config`. Возможные задержки генерации псевдонимов и возможные инциденты

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` — дорожная карта реализации в контексте управления.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — операции работника хранилища, а также сборник инструкций по реестру.