---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "SoraFS پرووائیڈر advert رول آؤٹ اور مطابقتی پلان"
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے ماخوذ۔

# SoraFS Рекламное объявление رول آؤٹ اور مطابقتی پلان

یہ پلان разрешающая реклама провайдера سے مکمل طور پر, управляемая `ProviderAdvertV1`
поверхность и вырезание, а также извлечение фрагментов из нескольких источников.
ضروری ہے۔ Результаты, которые вы можете получить:

- **Руководство для оператора.** Обратитесь к поставщикам систем хранения данных, которые могут использовать ворота.
  سے پہلے مکمل کرنا ہیں۔
- **Охват телеметрии.** Панели мониторинга и оповещения Возможность наблюдения и оперативного управления.
  Рекламные объявления, соответствующие требованиям قبول کرے۔
  SDK и инструментарий, новые выпуски и другие инструменты

یہ Внедрение [SoraFS дорожная карта миграции] (./migration-roadmap) и основные этапы SF-2b/2c
کے ساتھ align ہے اور فرض کرتا ہے کہ [политика допуска поставщика](./provider-admission-policy)
پہلے سے نافذ ہے۔

## Временная шкала этапов

| Фаза | Окно (цель) | Поведение | Действия оператора | Фокус наблюдаемости |
|-------|-----------------|-----------|------------------|-------------------|

## Контрольный список оператора

1. **Реклама инвентаря.** Опубликованное объявление может быть использовано в следующих случаях:
   - Путь управляющего конверта (производственный эквивалент `defaults/nexus/sorafs_admission/...`).
   - объявление `profile_id` или `profile_aliases`.
   - список возможностей (کم از کم `torii_gateway` или `chunk_range_fetch`).
   - Флаг `allow_unknown_capabilities` (зарезервированные поставщиком TLV-значения или значения TLV)۔
