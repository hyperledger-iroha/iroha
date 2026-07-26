---
lang: ru
direction: ltr
source: docs/portal/docs/da/replication-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание Fonte canonica
Эспелья `docs/source/da/replication_policy.md`. Мантенья как дуа-версоэс
:::

# Политика репликации доступности данных (DA-4)

_Статус: В процессе -- Ответственные: Рабочая группа по базовому протоколу / Группа хранения данных / SRE_

Конвейер приема DA Agora Applica Metas Deterministicas de Retencao Para Cada
класс описания большого двоичного объекта в `roadmap.md` (рабочий поток DA-4). Torii упорный отказ
конверты со скрытыми телефонами, которые звонят по политическим вопросам
конфигурация, гарантия того, что каждый узел проверит/зафиксирует сохранение или номер
Требуемые эпохи и реплики зависят от намерения эмиссара.

## Политика падрао

| Класс двоичных объектов | Ретенсао горячая | Ретенцао холодный | Требуются реплики | Класс вооружения | Таг де губернатора |
|---------------|---------------|----------------|-----|-------------------------|-------------------|
| `taikai_segment` | 24 часа | 14 диам | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 часов | 7 диам | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 часов | 180 диам | 3 | `cold` | `da.governance` |
| _Default (все как классы demais)_ | 6 часов | 30 диам | 3 | `warm` | `da.default` |

Они важны для внедрения в `torii.da_ingest.replication_policy` и приложениях
todas в качестве материалов `/v1/da/ingest`. Torii повторно заблокировать манифесты с помощью perfil
de retencao imposto e emite um alert quando callers fornecem valores divergentes
для того, чтобы операторы обнаруживали устаревшие SDK.

### Занятия по развитию тайкай

В манифестах Roteamento Taikai (`taikai.trm`) указано `availability_class`.
(`hot`, `warm`, или `cold`). Torii приложение к политическому корреспонденту раньше
фрагментирование для того, чтобы операторы могли заразить реплики в потоке
отредактируйте глобальную таблицу. По умолчанию:

| Класс недоступности | Ретенсао горячая | Ретенцао холодный | Требуются реплики | Класс вооружения | Таг де губернатора |
|---------------------------|--------------|----------------|---------------------|----------|-------------------|
| `hot` | 24 часа | 14 диам | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 часов | 30 диам | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 час | 180 диам | 3 | `cold` | `da.taikai.archive` |

Подсказки, которые мы используем `hot` для того, чтобы передать их в живую
политика больше всего. Заменить настройки по умолчанию через
`torii.da_ingest.replication_policy.taikai_availability`, если вы хотите использовать его
все разные.

## Настройка

Политика vive sob `torii.da_ingest.replication_policy` и шаблон экспонирования
*по умолчанию* обычно является массивом переопределений класса. Идентификаторы класса НАО
дифференциалы maiusculas/minusculas и aceitam `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact` или `custom:<u16>` для расширенных разрешений для управления.
Классы вооружения, соответствующие `hot`, `warm` или `cold`.

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
```Используйте неповрежденный блок для просмотра с настройками по умолчанию. Пара эндуресер ума
класс, настроить или переопределить корреспондента; для создания базовых классов,
отредактируйте `default_retention`.

Классы неспособности Тайкай могут стать независимыми специалистами
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

- Torii заменить или `RetentionPolicy` fornecido pelo usuario pelo perfil imposto
  перед тем, как разбить или выпустить манифест.
- Демонстрирует предубеждения о том, что они объявляют о недопустимости удержания расхождений в Сан-Франциско.
  rejeitados com `400 schema mismatch` для устаревших клиентов, которые у нас есть
  enfraquecer o contrato.
- Событие отмены и входа в систему (`blob_class`, политика отправлена против успеха)
  Для экспорта вызывающих абонентов это не соответствует во время развертывания.

Veja [План приема данных о доступности] (ingest-plan.md) (контрольный список проверки), пункт
o Gate atualizado cobrindo принудительного исполнения удержания.

## Рабочий процесс повторного копирования (последний DA-4)

О принуждении к удержанию и отсрочке или первом пропуске. Operadores tambem devem
Проверяй, что воплощаются в жизнь и постоянные порядки репликации в политике
настроить для того, чтобы SoraFS можно было повторно реплицировать большие двоичные объекты для соответствия форме
автоматика.

1. **Наблюдайте за дрейфом.** Torii излучает
   `overriding DA retention policy to match configured network baseline` когда это
   гм вызывающий абонент имеет значение неудовлетворительного удержания. Объединить esse log com a
   телеметрия `torii_sorafs_replication_*` для обнаружения ошибок или реплик
   перераспределяет атрасадос.
2. **Различие намерения и реплик в реальном времени.** Используйте новый помощник аудитории:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Команда Carrega `torii.da_ingest.replication_policy` для настройки
   fornecida, декодированный манифест (JSON или Norito), и дополнительное совпадение фаз
   полезные нагрузки `ReplicationOrderV1` для дайджеста манифеста. O резюме Sinaliza Duas
   кондико:

   - `policy_mismatch` - о сохранении информации о расхождении в политике
     неправильно (это не означает, что Torii была неправильно настроена).
   - `replica_shortfall` - порядок репликации в реальном времени, требующий меньших реплик.
     que `RetentionPolicy.required_replicas` или fornece menos atribuicoes do que
     о альво.

   UM status desayda nao null indica um нехватка средств, чтобы автоматически сделать это
   CI/по вызову можно открыть немедленно. Приложение или связка JSON в пакете
   `docs/examples/da_manifest_review_template.md` для голосования в Парламенте.
3. **Откажитесь от повторной репликации.** Когда аудитория сообщает о нехватке, эмитата гм
   Ново `ReplicationOrderV1` через как описательные устройства управления
   [SoraFS рынок емкости хранения](../sorafs/storage-capacity-marketplace.md)
   Он поехал в новую аудиторию и съел набор реплик, которые были преобразованы. Параметр переопределяет
   В случае чрезвычайной ситуации воспользуйтесь CLI с помощью `iroha app da prove-availability` для
   какие SRE могут ссылаться на обзор материалов и доказательства PDP.

Регрессия в жизни в `integration_tests/tests/da/replication_policy.rs`;
пакет политики сохранения расходящихся средств для `/v1/da/ingest` и проверки
что бы вы ни заявили, что разоблачили или наложили на себя все намерения звонящего.