---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: storage-capacity-marketplace
title: Маркетплейс емкости хранения SoraFS
sidebar_label: Маркетплейс емкости
description: План SF-2c для маркетплейса емкости, replication orders, телеметрии и governance hooks.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/storage_capacity_marketplace.md`. Держите обе копии синхронными, пока активна устаревшая документация.
:::

# Маркетплейс емкости хранения SoraFS (черновик SF-2c)

Пункт roadmap SF-2c вводит управляемый marketplace, где providers хранилища
декларируют коммитнутую емкость, получают replication orders и зарабатывают fees
пропорционально предоставленной доступности. Этот документ очерчивает deliverables
для первого релиза и разбивает их на actionable треки.

## Цели

- Фиксировать обязательства providers по емкости (общие байты, лимиты по lane, срок действия)
  в проверяемой форме, пригодной для governance, транспорта SoraNet и Torii.
- Распределять pins между providers согласно заявленной емкости, stake и policy-ограничениям,
  сохраняя детерминированное поведение.
- Измерять доставку хранения (успех репликации, uptime, proofs целостности) и
  экспортировать телеметрию для распределения fees.
- Предоставлять процессы revocation и dispute, чтобы нечестные providers могли быть
  наказаны или удалены.

## Доменные концепции

| Концепция | Описание | Первичный deliverable |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito payload, описывающий ID provider, поддержку профиля chunker, коммитнутые GiB, лимиты по lane, hints по pricing, staking commitment и срок действия. | Схема + валидатор в `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Инструкция, выпущенная governance, назначающая CID manifest одному или нескольким providers, включая уровень избыточности и метрики SLA. | Norito схема, общая для Torii + API смарт-контракта. |
| `CapacityLedger` | On-chain/off-chain registry, отслеживающий активные декларации емкости, replication orders, метрики производительности и накопление fees. | Модуль смарт-контракта или off-chain stub сервиса с детерминированным snapshot. |
| `MarketplacePolicy` | Политика governance, определяющая минимальный stake, требования аудита и кривые штрафов. | Config struct в `sorafs_manifest` + документ governance. |

### Реализованные схемы (статус)

## Разбиение работ

### 1. Слой схем и реестра

