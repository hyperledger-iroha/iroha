---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: «План развертывания рекламы поставщиков SoraFS»
---

> Адаптация [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# План развертывания рекламы поставщиков SoraFS

Этот план скоординирован или отключен от рекламы, разрешенной поставщиками услуг.
общее управление `ProviderAdvertV1` exigida для поиска
фрагменты из нескольких источников. Основные результаты:

- **Руководство по эксплуатации.** Пройдите, как поставщики хранилищ предварительно подключатся к воротам.
- **Кобертура телеметрии.** Панели мониторинга и оповещения, которые используются для наблюдения и эксплуатации.
  Чтобы подтвердить, что реклама Rede aceita apenas соответствует.
  для оснащения SDK и инструментов для выпуска релизов.

О развертывании можно узнать у Маркоса SF-2b/2c нет
[дорожная карта миграции SoraFS](./migration-roadmap) Предположим, что это политика допуска
нет [политика приема поставщика](./provider-admission-policy) ja esta ativa.

## Хронология фаз

| Фаза | Джанела (альво) | Компактность | Действия оператора | Фокус наблюдения |
|-------|-----------------|-----------|------------------|-------------------|

## Контрольный список действий оператора

1. **Рекламные объявления.** Список всех опубликованных и зарегистрированных объявлений:
   - Caminho do управляющий конверт (`defaults/nexus/sorafs_admission/...` или эквивалент производства).
   - `profile_id` и `profile_aliases` делают рекламу.
   - Список возможностей (пожалуйста, выберите `torii_gateway` и `chunk_range_fetch`).
   - Флаг `allow_unknown_capabilities` (необходимо указать TLV, зарезервированные поставщиком).
2. **Обновите инструменты поставщика услуг.**
   - Реконструкция полезной нагрузки от издателя рекламы поставщика, гарантия:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` с `max_span` определено
     - `allow_unknown_capabilities=<true|false>` при снятии ПДК СМАЗКА
   - Действительно через `/v2/sorafs/providers` и `sorafs_fetch`; предупреждения трезвые возможности
     desconhecidas devem ser triageadas.
3. **Многоисточник готовности Validar.**
   - Выполните `sorafs_fetch` com `--provider-advert=<path>`; o CLI agora falha quando
     `chunk_range_fetch` есть предупреждения и предупреждения о возможностях, которые могут быть отключены
     игнорады. Создавайте отчеты в формате JSON и архивируйте журналы операций.
4. **Подготовка к ремонту.**
   - Конверты Envie `ProviderAdmissionRenewalV1` pelo menos за 30 дней до этого.
     принудительное исполнение без шлюза (R2). Renovacoes разработали способ обработки canonico e o
     набор возможностей; apenas кол, конечные точки или метаданные должны быть определены.
5. **Коммуникар оборудует иждивенцев.**
   - В SDK разрабатываются версии, которые чаще всего выдают предупреждения в любой момент работы.
     реклама для недовольных.
   - DevRel объявляет о быстром переходе; включая ссылки на информационные панели и логику
     де пороги abaixo.
6. **Установка информационных панелей и оповещений.**
   - Импорт или экспорт Grafana и ввод сообщения **SoraFS / Развертывание поставщика** с UID.
     `sorafs-provider-admission`.
   - Гарантия, что будет объявлено о тревоге перед совмещением каналов.
     `sorafs-advert-rollout` в организации производства.

## Телеметрия и информационные панели

Как показываются метрики и результаты через `iroha_telemetry`:- `torii_sorafs_admission_total{result,reason}` - сведения об этом, ошибки и
  предупреждения. В число мотивов входят `missing_envelope`, `unknown_capability`, `stale`.
  е `policy_violation`.

Grafana экспорт: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Импортировать или заархивировать хранилище, содержащее панели мониторинга (`observability/dashboards`).
Перед публикацией можно реализовать возможности или UID для источника данных.

Публикация и публикация Grafana **SoraFS / Provider Rollout** с или UID
эставель `sorafs-provider-admission`. По мере предупреждения
`sorafs-admission-warn` (предупреждение) и `sorafs-admission-reject` (критическое) осталось
предварительно настроены для использования политики уведомлений `sorafs-advert-rollout`; отрегулировать
Это контактная точка, где вам будет назначено редактирование или JSON для панели управления.

Painels Grafana рекомендует:

| Панель | Запрос | Заметки |
|-------|-------|-------|
| **Показатель результатов поступления** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Стековая диаграмма для визуализации: принять, предупредить или отклонить. Оповещение, когда предупреждение > 0,05 * общее количество (предупреждение) или отклонение > 0 (критическое). |
| **Коэффициент предупреждений** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Уникальная временная серия, в которой содержится пища или порог для пейджера (5% частота предупреждений за 15 минут). |
| **Причины отклонения** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guia triage do runbook; ссылки на приложения для смягчения последствий. |
| **Обновить долг** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Поставщики Indica, которые просрочат или обновят крайний срок; Журналы Cruze Com выполняют кэш обнаружения. |

Артефакты CLI для руководства по панелям мониторинга:

- `sorafs_fetch --provider-metrics-out` escreve contadores `failures`, `successes`
  e `disabled` пор-провайдер. Импортируйте информационные панели для конкретного случая для мониторинга пробных прогонов.
  сделать оркестратор перед поставщиками троакаров в производстве.
- Кампос `chunk_retry_rate` и `provider_failure_rate` отправляют отчет в формате JSON destacam.
  регулирование или ограничение полезной нагрузки, которая устарела до отказа в приеме.

### Макет приборной панели Grafana

Специальная доска Observability Publica um — **SoraFS Допуск поставщика
Развертывание** (`sorafs-provider-admission`) - всхлип **SoraFS / Развертывание поставщика**
com с последующими идентификаторами канонических панелей:

- Панель 1 - *Частота исходов госпитализации* (площадь с накоплением, единица «операций/мин»).
- Панель 2 - *Коэффициент предупреждения* (одна серия), в экспресс-режиме
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Панель 3 - *Причины отклонения* (временной ряд по `reason`), ordenada по
  `rate(...[5m])`.
- Панель 4 - *Обновить задолженность* (статистика), ответив на запрос таблицы acima и e anotada
  com сроки обновления объявлений, дополнительные объявления, журнал миграции.

Копия (или крик) или скелет JSON без хранилища информационных панелей в инфра-эмисменте
`observability/dashboards/sorafs_provider_admission.json`, предварительно настройте апены
o UID источника данных; Идентификаторы панелей и уведомления об оповещениях по ссылкам на страницы
runbooks abaixo, entao evite renumerar sem revisar esta documentacao.

Для удобства в хранилище включена определенная информационная панель ссылок.
`docs/source/grafana_sorafs_admission.json`; копия для макарон Grafana se
Precisar de um ponto de partida para testes localis.

### Сообщение об оповещении PrometheusДобавление или следующая группа регра в них
`observability/prometheus/sorafs_admission.rules.yml` (crie o arquivo se este for
o первой группе регистрации SoraFS) и включенной в конфигурацию Prometheus.
Замените `<pagerduty>` на этикетке реального ротационного движения по вызову.

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

Выполнить `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
Перед отправкой необходимо гарантировать, что синтаксис пройдет `promtool check rules`.

## Матрица развертывания

| Характеристики объявления | Р0 | Р1 | Р2 | Р3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` присутствует, псевдонимы canonicos, `signature_strict=true` | ОК | ОК | ОК | ОК |
| Возможность Ausencia de `chunk_range_fetch` | ПРЕДУПРЕЖДЕНИЕ (захват + телеметрия) | ПРЕДУПРЕЖДАТЬ | ОТКЛОНИТЬ (`reason="missing_capability"`) | ОТКЛОНИТЬ |
| TLV-значения возможности отключения `allow_unknown_capabilities=true` | ОК | ПРЕДУПРЕЖДЕНИЕ (`reason="unknown_capability"`) | ОТКЛОНИТЬ | ОТКЛОНИТЬ |
| `refresh_deadline` истек | ОТКЛОНИТЬ | ОТКЛОНИТЬ | ОТКЛОНИТЬ | ОТКЛОНИТЬ |
| `signature_strict=false` (диагностические приборы) | ОК (приложения для разработки) | ПРЕДУПРЕЖДАТЬ | ПРЕДУПРЕЖДАТЬ | ОТКЛОНИТЬ |

Все часы дня UTC. Данные принудительного исполнения sao refletidas без миграции
бухгалтерская книга и нао мудам сем вото до совета; Что нужно сделать, чтобы проверить это
arquivo e o Ledger без серьезного PR.

> **Примечание о внедрении:** R1 представляет серию `result="warn"` em
> `torii_sorafs_admission_total`. Патч для приема Torii, который добавляется
> Новая метка и сопровождение тарифов телеметрии SF-2; съел ла, используй

## Сообщение и обработка инцидентов

- **Еженедельная рассылка о статусе.** DevRel сравнивает результаты с показателями поступления,
  предупреждения pendentes e крайние сроки proximas.
- **Реагирование на инциденты.** Если оповещения `reject` не совпадают, дежурные инженеры:
  1. Запускайте или оскорбляйте рекламу через обнаружение Torii (`/v2/sorafs/providers`).
  2. Повторно выполните проверку правильности объявления без конвейера у провайдера и сравнения с ним.
     `/v2/sorafs/providers` для воспроизведения или ошибки.
  3. Уведомляйте поставщика о ротационном объявлении до ближайшего крайнего срока обновления.
- **Изменение зависает.** В течение R1/R2 отсутствует схема возможностей.
  menos que o comite de развертывание одобрено; испытания GREASE разработаны для повесток дня
  Джанела семанал де ручное управление и регистрация в миграционном реестре.

## Ссылки

- [SoraFS Протокол узла/клиента](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Политика приема поставщиков] (./provider-admission-policy)
- [Дорожная карта миграции](./migration-roadmap)
- [Расширения нескольких источников рекламы поставщика] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)