<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 439248de434ac053bdf457055a2ac9ff501d19b371a435521e61e6a33566a1f6
source_last_modified: "2025-11-07T10:31:34.047816+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: "План rollout и совместимости advert провайдеров SoraFS"
---

> Адаптировано из [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# План rollout и совместимости advert провайдеров SoraFS

Этот план координирует переход от permissive advert провайдеров к полностью
управляемой поверхности `ProviderAdvertV1`, необходимой для multi-source выдачи
chunks. Он фокусируется на трех deliverables:

- **Руководство оператора.** Пошаговые действия, которые провайдеры хранения должны
  выполнить до включения каждого gate.
- **Покрытие телеметрией.** Дашборды и alerts, которые Observability и Ops используют,
  чтобы подтвердить, что сеть принимает только совместимые adverts.
- **График совместимости.** Явные даты отклонения legacy envelopes, чтобы команды
  SDK и tooling могли планировать релизы.

Rollout согласован с вехами SF-2b/2c в
[roadmap миграции SoraFS](./migration-roadmap) и предполагает, что policy допуска в
[provider admission policy](./provider-admission-policy) уже действует.

## Таймлайн фаз

| Фаза | Окно (цель) | Поведение | Действия операторов | Фокус наблюдаемости |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 – Базовое наблюдение** | До **2025-03-31** | Torii принимает как adverts, одобренные governance, так и legacy payloads до `ProviderAdvertV1`. Логи ingestion предупреждают, когда adverts не содержат `chunk_range_fetch` или канонические `profile_aliases`. | - Регенерировать adverts через pipeline публикации provider advert (ProviderAdvertV1 + governance envelope), убедившись в `profile_id=sorafs.sf1@1.0.0`, канонических `profile_aliases` и `signature_strict=true`. <br />- Запустить новые тесты `sorafs_fetch` локально; предупреждения о неизвестных capabilities нужно триажить. | Опубликовать временные панели Grafana (см. ниже) и установить пороги alert, но держать их в режиме предупреждений. |
| **R1 – Gate предупреждений** | **2025-04-01 → 2025-05-15** | Torii продолжает принимать legacy adverts, но инкрементирует `torii_sorafs_admission_total{result="warn"}`, когда payload не содержит `chunk_range_fetch` или несет неизвестные capabilities без `allow_unknown_capabilities=true`. CLI tooling теперь падает при регенерации, если нет канонического handle. | - Повернуть adverts в staging и production, включив payloads `CapabilityType::ChunkRangeFetch` и, при GREASE testing, установить `allow_unknown_capabilities=true`. <br />- Обновить operations runbooks новыми запросами телеметрии. | Продвинуть dashboards в on-call ротацию; настроить предупреждения, когда события `warn` превышают 5% трафика за 15 минут. |
| **R2 – Enforcement** | **2025-05-16 → 2025-06-30** | Torii отклоняет adverts без governance envelopes, канонического handle профиля или capability `chunk_range_fetch`. Legacy handles только `namespace-name` больше не парсятся. Неизвестные capabilities без GREASE opt-in теперь падают с `reason="unknown_capability"`. | - Подтвердить, что production envelopes лежат в `torii.sorafs.admission_envelopes_dir`, и заменить все оставшиеся legacy adverts. <br />- Проверить, что SDKs эмитят только канонические handles плюс опциональные aliases для обратной совместимости. | Включить pager alerts: `torii_sorafs_admission_total{result="reject"}` > 0 в течение 5 минут требует реакции оператора. Отслеживать ratio принятия и гистограммы причин допуска. |
| **R3 – Отключение legacy** | **2025-07-01 и далее** | Discovery отключает поддержку бинарных adverts, которые не выставляют `signature_strict=true` или не содержат `profile_aliases`. Кэш discovery Torii очищает устаревшие записи, у которых прошел refresh deadline без продления. | - Запланировать финальное окно decommission для legacy provider stacks. <br />- Убедиться, что GREASE `--allow-unknown` выполняется только во время контролируемых drills и логируется. <br />- Обновить incident playbooks, чтобы предупреждения `sorafs_fetch` считались блокером перед релизами. | Ужесточить alerts: любой `warn` сигналит on-call. Добавить синтетические проверки, которые получают discovery JSON и валидируют списки capabilities providers. |

## Чеклист оператора

1. **Инвентаризировать adverts.** Перечислите каждый опубликованный advert и зафиксируйте:
   - Путь к governing envelope (`defaults/nexus/sorafs_admission/...` или production-эквивалент).
   - `profile_id` и `profile_aliases` advert.
   - Список capabilities (ожидается как минимум `torii_gateway` и `chunk_range_fetch`).
   - Флаг `allow_unknown_capabilities` (обязателен при наличии vendor-reserved TLV).
2. **Регенерация через provider tooling.**
   - Пересоберите payload через publisher advert, убедившись в:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` с определенным `max_span`
     - `allow_unknown_capabilities=<true|false>` при наличии GREASE TLV
   - Проверьте через `/v1/sorafs/providers` и `sorafs_fetch`; предупреждения о
     неизвестных capabilities нужно триажить.
3. **Проверка multi-source readiness.**
   - Выполните `sorafs_fetch` с `--provider-advert=<path>`; CLI теперь падает,
     когда отсутствует `chunk_range_fetch`, и печатает предупреждения о
     проигнорированных неизвестных capabilities. Зафиксируйте JSON-отчет и
     архивируйте его с operations logs.
4. **Подготовка продлений.**
   - Отправьте `ProviderAdmissionRenewalV1` envelopes минимум за 30 дней до
     gateway enforcement (R2). Продления должны сохранять канонический handle и
     набор capabilities; менять следует только stake, endpoints или metadata.
5. **Коммуникация с зависимыми командами.**
   - Владельцы SDK должны выпускать версии, которые показывают warnings операторам
     при отклонении adverts.
   - DevRel анонсирует каждую фазу; включайте ссылки на dashboards и логику
     порогов ниже.
6. **Установка dashboards и alerts.**
   - Импортируйте Grafana export и разместите его в **SoraFS / Provider
     Rollout** с UID `sorafs-provider-admission`.
   - Убедитесь, что alert rules направлены в общий канал
     `sorafs-advert-rollout` в staging и production.

## Телеметрия и дашборды

Следующие метрики уже доступны через `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — счетчики принятия, отклонения
  и warnings. Причины включают `missing_envelope`, `unknown_capability`, `stale` и
  `policy_violation`.

Grafana export: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Импортируйте файл в общий репозиторий дашбордов (`observability/dashboards`) и
обновите только UID datasource перед публикацией.

Дашборд публикуется в папке Grafana **SoraFS / Provider Rollout** с
стабильным UID `sorafs-provider-admission`. Alert rules
`sorafs-admission-warn` (warning) и `sorafs-admission-reject` (critical)
преднастроены на policy уведомлений `sorafs-advert-rollout`; меняйте контактный
пункт при изменении списка получателей вместо правки JSON дашборда.

Рекомендуемые панели Grafana:

| Panel | Query | Notes |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Stack chart для визуализации accept vs warn vs reject. Alert при warn > 0.05 * total (warning) или reject > 0 (critical). |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Однолинейная timeseries, питающая порог pager (5% warning rate в скользящем 15-минутном окне). |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Для triage в runbook; прикрепляйте ссылки на шаги mitigation. |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Указывает на providers, пропустивших refresh deadline; сверяйте с логами discovery cache. |

CLI артефакты для ручных дашбордов:

- `sorafs_fetch --provider-metrics-out` пишет счетчики `failures`, `successes` и
  `disabled` по каждому provider. Импортируйте в ad-hoc dashboards, чтобы
  мониторить dry-run orchestrator перед переключением production providers.
- Поля `chunk_retry_rate` и `provider_failure_rate` в JSON-отчете
  подсвечивают throttling или симптомы stale payloads, которые часто предшествуют
  отклонениям admission.

### Раскладка Grafana дашборда

Observability публикует отдельный board — **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) — в **SoraFS / Provider Rollout**
со следующими каноническими panel IDs:

