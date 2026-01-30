---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fc980211cb96290ff9102ca762b5940b77447401eceb6d7689e1da8bd720ed7
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
id: multi-source-rollout
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Канонический источник
Эта страница отражает `docs/source/sorafs/runbooks/multi_source_rollout.md`. Держите обе копии синхронизированными, пока устаревший набор документации не будет выведен из обращения.
:::

## Назначение

Этот runbook проводит SRE и дежурных инженеров через два критических процесса:

1. Выкатывать мульти-источниковый оркестратор контролируемыми волнами.
2. Заносить в чёрный список или понижать приоритет проблемных провайдеров без дестабилизации текущих сессий.

Предполагается, что стек оркестрации, поставленный в рамках SF-6, уже развернут (`sorafs_orchestrator`, gateway API диапазона чанков, экспортеры телеметрии).

> **См. также:** [Runbook по эксплуатации оркестратора](./orchestrator-ops.md) подробно описывает процедуры на прогон (снятие scoreboard, переключатели поэтапного rollout, rollback). Используйте обе ссылки вместе во время живых изменений.

## 1. Предстартовая валидация

1. **Подтвердить входные данные governance.**
   - Все кандидаты-провайдеры должны публиковать конверты `ProviderAdvertV1` с payload'ами диапазонных возможностей и бюджетами потоков. Проверяйте через `/v1/sorafs/providers` и сверяйте с ожидаемыми полями возможностей.
   - Снимки телеметрии с метриками латентности/сбоев должны быть не старше 15 минут перед каждым canary-прогоном.
2. **Подготовить конфигурацию.**
   - Сохраните JSON-конфиг оркестратора в слоистом дереве `iroha_config`:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Обновите JSON с лимитами под rollout (`max_providers`, бюджеты ретраев). Используйте один и тот же файл для staging/production, чтобы различия оставались минимальными.
3. **Прогнать канонические fixtures.**
   - Заполните переменные окружения manifest/token и выполните детерминированный fetch:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Переменные окружения должны содержать digest payload манифеста (hex) и base64-кодированные stream-токены для каждого провайдера, участвующего в canary.
   - Сравните `artifacts/canary.scoreboard.json` с прошлым релизом. Любой новый невалидный провайдер или сдвиг веса >10% требует ревью.
4. **Проверить, что телеметрия подключена.**
   - Откройте экспорт Grafana в `docs/examples/sorafs_fetch_dashboard.json`. Убедитесь, что метрики `sorafs_orchestrator_*` заполняются в staging перед продолжением.

## 2. Экстренное занесение провайдеров в чёрный список

Следуйте этой процедуре, когда провайдер отдает поврежденные чанки, устойчиво таймаутится или не проходит проверки соответствия.

1. **Зафиксировать доказательства.**
   - Экспортируйте последний fetch-свод (вывод `--json-out`). Запишите индексы сбойных чанков, алиасы провайдеров и несовпадения digest.
   - Сохраните релевантные фрагменты логов из таргетов `telemetry::sorafs.fetch.*`.
2. **Применить немедленный override.**
   - Отметьте провайдера как penalized в снимке телеметрии, передаваемом оркестратору (установите `penalty=true` или ограничьте `token_health` до `0`). Следующая сборка scoreboard автоматически исключит провайдера.
   - Для ad-hoc smoke-тестов передайте `--deny-provider gw-alpha` в `sorafs_cli fetch`, чтобы отработать путь отказа без ожидания распространения телеметрии.
   - Переразверните обновленный пакет телеметрии/конфигурации в затронутой среде (staging → canary → production). Задокументируйте изменение в журнале инцидента.
3. **Проверить override.**
   - Повторите fetch канонического fixture. Убедитесь, что scoreboard помечает провайдера как невалидного с причиной `policy_denied`.
   - Проверьте `sorafs_orchestrator_provider_failures_total`, чтобы убедиться, что счетчик перестал расти для отклоненного провайдера.
4. **Эскалировать долгие блокировки.**
   - Если провайдер будет заблокирован >24 h, заведите governance-тикет на ротацию или приостановку его advert. До голосования держите deny-list и обновляйте снимки телеметрии, чтобы провайдер не вернулся в scoreboard.
5. **Протокол отката.**
   - Чтобы вернуть провайдера, удалите его из deny-листа, переразверните и снимите новый snapshot scoreboard. Приложите изменение к postmortem инцидента.

## 3. План поэтапного rollout

| Фаза | Охват | Требуемые сигналы | Критерии Go/No-Go |
|------|-------|-------------------|-------------------|
| **Lab** | Выделенный интеграционный кластер | Ручной CLI fetch по payloads фикстур | Все чанки проходят, счетчики провайдерских сбоев остаются на 0, доля ретраев < 5%. |
| **Staging** | Полный staging control-plane | Dashboard Grafana подключен; правила алертов в режиме warning-only | `sorafs_orchestrator_active_fetches` возвращается к нулю после каждого тестового прогона; нет алертов `warn/critical`. |
| **Canary** | ≤10% прод-трафика | Pager выключен, но телеметрия мониторится в реальном времени | Доля ретраев < 10%, провайдерские сбои ограничены известными шумными пирами, гистограмма латентности совпадает со staging baseline ±20%. |
| **Общая доступность** | 100% rollout | Правила pager активны | Ноль ошибок `NoHealthyProviders` за 24 h, доля ретраев стабильна, панели SLA на дашборде зеленые. |

Для каждой фазы:

1. Обновите JSON оркестратора с планируемыми `max_providers` и бюджетами ретраев.
2. Запустите `sorafs_cli fetch` или интеграционные тесты SDK на каноническом fixture и репрезентативном manifest из среды.
3. Сохраните artefacts scoreboard + summary и приложите их к записи о релизе.
4. Проверьте дашборды телеметрии с дежурным инженером перед переходом к следующей фазе.

## 4. Наблюдаемость и incident hooks

- **Метрики:** Убедитесь, что Alertmanager мониторит `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` и `sorafs_orchestrator_retries_total`. Резкий всплеск обычно означает деградацию провайдера под нагрузкой.
- **Логи:** Направляйте таргеты `telemetry::sorafs.fetch.*` в общий лог-агрегатор. Настройте сохраненные поиски для `event=complete status=failed`, чтобы ускорить триаж.
- **Scoreboards:** Сохраняйте каждый artefact scoreboard в долговременное хранилище. JSON также служит доказательной трассой для compliance-проверок и поэтапных откатов.
- **Dashboards:** Клонируйте канонический Grafana-дэшборд (`docs/examples/sorafs_fetch_dashboard.json`) в продакшн-папку с правилами алертов из `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Коммуникации и документация

- Логируйте каждое изменение deny/boost в операционном changelog с отметкой времени, оператором, причиной и связанным инцидентом.
- Уведомляйте команды SDK при изменениях весов провайдеров или бюджетов ретраев, чтобы синхронизировать ожидания на стороне клиента.
- После завершения GA обновите `status.md` сводкой rollout и заархивируйте эту ссылку на runbook в релизных заметках.
