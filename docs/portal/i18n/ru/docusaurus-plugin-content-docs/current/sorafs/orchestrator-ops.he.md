---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/orchestrator-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56d6cd0ec538422e5ce7c0be4fe00bcf5e72ac0e9ae688b0652f693141a9f628
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: orchestrator-ops
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Runbook по эксплуатации оркестратора SoraFS
sidebar_label: Runbook оркестратора
description: Пошаговое операционное руководство по развёртыванию, мониторингу и откату мульти-источникового оркестратора.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Держите обе копии синхронизированными, пока набор документации Sphinx не будет полностью мигрирован.
:::

Этот runbook проводит SRE через подготовку, развёртывание и эксплуатацию мульти-источникового fetch-оркестратора. Он дополняет руководство разработчика процедурами, рассчитанными на прод-роллауты, включая поэтапное включение и занесение пиров в чёрный список.

> **См. также:** [Runbook по мульти-источниковому rollout](./multi-source-rollout.md) посвящён волнам развёртывания на уровне флота и экстренному отклонению провайдеров. Используйте его для координации governance / staging, а этот документ — для повседневной эксплуатации оркестратора.

## 1. Предстартовый чек-лист

1. **Собрать входные данные от провайдеров**
   - Последние анонсы провайдеров (`ProviderAdvertV1`) и снимок телеметрии для целевого флота.
   - План payload (`plan.json`), полученный из тестируемого манифеста.
2. **Сформировать детерминированный scoreboard**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - Убедитесь, что `artifacts/scoreboard.json` содержит каждого прод-провайдера как `eligible`.
   - Архивируйте JSON-сводку вместе со scoreboard; аудиторы опираются на счётчики ретраев чанков при сертификации запроса на изменение.
3. **Dry-run с fixtures** — Выполните ту же команду на публичных fixtures из `docs/examples/sorafs_ci_sample/`, чтобы убедиться, что бинарник оркестратора соответствует ожидаемой версии, прежде чем трогать прод-payloads.

## 2. Процедура поэтапного rollout

1. **Канарейка (≤2 провайдера)**
   - Пересоберите scoreboard и запустите с `--max-peers=2`, чтобы ограничить оркестратор небольшим подмножеством.
   - Мониторьте:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Продолжайте, когда доля ретраев держится ниже 1% для полного fetch манифеста и ни один провайдер не накапливает ошибок.
2. **Этап разгона (50% провайдеров)**
   - Увеличьте `--max-peers` и перезапустите со свежим снимком телеметрии.
   - Сохраняйте каждый запуск с `--provider-metrics-out` и `--chunk-receipts-out`. Храните артефакты ≥7 дней.
3. **Полный rollout**
   - Уберите `--max-peers` (или установите его на полный набор eligible).
   - Включите режим оркестратора в клиентских деплоях: распространяйте сохранённый scoreboard и JSON-конфиг через систему управления конфигурацией.
   - Обновите дашборды, чтобы показывать `sorafs_orchestrator_fetch_duration_ms` p95/p99 и гистограммы ретраев по регионам.

## 3. Блокировка и усиление пиров

Используйте overrides политики скоринга в CLI, чтобы разбирать проблемных провайдеров без ожидания обновлений governance.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` исключает указанный alias из рассмотрения в текущей сессии.
- `--boost-provider=<alias>=<weight>` повышает вес провайдера в планировщике. Значения добавляются к нормализованному весу scoreboard и применяются только к локальному запуску.
- Зафиксируйте overrides в инцидент-тикете и приложите JSON-выходы, чтобы ответственная команда могла согласовать состояние после устранения первопричины.

Для постоянных изменений обновите исходную телеметрию (пометьте нарушителя как penalised) или пересоздайте advert с обновлёнными бюджетами потоков до очистки CLI-override.

## 4. Разбор сбоев

Когда fetch падает:

1. Перед повторным запуском соберите следующие артефакты:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Проверьте `session.summary.json` на человекочитаемую строку ошибки:
   - `no providers were supplied` → проверьте пути к провайдерам и объявления.
   - `retry budget exhausted ...` → увеличьте `--retry-budget` или исключите нестабильных пиров.
   - `no compatible providers available ...` → проверьте метаданные диапазонных возможностей провайдера-нарушителя.
3. Сопоставьте имя провайдера с `sorafs_orchestrator_provider_failures_total` и создайте тикет-наблюдение, если метрика резко растёт.
4. Воспроизведите fetch офлайн с `--scoreboard-json` и захваченной телеметрией, чтобы детерминированно воспроизвести сбой.

## 5. Rollback

Чтобы откатить rollout оркестратора:

1. Распространите конфигурацию с `--max-peers=1` (фактически отключает мульти-источниковое планирование) или верните клиентов на устаревший одноисточниковый fetch-путь.
2. Уберите любые overrides `--boost-provider`, чтобы scoreboard вернулся к нейтральному весу.
3. Продолжайте собирать метрики оркестратора минимум сутки, чтобы подтвердить отсутствие оставшихся fetch-операций.

Дисциплинированный сбор артефактов и поэтапные rollouts гарантируют безопасную эксплуатацию мульти-источникового оркестратора на разнородных флотах провайдеров при сохранении требований по наблюдаемости и аудиту.