- Panel 1 — *Admission outcome rate* (stacked area, единица "ops/min").
- Panel 2 — *Warning ratio* (single series), выражение
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Rejection reasons* (time series, сгруппированные по `reason`), сортировка по
  `rate(...[5m])`.
- Panel 4 — *Refresh debt* (stat), отражает запрос из таблицы выше и
  аннотирован refresh deadline из migration ledger.

Скопируйте (или создайте) JSON skeleton в репозитории инфраструктурных дашбордов
`observability/dashboards/sorafs_provider_admission.json`, затем обновите только
UID datasource; panel IDs и alert rules используются в runbooks ниже, поэтому не
перенумеровывайте их без обновления этой документации.

Для удобства репозиторий уже содержит reference dashboard definition в
`docs/source/grafana_sorafs_admission.json`; скопируйте его в вашу Grafana папку,
если нужен стартовый вариант для локального тестирования.

### Правила алертов Prometheus

Добавьте следующую группу правил в
`observability/prometheus/sorafs_admission.rules.yml` (создайте файл, если это
первая группа правил SoraFS) и подключите ее в конфигурации Prometheus.
Замените `<pagerduty>` на реальный routing label для вашей on-call ротации.

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

| Характеристики advert | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` присутствует, канонические aliases, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Нет capability `chunk_range_fetch` | ⚠️ Warn (ingest + telemetry) | ⚠️ Warn | ❌ Reject (`reason="missing_capability"`) | ❌ Reject |
| TLV неизвестной capability без `allow_unknown_capabilities=true` | ✅ | ⚠️ Warn (`reason="unknown_capability"`) | ❌ Reject | ❌ Reject |
| Legacy handle только (`profile_id = sorafs.sf1@1.0.0`) | ⚠️ Warn | ❌ Reject | ❌ Reject | ❌ Reject |
| Истекший `refresh_deadline` | ❌ Reject | ❌ Reject | ❌ Reject | ❌ Reject |
| `signature_strict=false` (diagnostic fixtures) | ✅ (только development) | ⚠️ Warn | ⚠️ Warn | ❌ Reject |

Все времена указаны в UTC. Даты enforcement отражены в migration ledger и не
будут изменены без голосования council; любые изменения требуют обновления этого
файла и ledger в одном PR.

> **Примечание по реализации:** R1 вводит серию `result="warn"` в
> `torii_sorafs_admission_total`. Патч ingestion Torii, добавляющий новый label,
> отслеживается вместе с задачами телеметрии SF-2; до его попадания используйте
> лог-сэмплинг для мониторинга legacy adverts.

## Коммуникация и обработка инцидентов

- **Еженедельная рассылка статуса.** DevRel рассылает краткое резюме метрик
  admission, текущих warnings и предстоящих deadlines.
- **Incident response.** Если срабатывают alerts `reject`, on-call инженеры:
  1. Забирают проблемный advert через discovery Torii (`/v1/sorafs/providers`).
  2. Повторяют валидацию advert в provider pipeline и сравнивают с
     `/v1/sorafs/providers`, чтобы воспроизвести ошибку.
  3. Координируют с провайдером ротацию advert до следующего refresh deadline.
- **Заморозка изменений.** Никаких изменений schema capabilities в R1/R2, если
  комитет rollout не одобрит; GREASE испытания проводите только в еженедельное
  окно обслуживания и фиксируйте в migration ledger.

## Ссылки

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