2. **Перегенерируйте с помощью инструментов поставщика.**
   - Поставщик, издатель, реклама, полезная нагрузка, которую вы можете использовать:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` اور واضح `max_span`
     - Предельно допустимые значения GREASE TLV `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` или `sorafs_fetch` سے validate کریں؛ неизвестно
     возможности کی предупреждения کو сортировка کریں۔
3. **Проверка готовности нескольких источников.**
   - `sorafs_fetch` или `--provider-advert=<path>`, как это сделать `chunk_range_fetch`
     Если произошел сбой CLI, игнорируются неизвестные возможности или появляются предупреждения.
     Отчет JSON для хранения журналов операций и архива
4. **Продление этапов.**
   - принудительное использование шлюза (R2) в течение 30 дней `ProviderAdmissionRenewalV1`
     конверты جمع کریں۔ обновления میں канонический дескриптор اور набор возможностей برقرار
     رہنا چاہئے؛ Доля, конечные точки и метаданные
5. **Общайтесь с зависимыми командами.**
   - Владельцы SDK могут выпускать и отклонять рекламу, а операторы - получать предупреждения.
   - Объявлено о фазовом переходе DevRel ہ؛ Ссылки на панель управления и пороговая логика شامل کریں۔
6. **Установите информационные панели и оповещения.**
   - Grafana экспортирует данные **SoraFS / Развертывание поставщика** позволяет указать UID
     `sorafs-provider-admission` رکھیں۔
   - یقینی بنائیں کہ правила оповещения постановка اور Production میں поделились
     Канал уведомлений `sorafs-advert-rollout` پر جائیں۔

## Телеметрия и информационные панели

یہ метрики `iroha_telemetry` کے ذریعے دستیاب ہیں:- `torii_sorafs_admission_total{result,reason}` — принято, отклонено с предупреждением
  результаты گنتا ہے۔ причины `missing_envelope`, `unknown_capability`, `stale`
  `policy_violation` شامل ہیں۔

Grafana экспорт: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Доступ к общему репозиторию информационных панелей (`observability/dashboards`) и импорту данных
Доступ к UID источника данных

Папка Grafana **SoraFS / Развертывание поставщика** Стабильный UID
`sorafs-provider-admission` کے ساتھ опубликовать ہوتا ہے۔ правила оповещений
`sorafs-admission-warn` (предупреждение) или `sorafs-admission-reject` (критический)
Политика уведомлений `sorafs-advert-rollout` может быть настроена.
Список пунктов назначения بدلے تو информационная панель JSON کو изменить کرنے کے بجائے точка контакта
обновление

Рекомендуемые панели Grafana:

| Панель | Запрос | Заметки |
|-------|-------|-------|
| **Показатель результатов поступления** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Стековая диаграмма: принять, предупредить или отклонить. предупреждение > 0,05 * всего (предупреждение) или отклонение > 0 (критическое) или предупреждение. |
| **Коэффициент предупреждений** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Однострочные временные ряды и порог пейджера и поток сообщений (15 минут или 5% вероятности предупреждения)۔ |
| **Причины отклонения** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | сортировка по Runbook کے لیے؛ Действия по смягчению последствий |
| **Обновить долг** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Крайний срок обновления пропустите поставщиков услуг, которые вам нужны. Журналы кэша обнаружения Использование перекрестных ссылок |

Ручные информационные панели и артефакты CLI:

- `sorafs_fetch --provider-metrics-out` - поставщик услуг `failures`, `successes`,
  Счетчики `disabled` пробные прогоны оркестратора и монитор
  специальные информационные панели или импорт کریں۔
- Отчет JSON о `chunk_retry_rate` и `provider_failure_rate`, регулирующих поля.
  симптомы устаревшей полезной нагрузки

### Grafana Макет приборной панели

Выделенная плата Observability — **SoraFS Допуск поставщика
Развертывание** (`sorafs-provider-admission`) — **SoraFS / Развертывание поставщика**
опубликовать или опубликовать канонические идентификаторы панелей:

- Панель 1 — *Частота исходов госпитализации* (область суммирования, یونٹ «операций/мин»).
- Панель 2 — *Коэффициент предупреждения* (одиночная серия), اظہار
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Панель 3 — *Причины отклонения* (`reason` کے حساب سے временной ряд), `rate(...[5m])`
  کے مطابق sort کی گئی۔
- Панель 4 — *Обновить долг* (статистика), а также запрос к таблице и зеркало.
  книга миграции سے حاصل کردہ сроки обновления объявления کے ساتھ с аннотациями ہے۔

Скелет JSON для репозитория информационных панелей инфраструктуры `observability/dashboards/sorafs_provider_admission.json`
Копировать или создать UID источника данных. Идентификаторы панелей и правила оповещений
Используйте runbooks или ссылки, а также перенумерацию или перенумерацию документов или документов.

Используйте эту справочную панель `docs/source/grafana_sorafs_admission.json`.
определение دیتا ہے؛ Для тестирования можно просмотреть папку Grafana, а затем скопировать папку.

### Prometheus правила оповещенийДля группы правил `observability/prometheus/sorafs_admission.rules.yml`
Выберите группу правил (например, SoraFS, группа правил), или Prometheus
конфигурация سے включает в себя کریں۔ `<pagerduty>` کو اپنے метка маршрутизации по вызову سے بدلیں۔

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

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
Синтаксис `promtool check rules`, который соответствует синтаксису `promtool check rules`.

## Коммуникация и обработка инцидентов

- **Еженедельная рассылка о состоянии.** Показатели приема DevRel, выдающиеся предупреждения и сроки выполнения, а также контрольные сроки.
- **Реагирование на инциденты.** `reject` оповещает о вызове по вызову:
  1. Обнаружение Torii (`/v2/sorafs/providers`) для получения оскорбительной рекламы.
  2. Конвейер провайдера проверяет рекламу или проверяет `/v2/sorafs/providers`, сравнивает, отображает ошибку, воспроизводит ہو۔
  3. Координаты провайдера и сроки обновления Сроки обновления سے پہلے Ротация рекламы ہو جائے۔
- **Изменение зависает.** R1/R2 может изменить схему возможностей, если требуется развертывание или развертывание. Испытания GREASE и окно обслуживания, график, журнал миграции и журнал регистрации.

## Ссылки

- [SoraFS Протокол узла/клиента](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Политика приема поставщиков] (./provider-admission-policy)
- [Дорожная карта миграции](./migration-roadmap)
- [Расширения нескольких источников рекламы поставщика] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)