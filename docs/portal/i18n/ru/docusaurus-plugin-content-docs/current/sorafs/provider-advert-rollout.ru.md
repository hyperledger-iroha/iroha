---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "План внедрения и совместимости провайдеров рекламы SoraFS"
---

> Адаптировано из [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# План развертывания и совместимости провайдеров рекламы SoraFS

Этот план координирует переход от разрешительных провайдеров рекламы к полному
управляемой поверхности `ProviderAdvertV1`, необходимой для выдачи из нескольких источников
куски. Он фокусируется на трех результатах:

- **Руководство оператора.** Пошаговые действия, которые должны выполнять провайдеры хранения.
  выполнить до включения каждого ворота.
- **Покрытие телеметрией.** Дашборды и оповещения, которые Observability и Ops используют,
  Чтобы быть уверенным, что сеть принимает только совместимую рекламу.
  SDK и инструменты могли планировать выпуски.

Соглашение о развертывании с автомобилями SF-2b/2c в
[дорожная карта поездок SoraFS](./migration-roadmap) и предполагает, что политика допускает в
[политика допуска провайдера](./provider-admission-policy) уже действует.

##Таймлайн фаза

| Фаза | Окно (цель) | Поведение | Действия операторов | Фокус наблюдения |
|-------|-----------------|-----------|------------------|-------------------|

## Чеклист оператора

1. **Инвентаризировать объявления.** Перечислите каждое опубликованное объявление и зафиксируйте:
   - Путь к управляющему конверту (`defaults/nexus/sorafs_admission/...` или производственный эквивалент).
   - реклама `profile_id` и `profile_aliases`.
   - Список возможностей (ожидается как минимум `torii_gateway` и `chunk_range_fetch`).
   - Флаг `allow_unknown_capabilities` (обязателен при наличии TLV, зарезервированного поставщиком).
2. **Регенерация с помощью инструментов провайдера.**
   - Пересоберите полезную нагрузку через рекламу издателя, убедившись в:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` с подвеской `max_span`
     - `allow_unknown_capabilities=<true|false>` в наличии СМАЗКА TLV
   - проверьте через `/v2/sorafs/providers` и `sorafs_fetch`; отзыв о
     неизвестных возможностей нужно триажить.
3. **Проверка готовности нескольких источников.**
   - Выполните `sorafs_fetch` с `--provider-advert=<path>`; CLI теперь падает,
     Когда отсутствует `chunk_range_fetch`, и печатает ссылку о
     проигнорированных неизвестных возможностей. Зафиксируйте JSON-отчет и
     архивируйте его с журналами операций.
4. **Подготовка продлений.**
   - Отправьте конверты `ProviderAdmissionRenewalV1` минимум за 30 дней до
     принудительное исполнение шлюза (R2). Продления должны сохранять канонический дескриптор и
     набор возможностей; менять следует только ставку, конечные точки или метаданные.
5. **Общение с зависимыми командами.**
   - Владельцы SDK должны выпустить версию, которая отображает предупреждения операторам.
     при отклонении объявлений.
   - DevRel анонсирует каждую фазу; включайте ссылки на дашборды и логику
     порогов ниже.
6. **Установка информационных панелей и оповещений.**
   - Импортируйте экспорт Grafana и добавьте его в **SoraFS / Provider.
     Внедрение** с UID `sorafs-provider-admission`.
   - Убедитесь, что правила оповещений направлены на общий канал.
     `sorafs-advert-rollout` в постановке и продакшене.

## Телеметрия и дашборды

Следующие метрики уже доступны через `iroha_telemetry`:- `torii_sorafs_admission_total{result,reason}` — счетчики приема, отклонений
  и предупреждения. Включая причины `missing_envelope`, `unknown_capability`, `stale` и
  `policy_violation`.

Grafana экспорт: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Импортируйте файл в общий репозиторий дашбордов (`observability/dashboards`) и
Обновите только источник данных UID перед публикацией.

Дашборд публикуется в почтовом ящике Grafana **SoraFS / Provider Rollout** с
стабильным UID `sorafs-provider-admission`. Правила оповещений
`sorafs-admission-warn` (предупреждение) и `sorafs-admission-reject` (критический)
преднастроены на политику протокола `sorafs-advert-rollout`; меняйте контактный
пункт при преобразовании списка получателей вместо правки JSON дашборда.

Рекомендуемые панели Grafana:

| Панель | Запрос | Заметки |
|-------|-------|-------|
| **Показатель результатов поступления** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Стековая диаграмма для визуализации: принять, предупредить или отклонить. Оповещение при предупреждении > 0,05 * всего (предупреждение) или отклонении > 0 (критическое). |
| **Коэффициент предупреждений** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Однолинейная таймсерия, питающая порог пейджера (5% частота предупреждений в скользящем 15-минутном окне). |
| **Причины отклонения** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Для сортировки в Runbook; прикрепляйте ссылки на шаги по смягчению последствий. |
| **Обновить долг** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Указывает на поставщиков, пропустивших срок обновления; сверяйте с логами обнаружения кэша. |

Референции CLI для ручных дашбордов:

- `sorafs_fetch --provider-metrics-out` записывает счетчики `failures`, `successes` и
  `disabled` у каждого провайдера. Импортируйте в специальные информационные панели, чтобы
  мониторить пробный оркестратор перед переключением поставщиков продукции.
- Поля `chunk_retry_rate` и `provider_failure_rate` в JSON-отчете
  подсвечивают дросселирование или симптомы устаревших полезных нагрузок, которые часто предшествуют
  нарушениям приема.

### Раскладка Grafana дашборда

Наблюдаемость публикует отдельную доску — **SoraFS Допуск провайдера
Развертывание** (`sorafs-provider-admission`) — в **SoraFS / Развертывание поставщика**
соблюдаем стандартные идентификаторы панелей:

- Панель 1 — *Частота поступления* (площадь сложения, единица «опс/мин»).
- Панель 2 — *Коэффициент предупреждения* (одиночная серия), выражение
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Панель 3 — *Причины отклонения* (временной ряд, сгруппированные по `reason`), сортировка по
  `rate(...[5m])`.
- Панель 4 — *Обновить задолженность* (статистика), отметьте запрос из таблицы выше и
  аннотирован срок обновления миграционного журнала.

Скопируйте (или создайте) JSON-скелет в репозитории инфраструктурных дашбордов.
`observability/dashboards/sorafs_provider_admission.json`, затем обновите только
источник данных UID; Идентификаторы панелей и правила оповещений используются в модулях Runbook ниже, поэтому не
перенумеровывайте их без обновления данной документации.

Для удобства репозиторий уже содержит определение справочной информационной панели в
`docs/source/grafana_sorafs_admission.json`; скопируйте его в вашу запись Grafana,
если нужен стартовый вариант для локального тестирования.

### Правила оповещений Prometheusдобавьте группу этих правил в
`observability/prometheus/sorafs_admission.rules.yml` (создайте файл, если это
первую группу правил SoraFS) и подключите ее к конфигурации Prometheus.
Замените `<pagerduty>` на реальную метку маршрутизации для вашей ротации по вызову.

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

Запустите `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
перед отправкой изменений, чтобы убедиться, что синтаксис проходит
`promtool check rules`.

## Матрица совместимости

| Характеристики объявления | Р0 | Р1 | Р2 | Р3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` присутствуют, канонические псевдонимы, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Нет возможности `chunk_range_fetch` | ⚠️ Предупреждать (захват + телеметрия) | ⚠️ Предупреждать | ❌ Отклонить (`reason="missing_capability"`) | ❌ Отклонить |
| TLV неизвестная возможность без `allow_unknown_capabilities=true` | ✅ | ⚠️ Предупреждаем (`reason="unknown_capability"`) | ❌ Отклонить | ❌ Отклонить |
| Истекший `refresh_deadline` | ❌ Отклонить | ❌ Отклонить | ❌ Отклонить | ❌ Отклонить |
| `signature_strict=false` (диагностические приборы) | ✅ (только разработка) | ⚠️ Предупреждать | ⚠️ Предупреждать | ❌ Отклонить |

Все времена указаны в UTC. Даты отражения правоприменения в миграционном регистре и нет
будут изменены без голосования совета; любые изменения требуют обновления этого
файл и реестр в одном PR.

> **Примечание по реализации:** R1 приводит серию `result="warn"` в
> `torii_sorafs_admission_total`. Патч ввода Torii, добавляющий новый ярлык,
> отслеживается вместе с задачами телеметрии SF-2; до его получения воспользуйтесь

## Коммуникация и обработка инцидентов

- **Еженедельная рассылка запроса.** DevRel рассылает краткое резюме метрики.
  прием, текущие предупреждения и ближайшие сроки.
- **Реагирование на инциденты.** Если сработает оповещение `reject`, вызов инженеров:
  1. Забирают проблемную рекламу через Discovery Torii (`/v2/sorafs/providers`).
  2. Повторно провести валидацию объявления в конвейере провайдера и сравнить с
     `/v2/sorafs/providers`, чтобы воспроизвести ошибку.
  3. Координируйте объявление о ротации провайдера до следующего крайнего срока обновления.
- **Заморозка изменений.** Возможности схемы изменений в R1/R2 отсутствуют, если
  комитет раскрутки не одобрит; Испытания GREASE проводятся только еженедельно.
  обслужите окно и исправьте в миграционном журнале.

## Ссылки

- [SoraFS Протокол узла/клиента](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Политика приема поставщиков] (./provider-admission-policy)
- [Дорожная карта миграции](./migration-roadmap)
- [Расширения нескольких источников рекламы поставщика] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)