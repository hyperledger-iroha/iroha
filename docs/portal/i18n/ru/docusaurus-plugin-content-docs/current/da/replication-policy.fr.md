---
lang: ru
direction: ltr
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Источник канонический
Reflete `docs/source/da/replication_policy.md`. Gardez les deux version ru
:::

# Политика репликации Доступность данных (DA-4)

_Статут: В работе -- Ответственные: Рабочая группа по базовому протоколу / группа хранения данных / SRE_

Конвейер приема DA-приложений, поддерживающий объекты хранения
определение класса больших двоичных объектов в `roadmap.md` (рабочий поток
ДА-4). Torii отказать в хранении конвертов по номиналу
звонящий, который не является корреспондентом в соответствии с политической конфигурацией, гарантируя, что
chaque noeud validateur/stockage retient le nombre requis d'epoques et de
реплики без зависимости от намерения.

## Политика по умолчанию

| Класс двоичных объектов | Удержание горячей | Удержание холода | Реквизиты для реплик | Класс хранения | Теги управления |
|---------------|----------------|----------------|-------------------|--------------------|--------------------|
| `taikai_segment` | 24 часа | 14 дней | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 часов | 7 дней | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 часов | 180 дней | 3 | `cold` | `da.governance` |
| _Default (показывает другие классы)_ | 6 часов | 30 дней | 3 | `warm` | `da.default` |

Эти значения являются целостными в `torii.da_ingest.replication_policy` и аппликациях
реклама материалов `/v2/da/ingest`. Torii перезаписывает файлы манифеста с файлом
профиль удержания навязывает и предупреждает четырех звонящих
des valeurs incoherentes, поскольку операторы обнаружения SDK устарели.

### Занятия по доступности Тайкай

Манифесты маршрутизации Taikai (`taikai.trm`) объявлены `availability_class`
(`hot`, `warm`, или `cold`). Torii аппликация корреспондента в политической жизни
разбивка на части, которые могут помочь операторам корректировать количество копий по номиналу
поток без редактирования глобальной таблицы. По умолчанию:

| Класс доступности | Удержание горячей | Удержание холода | Реквизиты для реплик | Класс хранения | Теги управления |
|---------|---------------|----------------|-------------------|--------------------|--------------------|
| `hot` | 24 часа | 14 дней | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 часов | 30 дней | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 час | 180 дней | 3 | `cold` | `da.taikai.archive` |

Подсказки возвращаются к `hot`, чтобы сохранить прямые трансляции
политика плюс сильная сторона. Замените настройки по умолчанию через
`torii.da_ingest.replication_policy.taikai_availability`, если вы используете это значение
des cibles Differentes.

## Конфигурация

Политика в нашей стране `torii.da_ingest.replication_policy` и раскрывает шаблон
*по умолчанию* плюс таблица переопределений по классу. Классовые идентификаторы
без учета регистра и принимает `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact` или `custom:<u16>` для расширений, одобренных по умолчанию.
управление. Принимаются классы хранения `hot`, `warm` или `cold`.```toml
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

Не трогайте блок, чтобы использовать настройки по умолчанию. Налей в дурку
класс, дайте корреспонденту возможность переопределить время; устройство смены заливки la base pour de
новые классы, editez `default_retention`.

Классы недоступности Taikai могут быть платными и независимыми через
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

## Семантика принудительного исполнения

- Torii заменяет `RetentionPolicy`, используемый пользователем в профиле.
  наложить предварительные дробления или выбросы манифеста.
- Лес манифестирует предварительные условия, которые объявляют разные профили удержания
  sont rejetes avec `400 schema mismatch`, пока клиенты не устареют
  puissent pas affaiblir le contrat.
- Запись события переопределения (`blob_class`, политика в отношении посещаемости)
  для подтверждения того, что абоненты не соответствуют подвеске развертывания.

Voir [План приема данных о доступности] (ingest-plan.md) (контрольный список проверки)
le ворота пропущены в течение дня, чтобы обеспечить соблюдение условий хранения.

## Рабочий процесс повторной репликации (suivi DA-4)

«Принуждение к удержанию» — это не премьерный этап. Les операторы doivent
aussi prouver que les Manifests live et les ordres de replication restent
Выровняйте политику с настройкой до того, как SoraFS можно повторно реплицировать BLOB-объекты.
не соответствует автоматике.

1. **Наблюдение за дрейфом.** Torii emet
   `overriding DA retention policy to match configured network baseline` сколько угодно
   un caller soumet des valeurs de удержания устарел. Ассоциация в журнале с
   телеметрия `torii_sorafs_replication_*` для устранения нехватки реплик
   или перераспределение замедляется.
2. **Различие намерений и реплик в реальном времени.** Используйте новый помощник для аудита:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Команда зарядки `torii.da_ingest.replication_policy` в зависимости от конфигурации
   Фурни, декодируйте манифест пакета (JSON или Norito) и другие параметры ассоциации.
   полезные нагрузки `ReplicationOrderV1` в виде дайджеста манифеста. Сигнал возобновления
   двойные условия:

   - `policy_mismatch` - профиль хранения манифеста расходится с профилем
     наложить (ceci ne devrait jamais прибытие sauf si Torii - неправильная настройка).
   - `replica_shortfall` - порядок репликации в реальном времени требует моих реплик
     que `RetentionPolicy.required_replicas` или четыре месяца назначений, которые
     это кабель.

   Ненулевой статус вылета, указывающий на нехватку активных средств в ближайшее время
   l'автоматизация CI/немедленный пейджер по вызову. Жоаньез ле раппорт JSON
   пакет `docs/examples/da_manifest_review_template.md` для голосов ваших
   Парламент.
3. **Отключить повторную репликацию.** Когда аудит сигнализирует о недостаточности,
   emettez un nouveau `ReplicationOrderV1` с помощью правительственных декретов
   dans [SoraFS рынок емкости хранения](../sorafs/storage-capacity-marketplace.md)
   et relancez l'audit просто конвергенция множества реплик. Для переопределений
   срочности, Ассоциация CLI с `iroha app da prove-availability` в ближайшее время
   Мощный референтный источник SRE, дайджест мемов и предварительный PDP.Регрессия регрессии в `integration_tests/tests/da/replication_policy.rs`;
политика сохранения не соответствует `/v2/da/ingest` и проверена
то, что манифест восстанавливает, раскрывает профиль, накладывает плутот, который указывает на намерение звонящего.