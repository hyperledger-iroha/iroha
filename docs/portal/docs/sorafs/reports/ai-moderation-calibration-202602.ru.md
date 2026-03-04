---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6066e49abf73a788490f9d6a738d90376b13dde4150509e81431508ffaaaf916
source_last_modified: "2025-11-10T16:27:41.586824+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Отчет о калибровке модерации ИИ (2026-02)
summary: Базовый калибровочный набор данных, пороги и scoreboard для первого релиза управления MINFO-1.
---

# Отчет о калибровке модерации ИИ - Февраль 2026

Этот отчет содержит начальные артефакты калибровки для **MINFO-1**. Датасет,
manifest и scoreboard были сформированы 2026-02-05, рассмотрены советом
министерства 2026-02-10 и закреплены в governance DAG на высоте `912044`.

## Манифест датасета

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Полный manifest находится в `docs/examples/ai_moderation_calibration_manifest_202602.json`
и содержит подпись управления, а также hash runner, зафиксированный в момент
релиза.

## Сводка scoreboard

Калибровки запускались с opset 17 и детерминированным seed pipeline. Полный JSON
scoreboard (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
содержит hashes и digests telemetry; таблица ниже выделяет ключевые метрики.

| Модель (семейство) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| ----------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

Сводные метрики: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Распределение
вердиктов в окне калибровки составило pass 91.2%, quarantine 6.8%,
escalate 2.0%, что соответствует ожиданиям политики в сводке manifest.
Бэклог ложных срабатываний оставался нулевым, а drift score (7.1%) находился
далеко ниже порога тревоги 20%.

## Пороговые значения и согласование

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

CI сохранила подписанный bundle в `artifacts/ministry/ai_moderation/2026-02/`
вместе с бинарями moderation runner. Указанные выше digest manifest и hashes
scoreboard должны использоваться при аудитах и апелляциях.

## Дашборды и алерты

SRE по модерации должны импортировать Grafana dashboard из
`dashboards/grafana/ministry_moderation_overview.json` и правила алертов Prometheus
из `dashboards/alerts/ministry_moderation_rules.yml` (покрытие тестами находится в
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Эти артефакты
генерируют алерты при блокировках ingestion, всплесках drift и росте очереди
quarantine, выполняя требования мониторинга, указанные в
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md).
