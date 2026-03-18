---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Отчет о калибровке модерации ИИ (2026-02)
תקציר: Базовый калибровочный набор данных, пороги и לוח התוצאות ל-первого релиза управления MINFO-1.
---

# Отчет о калибровке модерации ИИ - פברואר 2026

Этот отчет содержит начальные артефакты калибровки ל-**MINFO-1**. Датасет,
מניפסט ולוח תוצאות были сформированы 2026-02-05, рассмотрены советом
מיניסטרסטווה 2026-02-10 и закреплены в governance DAG на высоте `912044`.

## Манифест датасета

- **הפניה למערך נתונים:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **שבלול:** `ai-moderation-calibration-202602`
- **כניסות:** מניפסט 480, נתח 12,800, מטא נתונים 920, אודיו 160
- **תערובת תווית:** בטוחה 68%, חשוד 19%, הסלמה 13%
- **תמצית חפצים:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **הפצה:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Полный manifest находится в `docs/examples/ai_moderation_calibration_manifest_202602.json`
и содержит подпись управления, а также hash runner, зафиксированный в момент
релиза.

## לוח התוצאות של Сводка

Калибровки запускались с opset 17 и детерминированным seed line. Полный JSON
לוח תוצאות (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
содержит hashes и digests טלמטריה; таблица ниже выделяет ключевые метрики.

| Модель (семейство) | ברייר | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| ------------------ | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 בטיחות (ראייה) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (מולטימודאלי) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| אנסמבל תפיסתי (תפיסתי) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

מדדים: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Распределение
вердиктов в окне калибровки составило עוברים 91.2%, הסגר 6.8%,
הסלמה ב-2.0%, что соответствует ожиданиям политики в сводке מניפסט.
Бэклог ложных срабатываний оставался нулевым, ציון סחף (7.1%) находился
דירוג נמוך של 20%.

## Пороговые значения и согласование

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- הצעת ממשל: `MINFO-2026-02-07`
- חתום על ידי `ministry-council-seat-03` ב-`2026-02-10T11:33:12Z`

CI сохранила подписанный חבילה ב-`artifacts/ministry/ai_moderation/2026-02/`
вместе с бинарями רץ מתינות. Указанные выше digest manifest и hashes
לוח התוצאות должны использоваться при аудитах и апелляциях.

## Дашборды и алерты

SRE по модерации должны импортировать Grafana לוח המחוונים из
`dashboards/grafana/ministry_moderation_overview.json` ו-Pравила алертов Prometheus
из `dashboards/alerts/ministry_moderation_rules.yml` (покрытие тестами находится в
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Эти артефакты
генерируют алерты при блокировках בליעה, всплесках סחף и росте очереди
הסגר, выполняя требования мониторинга, указанные в
[מפרט AI Moderation Runner](../../ministry/ai-moderation-runner.md).