| Задача | Owner(s) | Примечания |
|------|----------|-------|
| Определить `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Storage Team / Governance | Использовать Norito; включить семантическое версионирование и ссылки на capabilities. |
| Реализовать модули parser + validator в `sorafs_manifest`. | Storage Team | Обеспечить монотонные IDs, ограничения емкости, требования по stake. |
| Расширить metadata реестра chunker значением `min_capacity_gib` для каждого профиля. | Tooling WG | Помогает клиентам применять минимальные требования к hardware по профилю. |
| Подготовить документ `MarketplacePolicy`, описывающий admission guardrails и график штрафов. | Governance Council | Опубликовать в docs рядом с policy defaults. |

#### Определения схем (реализованы)

- `CapacityDeclarationV1` фиксирует подписанные обязательства емкости для каждого provider, включая канонические handles chunker, ссылки на capabilities, опциональные caps по lane, hints по pricing, окна валидности и metadata. Валидация обеспечивает ненулевой stake, канонические handles, дедуплицированные aliases, caps по lane в пределах заявленного total и монотонный учет GiB.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` связывает manifests с назначениями, выпущенными governance, с целями избыточности, порогами SLA и гарантиями на assignment; валидаторы обеспечивают канонические handles chunker, уникальные providers и ограничения по deadline до того, как Torii или registry примут order.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` описывает snapshots эпох (заявленные vs использованные GiB, счетчики репликации, проценты uptime/PoR), которые питают распределение fees. Проверки границ удерживают использование внутри деклараций, а проценты - в пределах 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Общие helpers (`CapacityMetadataEntry`, `PricingScheduleV1`, валидаторы lane/assignment/SLA) дают детерминированную проверку ключей и репорты ошибок, которые могут переиспользовать CI и downstream tooling.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` теперь публикует on-chain snapshot через `/v1/sorafs/capacity/state`, объединяя декларации providers и записи fee ledger за детерминированным Norito JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Покрытие валидации проверяет соблюдение канонических handles, обнаружение дубликатов, границы по lane, guards назначения репликации и проверки диапазонов телеметрии, чтобы регрессии всплывали сразу в CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Operator tooling: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` конвертирует человекочитаемые specs в канонические Norito payloads, base64 blobs и JSON summaries, чтобы операторы могли подготовить fixtures `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` и replication order fixtures с локальной валидацией.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Reference fixtures живут в `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) и генерируются через `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Интеграция control plane

| Задача | Owner(s) | Примечания |
|------|----------|-------|
| Добавить обработчики Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` с Norito JSON payloads. | Torii Team | Зеркалировать логику валидации; переиспользовать Norito JSON helpers. |
| Протолкнуть snapshots `CapacityDeclarationV1` в metadata scoreboard orchestrator и планы fetch gateway. | Tooling WG / Orchestrator team | Расширить `provider_metadata` ссылками на capacity, чтобы мульти-источниковый scoring соблюдал лимиты по lane. |
| Подавать replication orders в clients orchestrator/gateway для управления assignments и hints failover. | Networking TL / Gateway team | Scoreboard builder потребляет подписанные governance replication orders. |
| CLI tooling: расширить `sorafs_cli` командами `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | Предоставить детерминированный JSON + outputs scoreboard. |

### 3. Политика marketplace и governance

| Задача | Owner(s) | Примечания |
|------|----------|-------|
| Утвердить `MarketplacePolicy` (минимальный stake, мультипликаторы штрафов, периодичность аудита). | Governance Council | Опубликовать в docs, зафиксировать историю ревизий. |
| Добавить governance hooks, чтобы Parliament мог approve, renew и revoke declarations. | Governance Council / Smart Contract team | Использовать Norito events + ingestion manifests. |
| Реализовать график штрафов (снижение fees, slashing bond), привязанный к телеметрируемым нарушениям SLA. | Governance Council / Treasury | Согласовать с outputs settlement `DealEngine`. |
| Документировать процесс dispute и матрицу эскалации. | Docs / Governance | Сослаться на dispute runbook + helpers CLI. |

### 4. Metering и распределение fees

| Задача | Owner(s) | Примечания |
|------|----------|-------|
| Расширить ingest metering в Torii для приема `CapacityTelemetryV1`. | Torii Team | Валидировать GiB-hour, успех PoR, uptime. |
| Обновить pipeline metering `sorafs_node` для отчета по использованию на order + статистике SLA. | Storage Team | Согласовать с replication orders и handles chunker. |
| Pipeline settlement: конвертировать телеметрию + репликацию в payouts, номинированные в XOR, выдавать governance-ready summaries и фиксировать состояние ledger. | Treasury / Storage Team | Подключить к Deal Engine / Treasury exports. |
| Экспортировать dashboards/alerts для здоровья metering (backlog ingest, устаревшая телеметрия). | Observability | Расширить пакет Grafana, на который ссылаются SF-6/SF-7. |

- Torii теперь публикует `/v1/sorafs/capacity/telemetry` и `/v1/sorafs/capacity/state` (JSON + Norito), чтобы операторы могли отправлять telemetry snapshots по эпохам, а инспекторы - получать канонический ledger для аудита или упаковки доказательств.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- Интеграция `PinProviderRegistry` гарантирует, что replication orders доступны через тот же endpoint; helpers CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) теперь валидируют/публикуют телеметрию из automation runs с детерминированным hashing и разрешением alias.
- Metering snapshots формируют записи `CapacityTelemetrySnapshot`, закрепленные за snapshot `metering`, а Prometheus exports питают готовый к импорту Grafana board в `docs/source/grafana_sorafs_metering.json`, чтобы команды биллинга отслеживали накопление GiB-hour, прогнозируемые nano-SORA fees и соблюдение SLA в реальном времени.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Когда включено metering smoothing, snapshot включает `smoothed_gib_hours` и `smoothed_por_success_bps`, чтобы операторы могли сравнивать EMA-трендовые значения с сырыми счетчиками, которые governance использует для payouts.【crates/sorafs_node/src/metering.rs:401】

### 5. Обработка dispute и revocation

| Задача | Owner(s) | Примечания |
|------|----------|-------|
| Определить payload `CapacityDisputeV1` (заявитель, evidence, целевой provider). | Governance Council | Norito схема + валидатор. |
| Поддержка CLI для подачи disputes и ответов (с attachments evidence). | Tooling WG | Обеспечить детерминированный hashing пакета evidence. |
| Добавить автоматические проверки повторяющихся нарушений SLA (auto-escalate в dispute). | Observability | Пороги alert и governance hooks. |
| Документировать playbook revocation (grace period, эвакуация pinned data). | Docs / Storage Team | Сослаться на policy doc и operator runbook. |

## Требования к тестированию и CI

- Юнит-тесты для всех новых валидаторов схем (`sorafs_manifest`).
- Интеграционные тесты, которые симулируют: декларация → replication order → metering → payout.
- CI workflow для регенерации sample деклараций/телеметрии емкости и проверки синхронизации подписей (расширить `ci/check_sorafs_fixtures.sh`).
- Load tests для registry API (симулировать 10k providers, 100k orders).

## Телеметрия и дашборды

- Панели дашборда:
  - Декларированная vs использованная емкость по provider.
  - Backlog replication orders и средняя задержка назначения.
  - Соответствие SLA (uptime %, частота успеха PoR).
  - Накопление fees и штрафы по эпохам.
- Alerts:
  - Provider ниже минимальной заявленной емкости.
  - Replication order завис более чем на SLA.
  - Сбои metering pipeline.

## Документационные материалы

- Руководство оператора по декларации емкости, продлению обязательств и мониторингу использования.
- Руководство по governance для утверждения деклараций, выдачи orders, обработки disputes.
- API reference для endpoints емкости и формата replication order.
- Marketplace FAQ для разработчиков.

## Чеклист готовности к GA

Пункт roadmap **SF-2c** блокирует production rollout до появления конкретных доказательств
по учету, обработке disputes и онбордингу. Используйте артефакты ниже, чтобы держать критерии
приемки в синхроне с реализацией.

### Ночной учет и сверка XOR
- Экспортируйте snapshot состояния емкости и экспорт XOR ledger за тот же период, затем запустите:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Хелпер завершится с ненулевым кодом при недостающих/переплаченных settlement или штрафах и
  выдаст текстовый файл Prometheus summary.
- Alert `SoraFSCapacityReconciliationMismatch` (в `dashboards/alerts/sorafs_capacity_rules.yml`)
  срабатывает, когда reconciliation метрики сообщают о расхождениях; dashboards лежат в
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Архивируйте JSON summary и hashes в `docs/examples/sorafs_capacity_marketplace_validation/`
  вместе с governance packets.

### Доказательства dispute и slashing
- Подавайте disputes через `sorafs_manifest_stub capacity dispute` (tests:
  `cargo test -p sorafs_car --test capacity_cli`), чтобы payloads оставались каноничными.
- Запускайте `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` и наборы
  штрафов (`record_capacity_telemetry_penalises_persistent_under_delivery`), чтобы доказать
  детерминированное воспроизведение disputes и slashes.
- Следуйте `docs/source/sorafs/dispute_revocation_runbook.md` для захвата доказательств и
  эскалации; привязывайте approvals strike обратно в validation report.

### Смоук-тесты онбординга и выхода providers
- Регенерируйте artefacts деклараций/телеметрии через `sorafs_manifest_stub capacity ...` и
  прогоняйте CLI tests перед подачей (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Отправляйте через Torii (`/v1/sorafs/capacity/declare`), затем фиксируйте
  `/v1/sorafs/capacity/state` плюс скриншоты Grafana. Следуйте flow выхода в
  `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Архивируйте подписанные artefacts и reconciliation outputs внутри
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Зависимости и последовательность

1. Завершить SF-2b (admission policy) - marketplace опирается на проверенных providers.
2. Реализовать слой схемы + registry (этот документ) перед интеграцией Torii.
3. Завершить metering pipeline до включения выплат.
4. Финальный шаг: включить governance-контролируемое распределение fees после проверки metering data в staging.

Прогресс следует отслеживать в roadmap со ссылками на этот документ. Обновляйте roadmap после того,
как каждая основная секция (схемы, control plane, интеграция, metering, обработка disputes) достигнет
feature complete статуса.
