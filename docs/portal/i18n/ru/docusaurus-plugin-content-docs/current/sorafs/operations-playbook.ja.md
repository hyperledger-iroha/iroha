---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 320e5fd4d20a7c0f6418e25d68437977d3843ff3c0a44862268fda072a97cc15
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
id: operations-playbook
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает ранбук, поддерживаемый в `docs/source/sorafs_ops_playbook.md`. Держите обе копии синхронизированными, пока набор документации Sphinx полностью не будет мигрирован.
:::

## Ключевые ссылки

- Активы наблюдаемости: используйте дашборды Grafana в `dashboards/grafana/` и правила алертов Prometheus в `dashboards/alerts/`.
- Каталог метрик: `docs/source/sorafs_observability_plan.md`.
- Поверхности телеметрии оркестратора: `docs/source/sorafs_orchestrator_plan.md`.

## Матрица эскалации

| Приоритет | Примеры триггеров | Основной on-call | Резерв | Примечания |
|-----------|-------------------|-----------------|--------|------------|
| P1 | Глобальная остановка gateway, уровень отказов PoR > 5% (15 мин), backlog репликации удваивается каждые 10 мин | Storage SRE | Observability TL | Подключите совет по governance, если влияние превышает 30 минут. |
| P2 | Нарушение регионального SLO по latency gateway, всплеск retries оркестратора без влияния на SLA | Observability TL | Storage SRE | Продолжайте rollout, но заблокируйте новые manifests. |
| P3 | Некритичные алерты (staleness manifests, емкость 80–90%) | Intake triage | Ops guild | Исправить в следующий рабочий день. |

## Отключение gateway / деградация доступности

**Обнаружение**

- Алерты: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Дашборд: `dashboards/grafana/sorafs_gateway_overview.json`.

**Немедленные действия**

1. Подтвердите масштаб (один провайдер или весь флот) по панели request-rate.
2. Переключите маршрутизацию Torii на здоровых провайдеров (если multi-provider), переключив `sorafs_gateway_route_weights` в ops-конфиге (`docs/source/sorafs_gateway_self_cert.md`).
3. Если затронуты все провайдеры, включите fallback “direct fetch” для клиентов CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- Проверьте утилизацию stream token относительно `sorafs_gateway_stream_token_limit`.
- Посмотрите логи gateway на TLS или admission ошибки.
- Запустите `scripts/telemetry/run_schema_diff.sh`, чтобы убедиться, что экспортируемая gateway схема соответствует ожидаемой версии.

**Варианты ремедиации**

- Перезапускайте только затронутый процесс gateway; избегайте перезапуска всего кластера, если не падают несколько провайдеров.
- Временно увеличьте лимит stream token на 10–15%, если подтверждена насыщенность.
- Повторно выполните self-cert (`scripts/sorafs_gateway_self_cert.sh`) после стабилизации.

**Post-incident**

- Оформите постмортем P1 с помощью `docs/source/sorafs/postmortem_template.md`.
- Запланируйте последующий хаос-дрилл, если ремедиация опиралась на ручные действия.

## Всплеск отказов proof (PoR / PoTR)

**Обнаружение**

- Алерты: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Дашборд: `dashboards/grafana/sorafs_proof_integrity.json`.
- Телеметрия: `torii_sorafs_proof_stream_events_total` и события `sorafs.fetch.error` с `provider_reason=corrupt_proof`.

**Немедленные действия**

1. Заморозьте прием новых manifests, пометив реестр manifests (`docs/source/sorafs/manifest_pipeline.md`).
2. Уведомите Governance о приостановке стимулов для затронутых провайдеров.

**Triage**

- Проверьте глубину очереди PoR challenge относительно `sorafs_node_replication_backlog_total`.
- Валидируйте pipeline проверки proof (`crates/sorafs_node/src/potr.rs`) для последних деплоев.
- Сравните версии firmware провайдеров с реестром операторов.

**Варианты ремедиации**

- Запустите PoR replays через `sorafs_cli proof stream` с последним manifest.
- Если proofs стабильно падают, исключите провайдера из активного набора через обновление реестра governance и принудительное обновление scoreboards оркестратора.

**Post-incident**

- Запустите сценарий хаос-дрилла PoR до следующего продакшен-деплоя.
- Зафиксируйте выводы в шаблоне постмортема и обновите checklist квалификации провайдеров.

## Задержка репликации / рост backlog

**Обнаружение**

- Алерты: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Импортируйте
  `dashboards/alerts/sorafs_capacity_rules.yml` и выполните
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  до промоушена, чтобы Alertmanager отражал документированные пороги.
- Дашборд: `dashboards/grafana/sorafs_capacity_health.json`.
- Метрики: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Немедленные действия**

1. Определите масштаб backlog (один провайдер или весь флот) и приостановите несущественные задачи репликации.
2. Если backlog локальный, временно переназначьте новые заказы на альтернативных провайдеров через scheduler репликации.

**Triage**

- Проверьте телеметрию оркестратора на всплески retries, которые могут раздувать backlog.
- Убедитесь, что у целей хранения достаточно headroom (`sorafs_node_capacity_utilisation_percent`).
- Проверьте последние изменения конфигурации (обновления chunk profile, cadence proofs).

**Варианты ремедиации**

- Запустите `sorafs_cli` с опцией `--rebalance` для перераспределения контента.
- Масштабируйте replication workers горизонтально для затронутого провайдера.
- Инициируйте manifest refresh для выравнивания окон TTL.

**Post-incident**

- Запланируйте capacity drill, фокусированный на сбоях из-за насыщения провайдеров.
- Обновите документацию SLA репликации в `docs/source/sorafs_node_client_protocol.md`.

## Периодичность хаос-дриллов

- **Ежеквартально**: совмещенная симуляция остановки gateway + retry storm оркестратора.
- **Два раза в год**: инъекция сбоев PoR/PoTR на двух провайдерах с восстановлением.
- **Ежемесячная spot-проверка**: сценарий задержки репликации с использованием staging manifests.
- Отслеживайте drills в общем runbook log (`ops/drill-log.md`) через:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Проверьте лог перед коммитами с помощью:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Используйте `--status scheduled` для будущих drills, `pass`/`fail` для завершенных запусков и `follow-up`, если остались открытые действия.
- Перезапишите назначение через `--log` для dry-run или автоматической проверки; без него скрипт продолжит обновлять `ops/drill-log.md`.

## Шаблон постмортема

Используйте `docs/source/sorafs/postmortem_template.md` для каждого инцидента P1/P2 и для ретроспектив хаос-дриллов. Шаблон покрывает таймлайн, количественную оценку влияния, факторы, корректирующие действия и задачи последующей валидации.
