---
lang: ru
direction: ltr
source: docs/portal/docs/da/replication-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::обратите внимание на Фуэнте каноника
Рефлея `docs/source/da/replication_policy.md`. Mantenga ambas versiones ru
:::

# Политика репликации доступности данных (DA-4)

_Эстадо: В процессе -- Ответственные: Рабочая группа по базовому протоколу / группа хранения данных / SRE_

Трубопровод приема DA теперь является приложением к определенным целям удержания
Cada clase de blob descrita en `roadmap.md` (рабочий поток DA-4). Torii речаза
сохранять конверты для сохранения для звонящего, что не совпало с ла
Конфигурация политики, гарантия того, что каждый номер подтверждения/сохранение информации
Требуемое количество эпох и реплик зависят от намерения эмизора.

## Политика для дефекта

| Клас-де-блоб | Удержание горячей | Удержание холода | Требуются реплики | Класе-де-альмасенамиенто | Таг де гобернанса |
|--------------|---------------|----------------|---------------------|--------------------------|-------------------|
| `taikai_segment` | 24 часа | 14 диам | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 часов | 7 диам | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 часов | 180 диам | 3 | `cold` | `da.governance` |
| _По умолчанию (все классы)_ | 6 часов | 30 диам | 3 | `warm` | `da.default` |

Это значение установлено в `torii.da_ingest.replication_policy` и используется в приложении.
todas las solicitudes `/v2/da/ingest`. Torii переписать манифесты с файлом Perfil de
удержание импульсов и реклама, когда звонящие переходят между ценностями
нет совпадений, когда операторы обнаруживают неактуализированные SDK.

### Классы диспонибилидад Тайкай

Лос-манифесты enrutamiento Taikai (`taikai.trm`) объявлены
`availability_class` (`hot`, `warm`, o `cold`). Torii применение политики
корреспондент перед разделением на фрагменты, чтобы операторы могли их увеличить
Контейнер реплик для потока без редактирования глобальной таблицы. По умолчанию:

| Класс диспонибилидад | Удержание горячей | Удержание холода | Требуются реплики | Класе-де-альмасенамиенто | Таг де гобернанса |
|-------------------------|---------------|----------------|---------------------|-------------------------|-------------------|
| `hot` | 24 часа | 14 диам | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 часов | 30 диам | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 час | 180 диам | 3 | `cold` | `da.taikai.archive` |

Las pistas faltantes usan `hot` из-за дефекта, который необходим для передачи в естественных условиях
возобновить политику, если она будет продолжаться. Собрать настройки по умолчанию через
`torii.da_ingest.replication_policy.taikai_availability`, если вы красные США, цели
разные.

## Конфигурация

La politica vive bajo `torii.da_ingest.replication_policy` и демонстрация шаблона
*по умолчанию* это список переопределений. Los identificadores de class no.
son sensibles a mayus/minus y aceptan `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact` или `custom:<u16>` для расширений, допустимых для правительства.
Классы замены будут приняты `hot`, `warm`, или `cold`.

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
```Используйте блок нетронутым, чтобы использовать списки по умолчанию. Пара эндуресер
в этом случае произойдет переопределение корреспондента; Чтобы сделать новую базу
классы, редактируйте `default_retention`.

Классы диспонибилидад Тайкай могут быть записаны в независимой форме
через `torii.da_ingest.replication_policy.taikai_availability`:

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

- Torii повторно заменить `RetentionPolicy` для пользователя с профилем
  импульс перед разделением или выбросом манифестов.
- Лос манифестирует предубеждения, которые объявляются особыми по своему усмотрению.
  rechazan con `400 schema mismatch`, чтобы устаревшие клиенты не могли быть использованы
  debilitar el contrato.
- Событие отмены регистрации (`blob_class`, политика enviada vs esperada)
  для вызывающих абонентов не требуется соответствие во время развертывания.

Версия [План получения данных о доступности] (ingest-plan.md) (контрольный список проверки)
Чтобы реализовать ворота, необходимо обеспечить соблюдение режима удержания.

## Flujo de re-replicacion (последний DA-4)

Принуждение к удержанию — это только первый шаг. Лос-операторы также должны работать
Проверь, что лос проявляется в естественных условиях и порядок репликации сохраняется
alineados с политической конфигурацией для SoraFS можно повторно создать репликацию больших двоичных объектов
автоматическое накопление форм.

1. **Видимый эль-дрифт.** Torii излучает
   `overriding DA retention policy to match configured network baseline` когда-либо
   Звонящий отправляет деактуализированные значения сохранения. Empareje ese log con
   телеметрия `torii_sorafs_replication_*` для обнаружения ошибок в репликах
   o перераспределяет деморадос.
2. **Разница между намерениями и репликами in vivo.** Используйте новый помощник аудитории:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Командир автомобиля `torii.da_ingest.replication_policy` из конфигурации
   проверка, декодированный манифест (JSON или Norito) и дополнительная обработка
   полезные нагрузки `ReplicationOrderV1` для дайджеста манифеста. Возобновленная марка дос
   условия:

   - `policy_mismatch` - ошибка сохранения манифеста расходится
     политика импуэста (это не вызывает сомнений, что Torii - это плохо
     настроено).
   - `replica_shortfall` - порядок репликации en vivo solicita menos реплики
     que `RetentionPolicy.required_replicas` или entrega menos asignaciones que su
     объективо.

   Un status de salida не указывает на то, что он неактивен для автоматизации.
   CI/дежурный вызов может быть открыт сразу. Дополнение к отчету JSON в пакете
   `docs/examples/da_manifest_review_template.md` для голосования в Парламенте.
3. **Откажитесь от повторной репликации.** Когда аудитория сообщит об ошибке, выпустите один
   nueva `ReplicationOrderV1` через las herramientas de gobernanza descritas en
   [SoraFS рынок емкости хранения](../sorafs/storage-capacity-marketplace.md)
   и вы выдвинулись из аудитории, где был конвертирован набор реплик. Пара
   отменяет аварийную ситуацию, включает в себя вызов CLI с `iroha app da prove-availability`
   для того, чтобы SRE могли ссылаться на дайджест ошибок и доказательства PDP.La cobertura de regresion vive en
`integration_tests/tests/da/replication_policy.rs`; la suite envia una politica de
сохранение не совпадает с `/v2/da/ingest` и проверка того, что получен манифест
Покажите, что это нарушение, и укажите намерение звонящего.