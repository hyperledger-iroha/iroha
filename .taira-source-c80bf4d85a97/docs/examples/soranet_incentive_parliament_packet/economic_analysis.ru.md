---
lang: ru
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-11-05T17:23:10.790688+00:00"
translation_last_reviewed: 2026-01-01
---

# Экономический анализ - 2025-10 -> 2025-11 Shadow Run

Исходный artefact: `docs/examples/soranet_incentive_shadow_run.json` (подпись + публичный ключ в том же каталоге). Симуляция воспроизвела 60 эпох на relay, закрепив reward engine на `RewardConfig`, записанном в `reward_config.json`.

## Сводка распределения

- **Суммарные выплаты:** 5,160 XOR за 360 вознагражденных эпох.
- **Огибающая справедливости:** коэффициент Джини 0.121; доля топ relay 23.26%
  (значительно ниже governance guardrail 30%).
- **Доступность:** среднее по флоту 96.97%, все relays оставались выше 94%.
- **Полоса пропускания:** среднее по флоту 91.20%, минимальный показатель 87.23%
  во время планового обслуживания; штрафы применялись автоматически.
- **Шум compliance:** наблюдались 9 warning эпох и 3 suspensions, переведенные
  в снижение выплат; ни один relay не превысил предел 12 warnings.
- **Операционная гигиена:** snapshots метрик не пропускались из-за
  отсутствующих config/bonds/дубликатов; ошибок калькулятора не было.

## Наблюдения

- Suspensions соответствуют эпохам, когда relays входили в режим обслуживания. Payout engine
  выдавал нулевые выплаты для этих эпох, сохраняя audit trail в JSON shadow-run.
- Warning штрафы сократили выплаты на 2%; распределение продолжает сходиться благодаря
  весам uptime/bandwidth (650/350 per mille).
- Вариация bandwidth следует анонимизированной heatmap guard. Самый низкий результат
  (`6666...6666`) сохранил 620 XOR за окно, выше пола 0.6x.
- Латентно-чувствительные алерты (`SoranetRelayLatencySpike`) оставались ниже порогов
  warning на протяжении окна; соответствующие дашборды сохранены в
  `dashboards/grafana/soranet_incentives.json`.

## Рекомендуемые действия перед GA

1. Продолжать ежемесячные shadow replays и обновлять набор artefacts и этот анализ,
   если меняется состав флота.
2. Гейтировать автоматические выплаты на наборе Grafana алертов из roadmap
   (`dashboards/alerts/soranet_incentives_rules.yml`); добавлять screenshots в
   governance minutes при запросе обновления.
3. Перезапускать экономический стресс-тест, если базовая награда, веса uptime/bandwidth или
   штраф compliance меняются на >=10%.
