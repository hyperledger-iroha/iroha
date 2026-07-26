---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: «План развертывания рекламы поставщиков SoraFS»
---

> Адаптирован к [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# План развертывания рекламы поставщиков SoraFS

Ce plan coordonne la bascule des Advertisements, разрешающий провайдерам на поверхности
entièrement gouvernée `ProviderAdvertV1` требование для восстановления фрагментов
многоисточниковый. Il se concentre sur trois livrables:

- **Руководство оператора.** Действия, выполняемые поставщиками складских услуг.
  Терминер Авант Чак ворота.
- **Телеметрия.** Панели мониторинга и оповещения о наблюдении и операциях.
  используйте для подтверждения того, что резолюция не принимается, что реклама соответствует.
- **Календарь развертывания.** Указываются даты для возврата конвертов.

Развертывание на жалюзи SF-2b/2c de la
[дорожная карта миграции SoraFS](./migration-roadmap) и предположим, что это политика
входной билет [политика приема провайдера](./provider-admission-policy) уже установлена
вигер.

## Хронология фаз

| Фаза | Фенетр (кабельный) | Одежда | Оператор действий | Наблюдаемость фокуса |
|-------|-----------------|-----------|------------------|-------------------|

## Оператор контрольного списка1. **Инвентаризация объявлений.** Список опубликованных и зарегистрированных объявлений:
   - Chemin de l'envelope gouvernant (`defaults/nexus/sorafs_admission/...` или эквивалент производства).
   - `profile_id` и `profile_aliases` объявления.
   - Список возможностей (например, `torii_gateway` и `chunk_range_fetch`).
   - Флаг `allow_unknown_capabilities` (требуется, сколько TLV-зарезервировано поставщиком).
2. **Регенератор с поставщиком инструментов.**
   - Восстановите полезную нагрузку с помощью вашего издателя рекламы провайдера и гарантируйте:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` с определенным `max_span`
     - `allow_unknown_capabilities=<true|false>`, где присутствуют TLV-СМАЗКИ
   - Валидация через `/v1/sorafs/providers` и `sorafs_fetch` ; предупреждения
     возможности несовместимы с doivent être triés.
3. **Проверка готовности нескольких источников.**
   - Исполнитель `sorafs_fetch` с `--provider-advert=<path>` ; сигнал CLI
     désormais quand `chunk_range_fetch` manque et affiche des alerts for des
     возможности смущают невежественных людей. Сбор данных JSON и архивирование с ним
     журналы операций.
4. **Подготовка к ремонту.**
   - Soumettre des конверты `ProviderAdmissionRenewalV1` за 30 дней до этого
     шлюз исполнения (R2). Les renouvellements doivent консервирует ручку
     канонический и ансамбль возможностей; только ставки, конечные точки или
     la метаданные doivent чейнджер.
5. **Сообщение с зависимыми людьми.**
   - Владельцы SDK могут публиковать версии, которые выставляют дополнительные предупреждения.
     Операторы по рекламе были отвергнуты.
   - DevRel объявляет о смене фазы перехода; включить панели мониторинга
     et la logique de seuil ci-dessous.
6. **Информационные панели и оповещения установщика.**
   - Импортер экспорта Grafana и местонахождение **SoraFS / Поставщик
     Внедрение** с UID `sorafs-provider-admission`.
   - Убедиться в том, что правила оповещения указывают на разделяющийся канал.
     `sorafs-advert-rollout` в постановке и производстве.

## Телеметрия и информационные панели

Следующие метрики были раскрыты через `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — считать принятые, отклоненные
  и предупреждающие сообщения. Причины включают `missing_envelope`, `unknown_capability`,
  `stale` и `policy_violation`.

Экспортировать Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Импортировать документацию в хранилище информационных панелей (`observability/dashboards`)
и получил уникальный уникальный идентификатор источника данных перед публикацией.

Плата опубликована в досье Grafana **SoraFS / Развертывание поставщика** с
I'UID стабильный `sorafs-provider-admission`. Правила бдительности
`sorafs-admission-warn` (предупреждение) и `sorafs-admission-reject` (критический) сонт
предварительно настроенные параметры для использования политики уведомлений `sorafs-advert-rollout` ;
отрегулируйте контактную точку, если измените список пунктов назначения, который нужно изменить
JSON панели управления.

Рекомендации Panneaux Grafana:| Панель | Запрос | Заметки |
|-------|-------|-------|
| **Показатель результатов поступления** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Стековая диаграмма для визуализатора: принять, предупредить или отклонить. Предупреждение, когда предупреждение > 0,05 * общее количество (предупреждение) или отклонение > 0 (критическое). |
| **Коэффициент предупреждений** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Уникальные таймсерии, которые можно получить на каждом пейджере (дополнительное предупреждение: 5 % за 15 минут). |
| **Причины отклонения** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Руководство по сортировке Runbook; Attacher des liens vers les étapes de mitigation. |
| **Обновить долг** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Индикация поставщиков, определяющих крайний срок обновления; croiser с кэшем журналов обнаружения. |

Артефакты CLI для управления панелями мониторинга:

- `sorafs_fetch --provider-metrics-out` écrit des compteurs `failures`, `successes` и др.
  `disabled` номинальный поставщик. Импорт в специальные информационные панели для наблюдения
  пробные прогоны оркестратора перед объединением поставщиков в производство.
- Les champs `chunk_retry_rate` и `provider_failure_rate` du Rapport JSON.
  Меттент и предупреждение об удушении или симптомах устаревшей полезной нагрузки
  сувениры для отказа от приема.

### Страница панели управления Grafana

Observabilité publie un board dédié — **SoraFS Допуск поставщика услуг
Внедрение** (`sorafs-provider-admission`) — sous **SoraFS / Внедрение поставщика**
со следующими идентификаторами канонических панелей:

- Панель 1 — *Частота исходов госпитализации* (площадь с накоплением, единица «операций/мин»).
- Панель 2 — *Коэффициент предупреждения* (одиночная серия), émettant l'expression
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Панель 3 — *Причины отклонения* (перегруппированные временные ряды по `reason`), тройные по номиналу
  `rate(...[5m])`.
- Панель 4 — *Обновление долга* (статистика), ответ на запрос таблицы ci-dessus и др.
  аннотация со сроками обновления дополнительных объявлений в миграционном реестре.

Скопируйте (или создайте) фрагмент JSON в хранилище информационных панелей в инфраструктуре.
`observability/dashboards/sorafs_provider_admission.json`, можно увидеть сегодня
уникальность UID источника данных; Идентификаторы панели и правила оповещения
ссылки на runbooks ci-dessous, évitez donc de les renuméroter sans
Mettre à Jour Cette Documentation.

Для удобства в репозитории указано определение эталонной информационной панели.
`docs/source/grafana_sorafs_admission.json` ; Копирование вашего досье Grafana
si vous avez besoin d'un point de départ для местных испытаний.

### Правила оповещения Prometheus

Ajoutez le groupe de regles suivant à
`observability/prometheus/sorafs_admission.rules.yml` (creez le fichier si c'est
le Premier Groupe de Règles SoraFS) и включает в себя вашу конфигурацию
Prometheus. Замените `<pagerduty>` по метке маршрутизации для вашего компьютера.
ротация по вызову.

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

Экзекутес `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
перед внесением изменений для проверки устаревшего синтаксиса
`promtool check rules`.

## Матрица развертывания| Особенности рекламы | Р0 | Р1 | Р2 | Р3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` присутствует, канонические псевдонимы, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Отсутствие возможности `chunk_range_fetch` | ⚠️ Предупреждать (захват + телеметрия) | ⚠️ Предупреждать | ❌ Отклонить (`reason="missing_capability"`) | ❌ Отклонить |
| TLVs возможности не поддерживаются без `allow_unknown_capabilities=true` | ✅ | ⚠️ Предупреждаем (`reason="unknown_capability"`) | ❌ Отклонить | ❌ Отклонить |
| Срок действия `refresh_deadline` истек | ❌ Отклонить | ❌ Отклонить | ❌ Отклонить | ❌ Отклонить |
| `signature_strict=false` (диагностика приборов) | ✅ (уникальность разработки) | ⚠️ Предупреждать | ⚠️ Предупреждать | ❌ Отклонить |

Для всех часов используется UTC. Даты приведения в исполнение, которые размышляют в
миграционную книгу и не бужеронт без голосования в совете; рекламировать изменения
Требуйте, чтобы ваша книга была опубликована и опубликована в рамках пиара.

> **Примечание по внедрению:** R1 представляет серию `result="warn"` в
> `torii_sorafs_admission_total`. Патч для проглатывания Torii, который дополняет новый уровень
> этикетка est suivi avec les tâches de télémétrie SF-2; jusque-là, utilisez le

## Сообщение и сообщение о происшествии

- **Почтовое сообщение со статусом.** DevRel распространяет краткое резюме по показателям.
  д'прием, предупреждения на курсе и крайние сроки на вечере.
- **Ответный инцидент.** Если оповещения `reject` отключены, вызов по вызову:
  1. Восстановите рекламу с помощью обнаружения Torii (`/v1/sorafs/providers`).
  2. Соотнесите валидацию рекламы с поставщиком конвейера и сравните ее с
     `/v1/sorafs/providers` для воспроизведения ошибки.
  3. Коордонн с поставщиком услуг для ярмарки туров и рекламой перед прошайном
     срок обновления.
- **Гель изменений.** Подвеска с новой модификацией схемы возможностей.
  R1/R2 в тот момент, когда комитет по развертыванию недействителен; les essais GREASE doivent
  эти планы на период обслуживания журналистов и журналистов
  в миграционном реестре.

## Ссылки

- [SoraFS Протокол узла/клиента](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Политика приема поставщиков] (./provider-admission-policy)
- [Дорожная карта миграции](./migration-roadmap)
- [Расширения нескольких источников рекламы поставщика] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)