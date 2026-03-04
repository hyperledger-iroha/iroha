---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "خطة طرح وتوافق adverts لمزودي SoraFS"
---

> Напишите [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة طرح وتوافق рекламные объявления لمزودي SoraFS

تنسق هذه الخطة الانتقال من adverts لمسموحة لمزودي التخزين إلى سطح `ProviderAdvertV1`
Затем нарежьте кусочки и нарежьте их кусочками. и Тэрри Султан
Ответ на вопрос:

- **دليل المشغل.** Он сказал, что в действительности он находится у ворот.
- **Обзор системы наблюдения.** Проверка наблюдаемости и оперативного управления.
  Реклама в Интернете в рекламе المتوافقة فقط.
- **الجدول الزمني للتوافق.** تواريخ واضحة لرفض конверты القديمة حتى تتمكن
  Воспользуйтесь SDK и инструментами для разработки.

Создан на базе SF-2b/2c в
[خارطة طريق هجرة SoraFS](./migration-roadmap) и нажмите на него
[политика допуска провайдера](./provider-admission-policy) مطبقة بالفعل.

## الجدول الزمني للمراحل| عرحلة | النافذة (الهدف) | السلوك | إجراءات المشغل | تركيز الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 - Информационный бюллетень** | ** 31 марта 2025 г.** | يقبل Torii в рекламных объявлениях, связанных с мобильными устройствами и полезными нагрузками. `ProviderAdvertV1`. تسجل سجلات ingesting تحذيرات عندما تهمل рекламные объявления `chunk_range_fetch` и `profile_aliases` القياسية. | - Отображение рекламы в конвейере и реклама поставщика (ProviderAdvertV1 + конверт управления) в `profile_id=sorafs.sf1@1.0.0` и `profile_aliases`. و`signature_strict=true`. - تشغيل اختبارات `sorafs_fetch` محليا؛ Возможности сортировки и сортировки. | Код Grafana Зарегистрирован (на английском языке) и установлен на сайте. В التحذير فقط. |
| **R1 - Информационный центр** | **01.04.2025 → 15.05.2025** | يستمر Torii для рекламы в Лос-Анджелесе `torii_sorafs_admission_total{result="warn"}` для полезной нагрузки `chunk_range_fetch` соответствует возможностям `allow_unknown_capabilities=true`. Инструменты CLI доступны для редактирования и управления ручкой. | - Показаны объявления о постановке и производстве полезных нагрузок `CapabilityType::ChunkRangeFetch`, а также тестирование GREASE `allow_unknown_capabilities=true`. - Вы можете просмотреть информацию о Runbooks в Интернете. | Приборные панели и работа по вызову; إعداد تحذيرات عندما تتجاوز أحداث `warn` на 5% в течение 15 дней. Да. |
| **R2 – Правоприменение** | **16 мая 2025 г. → 30 июня 2025 г.** | يرفض Torii реклама в конвертах и в ручке, а также в дополнительных возможностях `chunk_range_fetch`. Он обрабатывает файл `namespace-name`. Дополнительные возможности можно получить при выборе подписки на GREASE `reason="unknown_capability"`. | - Наслаждайтесь конвертами с номером `torii.sorafs.admission_envelopes_dir` и рекламой на сайте. - В пакете SDK обрабатываются псевдонимы псевдонимов пользователя и пользователя. | Оповещения пейджера: `torii_sorafs_admission_total{result="reject"}` > 0 за 5 дней до завершения. Он был убит в Нью-Йорке. |
| **R3 - Информационный бюллетень** | **Обновление от 01.07.2025** | Откройте для себя Discovery в рекламных объявлениях, посвященных `signature_strict=true`, а также `profile_aliases`. Кэш обнаружения Torii был установлен в ближайшее время. Крайний срок истекает. | - Вывод из эксплуатации в Лас-Вегасе. - Используйте СМАЗКУ GREASE `--allow-unknown` для сверл, которые необходимо использовать. - Пособия по играм для детей и взрослых `sorafs_fetch`. | Сообщение: По вызову `warn`. Вы можете получить доступ к открытию JSON с учетом имеющихся возможностей. |

## قائمة تحقق المشغل1. **Информационная реклама.** Информация о рекламе:
   - مسار الـ управляющий конверт (`defaults/nexus/sorafs_admission/...` أو ما يعادله في الإنتاج).
   - `profile_id` и `profile_aliases` в рекламе.
   - Дополнительные возможности (можно использовать `torii_gateway` и `chunk_range_fetch`).
   - علم `allow_unknown_capabilities` (необходимо указать TLV от поставщика).
2. **Доступно для изготовления инструментов.**
   - Полезная нагрузка отображается в объявлении поставщика, а также в следующих случаях:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` в `max_span` в наличии
     - `allow_unknown_capabilities=<true|false>` для определения ПДК для смазки.
   - تحقق عبر `/v1/sorafs/providers` и `sorafs_fetch`; Сортировочная сортировка
     возможности غير المعروفة.
3. **Использование нескольких источников.**
   - نفذ `sorafs_fetch` مع `--provider-advert=<path>`; Интерфейс CLI
     `chunk_range_fetch` позволяет использовать возможности расширения.
     Создайте файл JSON для веб-сайта.
4. **Получить информацию.**
   - Конверты от `ProviderAdmissionRenewalV1` до 30 дней в году.
     принудительное исполнение через шлюз (R2). Он и его помощник в обращении с ручкой.
     Широкие возможности; Здесь можно указать долю, конечные точки и метаданные.
5. **Полный доступ к информации.**
   - Воспользуйтесь SDK, чтобы просмотреть рекламу в Интернете.
   - Информация DevRel в новом выпуске Используйте приборные панели для просмотра.
6. **Открытые панели мониторинга.**
   - Зарегистрируйте Grafana и укажите **SoraFS / Развертывание поставщика** с UID.
     `sorafs-provider-admission`.
   - تأكد من قواعد التنبيه تشير إلى قناة `sorafs-advert-rollout` المشتركة
     И постановка, и производство.

## التليمترية ولوحات المعلومات

Обратитесь к файлу `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — يعد القبول والرفض ونتائج التحذير.
  Установите `missing_envelope` и `unknown_capability` и `stale` и `policy_violation`.

Код Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Для создания панелей мониторинга (`observability/dashboards`) مع
UID будет указан в вашем аккаунте.

Для этого введите Grafana **SoraFS / Развертывание поставщика** с UID-кодом.
`sorafs-provider-admission`. قواعد التنبيه `sorafs-admission-warn` (предупреждение) и
`sorafs-admission-reject` (критический)
`sorafs-advert-rollout`; Он сказал, что хочет, чтобы это произошло с ним.
Формат JSON.

Код Grafana Сообщение:

| Новости | الاستعلام | الملاحظات |
|-------|-------|-------|
| **Показатель результатов поступления** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Выберите вариант «принять, предупредить или отклонить». Отменить предупреждение > 0,05 * всего (предупреждение) или отклонить > 0 (критическое). |
| **Коэффициент предупреждений** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Используйте пейджер для пейджера (от 5% до 15 дней). |
| **Причины отклонения** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Сортировка в Runbook; Он сказал, что это не так. |
| **Обновить долг** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Он сказал: Создан кэш обнаружения. |

Артефакты CLI на информационных панелях:- `sorafs_fetch --provider-metrics-out` для `failures` и `successes` و
  `disabled` в порядке. Создание информационных панелей для специальных пробных прогонов
  оркестратор Кейбл Торонто Уилсон.
- Задайте `chunk_retry_rate` и `provider_failure_rate` в формате JSON в формате JSON.
  регулирование и устаревшие полезные данные

### تخطيط لوحة Grafana

تنشر Observability لوحة مخصصة — **SoraFS Доступ поставщика
Развертывание** (`sorafs-provider-admission`) — ضمن **SoraFS / Развертывание поставщика**
В качестве примера можно привести:

- Панель 1 — *Частота исходов госпитализации* (область суммирования, например «операций/мин»).
- Панель 2 — *Коэффициент предупреждения* (одна серия), مع التعبير
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Панель 3 — *Причины отклонения* (временной ряд مجمعة حسب `reason`), مرتبة حسب
  `rate(...[5m])`.
- Панель 4 — *Обновить задолженность* (статистика), يعكس الستعلام أعلاه ومُعلق بمهل обновить
  Создайте миграционную книгу.

Скелет JSON (идентификатор) для просмотра изображений в формате JSON.
`observability/dashboards/sorafs_provider_admission.json`, код UID для UID
بيانات؛ Вы можете получить доступ к Runbooks, созданным в рамках программы Runbooks.
Он провёл Дэниела Трэвиса в Вашингтоне.

Он сказал, что хочет, чтобы он сказал:
`docs/source/grafana_sorafs_admission.json`; انسخه إلى مجلد Grafana عند الحاجة
Если вы хотите, чтобы это произошло, вы должны сделать это.

### قواعد تنبيه Prometheus

أضف مجموعة القواعد التالية إلى
`observability/prometheus/sorafs_admission.rules.yml` (أنشئ الملف إن كانت هذه
أول مجموعة قواعد SoraFS) и нажмите Prometheus. Код `<pagerduty>`
بعلامة التوجيه الفعلية لدوام المناوبة.

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

Код `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
Он был установлен в соответствии с требованиями `promtool check rules`.

## مصفوفة التوافق

| خصائص реклама | Р0 | Р1 | Р2 | Р3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` — псевдонимы قياسية, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| غياب возможности `chunk_range_fetch` | ⚠️ Предупреждать (захват + телеметрия) | ⚠️ Предупреждать | ❌ Отклонить (`reason="missing_capability"`) | ❌ Отклонить |
| Возможности TLVs غير معروفة دون `allow_unknown_capabilities=true` | ✅ | ⚠️ Предупреждаем (`reason="unknown_capability"`) | ❌ Отклонить | ❌ Отклонить |
| `refresh_deadline` منتهي | ❌ Отклонить | ❌ Отклонить | ❌ Отклонить | ❌ Отклонить |
| `signature_strict=false` (диагностические приборы) | ✅ (للتطوير فقط) | ⚠️ Предупреждать | ⚠️ Предупреждать | ❌ Отклонить |

В штате UTC. Правоприменение в Миграционном реестре в Сбербанке
Дэн Тэйлор В журнале The Wall Street Journal в журнале PR.

> **Подробнее:** Для R1 используется `result="warn"`.
> `torii_sorafs_admission_total`. Выполните вставку в Torii для ввода данных.
> В наличии есть SF-2; Он сказал, что он хочет, чтобы он был в центре внимания.

## التواصل ومعالجة الحوادث- **Создание приложения.** Создано DevRel, созданное в 1999 году.
  والمواعيد القادمة.
- **Зарегистрируйтесь.** Информацию о вызове `reject` можно получить по вызову Бэнна Колла:
  1. Рекламное объявление о открытии Torii (`/v1/sorafs/providers`).
  2. Разместите рекламу в трубопроводе, посвященном технологическому процессу.
     `/v1/sorafs/providers` отключен.
  3. Обновите рекламное объявление о новом обновлении.
- **Открыть доступ.** Доступ к схеме и возможностям R1/R2 на уровне R1/R2.
  В начале сентября было развернуто внедрение; СМАЗКА СМАЗКИ
  Создан в миграционном журнале.

## المراجع

- [SoraFS Протокол узла/клиента](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Политика приема поставщиков] (./provider-admission-policy)
- [Дорожная карта миграции](./migration-roadmap)
- [Расширения нескольких источников рекламы поставщика] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)