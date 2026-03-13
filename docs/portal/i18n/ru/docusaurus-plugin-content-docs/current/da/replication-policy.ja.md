---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/da/replication-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bbdb81ae30c934e9fff5609b75337c2d19830533035b6c47372456c8e0ff79dc
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ru
direction: ltr
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает `docs/source/da/replication_policy.md`. Держите обе версии
:::

# Политика репликации Data Availability (DA-4)

_Статус: В работе — Владельцы: Core Protocol WG / Storage Team / SRE_

DA ingest pipeline теперь применяет детерминированные цели retention для каждого
класса blob, описанного в `roadmap.md` (workstream DA-4). Torii отказывается
сохранять retention envelopes, предоставленные caller, если они не совпадают с
настроенной политикой, гарантируя, что каждый валидатор/узел хранения удерживает
необходимое число эпох и реплик без опоры на намерения отправителя.

## Политика по умолчанию

| Класс blob | Hot retention | Cold retention | Требуемые реплики | Класс хранения | Governance tag |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 hours | 7 days | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 hours | 180 days | 3 | `cold` | `da.governance` |
| _Default (все остальные классы)_ | 6 hours | 30 days | 3 | `warm` | `da.default` |

Эти значения встроены в `torii.da_ingest.replication_policy` и применяются ко
всем `/v2/da/ingest` отправкам. Torii переписывает manifests с примененным
профилем retention и выдает предупреждение, если callers передают несоответствующие
значения, чтобы операторы могли выявлять устаревшие SDK.

### Классы доступности Taikai

Taikai routing manifests (`taikai.trm`) объявляют `availability_class`
(`hot`, `warm`, или `cold`). Torii применяет соответствующую политику до chunking,
чтобы операторы могли масштабировать число реплик по stream без редактирования
глобальной таблицы. Defaults:

| Класс доступности | Hot retention | Cold retention | Требуемые реплики | Класс хранения | Governance tag |
|-------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 hours | 14 days | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 hours | 30 days | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hour | 180 days | 3 | `cold` | `da.taikai.archive` |

Если подсказок нет, используется `hot`, чтобы live broadcast удерживали самый
строгий профиль. Переопределяйте defaults через
`torii.da_ingest.replication_policy.taikai_availability`, если сеть использует
другие цели.

## Конфигурация

Политика находится под `torii.da_ingest.replication_policy` и предоставляет
*default* шаблон плюс массив override для каждого класса. Идентификаторы классов
регистронезависимы и принимают `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, либо `custom:<u16>` для расширений, одобренных управлением.
Классы хранения принимают `hot`, `warm`, или `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Оставьте блок без изменений, чтобы использовать defaults выше. Чтобы ужесточить
класс, обновите соответствующий override; чтобы изменить базу для новых классов,
отредактируйте `default_retention`.

Taikai availability classes можно переопределять отдельно через
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Семантика enforcement

- Torii заменяет пользовательский `RetentionPolicy` на принудительный профиль
  перед chunking или выпуском manifest.
- Предсобранные manifests, которые декларируют несовпадающий retention профиль,
  отклоняются с `400 schema mismatch`, чтобы устаревшие клиенты не могли ослабить
  контракт.
- Каждое событие override логируется (`blob_class`, отправленная политика vs
  ожидаемая), чтобы выявлять non-compliant callers во время rollout.

Смотрите [Data Availability Ingest Plan](ingest-plan.md) (Validation checklist) для
обновленного gate, покрывающего enforcement retention.

## Workflow повторной репликации (follow-up DA-4)

Enforcement retention — лишь первый шаг. Операторы также должны доказать, что live
manifests и replication orders остаются согласованными с настроенной политикой,
чтобы SoraFS мог автоматически re-replicate несоответствующие blobs.

1. **Следите за drift.** Torii пишет
   `overriding DA retention policy to match configured network baseline` когда
   caller отправляет устаревшие retention значения. Сопоставляйте этот лог с
   телеметрией `torii_sorafs_replication_*`, чтобы обнаружить shortfalls реплик
   или задержанные redeployments.
2. **Сравните intent и live replicas.** Используйте новый audit helper:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Команда загружает `torii.da_ingest.replication_policy` из конфигурации,
   декодирует каждый manifest (JSON или Norito), и опционально сопоставляет
   payloads `ReplicationOrderV1` по digest manifest. Итог помечает две ситуации:

   - `policy_mismatch` - retention профиль manifest расходится с принудительным
     профилем (такого не должно быть, если Torii настроен корректно).
   - `replica_shortfall` - live replication order запрашивает меньше реплик, чем
     `RetentionPolicy.required_replicas`, или выдает меньше назначений, чем цель.

   Ненулевой код выхода означает активный shortfall, чтобы CI/on-call автоматизация
   могла немедленно пейджить. Приложите JSON отчет к пакету
   `docs/examples/da_manifest_review_template.md` для голосований парламента.
3. **Запустите re-replication.** Если аудит сообщает о shortfall, выпустите
   новый `ReplicationOrderV1` через инструменты управления, описанные в
   [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md),
   и повторяйте аудит, пока набор реплик не сойдется. Для emergency overrides
   сопоставьте CLI вывод с `iroha app da prove-availability`, чтобы SRE могли ссылаться
   на тот же digest и PDP evidence.

Regression coverage находится в `integration_tests/tests/da/replication_policy.rs`;
suite отправляет несовпадающую политику retention в `/v2/da/ingest` и проверяет,
что полученный manifest показывает принудительный профиль, а не intent caller.
