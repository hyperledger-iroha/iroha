---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "План показа рекламы поставщиков SoraFS"
---

> Адаптация [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# План показа рекламных объявлений SoraFS

Этот план координации отключения от рекламы, разрешенной поставщиками, теперь
superficie gobernada `ProviderAdvertV1` требуется для рекуперации нескольких источников
де куски. Se centra en tres результатов:

- **Руководство по эксплуатации.** Действия, которые происходят с поставщиками услуг хранения данных
  завершить ворота Antes de Cada.
- **Кобертура телеметрии.** Панели мониторинга и оповещения для наблюдения и эксплуатации в США.
  Чтобы подтвердить, что реклама Red Solo соответствует требованиям.
  для оборудования SDK и инструментов, запланированных к выпуску.

Выпуск будет осуществляться с хитами SF-2b/2c del
[дорожная карта миграции SoraFS](./migration-roadmap) и предположите, что такое политика
прием дель [политика допуска провайдера](./provider-admission-policy) ya esta en
энергия.

## Хронограмма фаз

| Фаза | Вентана (объект) | Транспорт | Действия оператора | Enfoque де наблюдения |
|-------|-----------------|-----------|------------------|-------------------|

## Контрольный список действий

1. **Рекламные объявления.** Список опубликованных и зарегистрированных объявлений:
   - Рута управляющего конверта (`defaults/nexus/sorafs_admission/...` или эквивалент в производстве).
   - `profile_id` и `profile_aliases` объявления.
   - Список возможностей (см. меню `torii_gateway` и `chunk_range_fetch`).
   - Флаг `allow_unknown_capabilities` (требуется, если TLV зарезервированы поставщиком).
2. **Обновите инструменты поставщиков.**
   - Восстановите полезную нагрузку с помощью рекламы издателя провайдера, гарантированно:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` с определенным `max_span`
     - `allow_unknown_capabilities=<true|false>` cuando haya TLVs СМАЗКА
   - Действительно через `/v1/sorafs/providers` и `sorafs_fetch`; las advertencias sobre
     возможности desconocidas deben ser triageadas.
3. **Валидарная готовность для нескольких источников.**
   - Выброс `sorafs_fetch` с `--provider-advert=<path>`; эль CLI сейчас падает
     cuando falta `chunk_range_fetch` и дополнительная информация о возможностях
     desconocidas ignoradas. Захват отчета в формате JSON и архивирование с журналами
     де операции.
4. **Подготовка к ремонту.**
   - Конверты Envia `ProviderAdmissionRenewalV1` за 30 дней до этого.
     принудительное исполнение на шлюзе (R2). При ремонте необходимо сохранить ручку
     canonico и набор возможностей; индивидуальная ставка, конечные точки или метаданные
     камбиар.
5. **Сообщайте об зависимостях оборудования.**
   - Владельцы SDK должны иметь свободные версии, которые выдвигают рекламные объявления.
     Operadores Cuando Los Adverts Шон Речазадос.
   - DevRel объявляет о поэтапном переходе; incluir объединяет информационные панели и ла
     логика тени абахо.
6. **Установка информационных панелей и оповещений.**
   - Импортировать экспорт Grafana и добавить **SoraFS / Provider
     Внедрение** с UID `sorafs-provider-admission`.
   - Убедитесь, что правила оповещения включены в общий канал.
     `sorafs-advert-rollout` в постановке и производстве.

## Телеметрия и информационные панелиСледующие метрики вы ожидаете через `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — cuenta aceptados, rechazados
  и результаты были объявлены. Разговоры включают `missing_envelope`, `unknown_capability`,
  `stale` и `policy_violation`.

Экспорт Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Импортировать архив в хранилище отсеков информационных панелей (`observability/dashboards`)
и актуализируйте только UID источника данных перед публикацией.

Публичная таблица находится на ковре Grafana **SoraFS / Развертывание поставщика** с
UID установлен `sorafs-provider-admission`. Правила оповещения
`sorafs-admission-warn` (предупреждение) y `sorafs-admission-reject` (критический) эстан
предварительные настройки для использования политики уведомлений `sorafs-advert-rollout`;
отрегулируйте эту контактную точку в списке мест назначения и воспользуйтесь редактором
JSON панели мониторинга.

Рекомендуемые панели Grafana:

| Панель | Запрос | Заметки |
|-------|-------|-------|
| **Показатель результатов поступления** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Стековая диаграмма для визуализации: принять, предупредить или отклонить. Предупреждение, когда предупреждение > 0,05 * всего (предупреждение) или отклонение > 0 (критическое). |
| **Коэффициент предупреждений** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Таймсерия одной линии, в которой питание находится в тени пейджера (5%-ная частота предупреждений за 15 минут). |
| **Причины отклонения** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Покажите сортировку Runbook; дополнение включает в себя обязательство по смягчению последствий. |
| **Обновить долг** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Поставщики Indica, которые не сообщили о крайнем сроке обновления; Cruza с кэшем журналов обнаружения. |

Артефакты CLI для руководств по панелям мониторинга:

- `sorafs_fetch --provider-metrics-out` опишите контрадоры `failures`, `successes` y
  `disabled` пор провайдер. Специальный импорт информационных панелей для мониторинга
  пробные прогоны оркестратора перед началом работы с поставщиками в производстве.
- Лос-кампос `chunk_retry_rate` и `provider_failure_rate` отчета JSON
  повторное регулирование или количество устаревших полезных нагрузок, которые были получены ранее
  де прием.

### Макет приборной панели Grafana

Публичная видимость на специальной доске — **SoraFS Допуск поставщика
Развертывание** (`sorafs-provider-admission`) — bajo **SoraFS / Развертывание поставщика**
с каноническими идентификаторами панели:

- Панель 1 — *Частота исходов госпитализации* (площадь сложения, единицы измерения «операций/мин»).
- Панель 2 — *Коэффициент предупреждения* (одиночная серия), выдача выражения
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Панель 3 — *Причины отклонения* (серия временной агрегации по `reason`), ordenada por
  `rate(...[5m])`.
- Панель 4 — *Обновить долг* (статистика), отобразить запрос предыдущей таблицы y
  было сообщено о сроках обновления дополнительных объявлений о миграции.

Скопируйте (или создайте) файл JSON в хранилище информационных панелей инфраструктуры в
`observability/dashboards/sorafs_provider_admission.json`, теперь актуализируемся соло
UID источника данных; идентификаторы панелей и правила оповещений, которые ссылаются на
los runbooks de abajo, asi que evita renumerarlos sin revisar this documentacion.Для удобства репозитория можно включить определение информационной панели.
ссылка на `docs/source/grafana_sorafs_admission.json`; Копиала на твоем ковре
Grafana, если вам нужен пункт, отвечающий за местные проверки.

### Правила оповещений Prometheus

Agrega el siguiente grupo de reglas a
`observability/prometheus/sorafs_admission.rules.yml` (создание архива, если это
начальная группа правил SoraFS) и включена в конфигурацию
Prometheus. Reemplaza `<pagerduty>` с этикетом реального ознакомления для вас
вращение по вызову.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Эекута `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
перед тем как выполнить задание, чтобы гарантировать, что синтаксис пройдет `promtool check rules`.

## Матрица развертывания

| Характеристики рекламы | Р0 | Р1 | Р2 | Р3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` присутствует, псевдонимы canonicos, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Уход за возможностями `chunk_range_fetch` | ⚠️ Предупреждать (захват + телеметрия) | ⚠️ Предупреждать | ❌ Отклонить (`reason="missing_capability"`) | ❌ Отклонить |
| TLV возможности не указаны без `allow_unknown_capabilities=true` | ✅ | ⚠️ Предупреждаем (`reason="unknown_capability"`) | ❌ Отклонить | ❌ Отклонить |
| `refresh_deadline` истек | ❌ Отклонить | ❌ Отклонить | ❌ Отклонить | ❌ Отклонить |
| `signature_strict=false` (диагностические приборы) | ✅ (соло десарролло) | ⚠️ Предупреждать | ⚠️ Предупреждать | ❌ Отклонить |

Все часы в США по UTC. Законы о принудительном исполнении будут отражены в процессе миграции
бухгалтерская книга и no se moveran sin un voto del Council; cualquier cambio требует актуализации
этот архив и бухгалтерская книга в пиар-сообщении.

> **Примечание о реализации:** R1 представляет серию `result="warn"` ru
> `torii_sorafs_admission_total`. El patch de ingesta Torii, который объединяет новое
> Этикет должен быть единым с картами телеметрии SF-2; Hasta que llegue,

## Коммуникация и управление инцидентами

- **Почтовое сообщение в штате**. DevRel рассылает краткие сведения о возобновлении работы
  Прием, объявления, ожидаемые сроки и ближайшие сроки.
- **Ответ на инцидент.** Если оповещения `reject` активированы, дежурный:
  1. Восстановите атаку с помощью объявления Torii (`/v1/sorafs/providers`).
  2. Повторно выполните проверку объявления в конвейере поставщика и сравните его.
     `/v1/sorafs/providers` для воспроизведения ошибки.
  3. Координируйте ротацию рекламы с поставщиком перед последующим обновлением.
     срок.
- **Связывание камбий.** Нет подходящих вариантов схемы возможностей.
  во время R1/R2 перед началом развертывания; los Trials GREASE Дебен
  программирование в течение всего сезона обслуживания и регистрации на эл.
  миграционную книгу.

## Ссылки

- [SoraFS Протокол узла/клиента](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Политика приема поставщиков] (./provider-admission-policy)
- [Дорожная карта миграции](./migration-roadmap)
- [Расширения нескольких источников рекламы поставщика] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)