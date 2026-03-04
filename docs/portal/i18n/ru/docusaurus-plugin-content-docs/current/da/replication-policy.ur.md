---
lang: ru
direction: ltr
source: docs/portal/docs/da/replication-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
Используйте синхронизацию для синхронизации
:::

# Политика репликации доступности данных (DA-4)

_حالت: В процессе – Владельцы: Основная рабочая группа по протоколу / Группа хранения данных / SRE_

Конвейер приема DA `roadmap.md` (рабочий поток DA-4) для класса BLOB-объектов
Детерминированные целевые показатели удержания Torii Удержание
конверты, сохраняющиеся, и звонящие, и звонящие, и другие настроенные
policy سے match نہ کریں، تاکہ ہر валидатор/узел хранения مطلوبہ epochs اور
реплики сохраняют намерение отправителя

## Политика по умолчанию

| Класс BLOB-объектов | Горячее сохранение | Удержание холода | Требуемые реплики | Класс хранения | Тег управления |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 часа | 14 дней | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 часов | 7 дней | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 часов | 180 дней | 3 | `cold` | `da.governance` |
| _По умолчанию (все остальные классы)_ | 6 часов | 30 дней | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` Встроенный встроенный ہیں تمام
`/v1/da/ingest` материалы для просмотра Torii профиль принудительного хранения
Вызывает несоответствие значений вызывающих абонентов.
Предупреждение о том, что операторы устарели SDK.

### Классы доступности Тайкай

Манифесты маршрутизации Taikai (`taikai.trm`) и `availability_class` (`hot`, `warm`,
یا `cold`) объявить کرتے ہیں۔ Torii разбиение на фрагменты и политика сопоставления.
Глобальная таблица операторов, редактирование потока, количество реплик
масштаб کر سکیں۔ По умолчанию:

| Класс доступности | Горячее сохранение | Удержание холода | Требуемые реплики | Класс хранения | Тег управления |
|----|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 часа | 14 дней | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 часов | 30 дней | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 час | 180 дней | 3 | `cold` | `da.taikai.archive` |

Отсутствующие подсказки по умолчанию طور پر `hot` رکھتے ہیں تاکہ прямые трансляции سب سے مضبوط
политика сохранения کریں۔ Сеть нацелена на то, чтобы получить доступ к сети
`torii.da_ingest.replication_policy.taikai_availability` — настройки по умолчанию
переопределить

## Конфигурация

یہ policy `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور ایک *default*
Возможность переопределения шаблонов для каждого класса и массива Идентификаторы классов
без учета регистра ہیں اور `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, یا `custom:<u16>` (расширения, одобренные правительством)
کرتے ہیں۔ Классы хранения данных `hot`, `warm`, یا `cold`.

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

Настройки по умолчанию: блокировка, блокировка, блокировка, блокировка, блокировка, блокировка, блокировка, блокировка کسی класс کو затянуть
Вы можете переопределить обновление. Классы и базовые показатели
`default_retention` редактировать کریں۔Классы доступности Taikai можно переопределить, используя
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

## Семантика принудительного применения

- Torii предоставленный пользователем `RetentionPolicy` для принудительного профиля سے замените کرتا ہے
  фрагментация یا манифестная эмиссия سے پہلے۔
- Предварительно созданные манифесты и профиль хранения несоответствий объявляют `400 schema mismatch`.
  Если вы отклоните контракт с устаревшими клиентами и ослабите его, вы можете отказаться от него.
- ہر переопределить журнал событий ہوتا ہے (`blob_class`, отправленная или ожидаемая политика)
  Внедрение и устранение абонентов, не соответствующих требованиям.

Обновлен шлюз کے لئے [План приема данных о доступности] (ingest-plan.md)
(Контрольный список проверки).

## Рабочий процесс повторной репликации (продолжение DA-4)

Принудительное удержание صرف پہلا قدم ہے۔ Операторы کو یہ بھی ثابت کرنا ہوگا کہ в прямом эфире
манифестирует порядок репликации, настроенную политику SoraFS
несовместимые большие двоичные объекты могут повторно реплицироваться

1. **Дрифт پر نظر رکھیں۔** Torii
   `overriding DA retention policy to match configured network baseline` излучает
   Значения устаревшего удержания вызывающего абонента بھیجتا ہے۔ اس log کو
   `torii_sorafs_replication_*` телеметрия может привести к нехватке реплик
   отложенное перераспределение کو پکڑیں۔
2. **Намерение позволяет создавать живые реплики и различаться** и использовать помощник по аудиту:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Команда `torii.da_ingest.replication_policy` нужна для настройки конфигурации.
   манифест (JSON یا Norito) декодирует файл `ReplicationOrderV1`
   полезные нагрузки и дайджест манифеста и совпадение Условия использования флага:

   - `policy_mismatch` — политика сохранения профиля хранения манифеста применяется سے مختلف ہے
     (Torii неправильная конфигурация или ошибка).
   - `replica_shortfall` - порядок живой репликации `RetentionPolicy.required_replicas`
     Несколько реплик и целевые задания и задания.

   Ненулевой статус выхода или дефицит, или CI/дежурство по вызову.
   автоматизация на странице کر سکے۔ Формат JSON
   Пакет `docs/examples/da_manifest_review_template.md` можно прикрепить
   Парламент проголосовал за решение دستیاب ہو۔
3. **Триггер повторной репликации** Для нехватки аудита
   `ReplicationOrderV1` Доступен с помощью инструментов управления.
   [SoraFS рынок емкости хранения](../sorafs/storage-capacity-marketplace.md)
   Если вы хотите, чтобы набор реплик сходился, вам нужно выполнить аудит.
   Аварийное переопределение или выход CLI, `iroha app da prove-availability`,
   Дайджест SRE и дайджест доказательств PDP.

Покрытие регрессии `integration_tests/tests/da/replication_policy.rs` میں ہے؛
suite `/v1/da/ingest` — несоответствующая политика хранения.
Получение манифеста о намерении вызывающего абонента Использование принудительного раскрытия профиля