---
lang: he
direction: rtl
source: docs/portal/docs/da/replication-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/da/replication_policy.md`. Держите обе версии
:::

# Политика репликации זמינות נתונים (DA-4)

_סטאטוס: В работе — סמלים: Core Protocol WG / Storage Team / SRE_

DA ingest pipeline теперь применяет детерминированные цели שמירה для каждого
כתם класса, описанного в `roadmap.md` (זרם עבודה DA-4). Torii отказывается
сохранять מעטפות שמירה, предоставленные מתקשר, если они не совпадают с
настроенной политикой, гарантируя, что каждый валидатор/узел хранения удерживает
необходимое число эпох и реплик без опоры на намерения отправителя.

## Политика по умолчанию

| כתם Класс | שימור חם | שימור קור | Требуемые реплики | Класс хранения | תג ממשל |
|------------|---------------|----------------|------------------|----------------|----------------|
| `taikai_segment` | 24 שעות | 14 ימים | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 שעות | 7 ימים | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 שעות | 180 ימים | 3 | `cold` | `da.governance` |
| _ברירת מחדל (все остальные классы)_ | 6 שעות | 30 ימים | 3 | `warm` | `da.default` |

Эти значения встроены в `torii.da_ingest.replication_policy` и применяются ко
всем `/v1/da/ingest` отправкам. Torii переписывает מניפסטים с применнным
שימור פרופילים и выдает предупреждение, если מתקשרים передают несоответствующие
значения, чтобы операторы могли выявлять устаревшие SDK.

### Классы доступности Taikai

מניפסטים לניתוב Taikai (`taikai.trm`) объявляют `availability_class`
(`hot`, `warm`, או `cold`). Torii מוצעת עבור chunking,
чтобы операторы могли масштабировать число реплик по stream без редактирования
глобальной таблицы. ברירות מחדל:

| Класс доступности | שימור חם | שימור קור | Требуемые реплики | Класс хранения | תג ממשל |
|--------------------|----------------|----------------|----------------|----------------|----------------|
| `hot` | 24 שעות | 14 ימים | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 שעות | 30 ימים | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 שעה | 180 ימים | 3 | `cold` | `da.taikai.archive` |

Если подсказок нет, используется `hot`, чтобы שידור חי удерживали самый
строгий профиль. ברירת המחדל של Переопределяйте через
`torii.da_ingest.replication_policy.taikai_availability`, если сеть использует
другие цели.

## Конфигурация

Политика находится под `torii.da_ingest.replication_policy` и предоставляет
*ברירת מחדל* шаблон плюс массив לעקוף для каждого класса. Идентификаторы классов
регистронезависимы и принимают `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, גם `custom:<u16>` עבור расширений, одобренных управлением.
Классы хранения принимают `hot`, `warm`, או `cold`.

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

Оставьте блок без изменений, чтобы использовать ברירות מחדל выше. Чтобы ужесточить
класс, обновите соответствующий ביטול; чтобы изменить базу для новых классов,
отредактируйте `default_retention`.שיעורי זמינות טאיקאי можно переопределять отдельно через
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

## אכיפה של סמן

- Torii заменяет пользовательский `RetentionPolicy` עבור принудительный профиль
  перед chunking или выпуском מניפסט.
- מניפסטים של Предсобранные, которые декларируют несовпадающий retention профиль,
  отклоняются с `400 schema mismatch`, чтобы устаревшие клиенты не могли ослабить
  контракт.
- Каждое событие לעקוף את логируется (`blob_class`, отправленная политика לעומת
  ожидаемая), чтобы выявлять מתקשרים שאינם תואמים во время השקה.

Смотрите [תוכנית הטמעת זמינות נתונים](ingest-plan.md) (רשימת אימות) для
обновленного שער, покрывающего שימור אכיפה.

## זרימת עבודה повторной репликации (המשך DA-4)

שימור אכיפה — лишь первый шаг. Операторы также должны доказать, что בשידור חי
מתבטאת בפקודות שכפול остаются согласованными с настроенной политикой,
чтобы SoraFS мог автоматически לשכפל מחדש כתמים несоответствующие.

1. **Следите за סחף.** Torii пишет
   `overriding DA retention policy to match configured network baseline` когда
   מתקשר отправляет устаревшие שימור значения. Сопоставляйте этот лог с
   телеметрией `torii_sorafs_replication_*`, чтобы обнаружить shortfalls реплик
   или задержанные פריסות מחדש.
2. **Sравните intent и העתקים חיים.** Используйте новый audit helper:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Команда загружает `torii.da_ingest.replication_policy` из конфигурации,
   декодирует каждый מניפסט (JSON или Norito), и опционально сопоставляет
   מטענים `ReplicationOrderV1` по תקציר מניפסט. Итог помечает две ситуации:

   - `policy_mismatch` - מניפסט שמירה профиль расходится с принудительным
     профилем (такого не должно быть, если Torii настроен корректно).
   - `replica_shortfall` - סדר שכפול חי запрашивает меньше реплик, чем
     `RetentionPolicy.required_replicas`, или выдает меньше назначений, чем цель.

   Ненулевой код выхода означает активный חסרון, чтобы CI/on-call автоматизация
   могла немедленно пейджить. Приложите JSON отчет к пакету
   `docs/examples/da_manifest_review_template.md` עבור голосований парламента.
3. **לשכפל מחדש.** Если аудит сообщает о shortfall, выпустите
   новый `ReplicationOrderV1` через инструменты управления, описанные в
   [שוק קיבולת אחסון SoraFS](../sorafs/storage-capacity-marketplace.md),
   и повторяйте аудит, пока набор реплик не сойдется. Для עוקף חירום
   сопоставьте CLI вывод с `iroha app da prove-availability`, чтобы SRE могли ссылаться
   на тот же digest и PDP ראיות.

כיסוי רגרסיה находится в `integration_tests/tests/da/replication_policy.rs`;
suite отправляет несовпадающую политику שימור в `/v1/da/ingest` и проверяет,
что полученный מניפסט показывает принудительный профиль, מתקשר ללא כוונות.