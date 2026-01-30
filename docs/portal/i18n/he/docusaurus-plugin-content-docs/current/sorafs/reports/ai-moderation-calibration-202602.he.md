---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a55aaa9bd22a7470f92f3bc86fe5d3cb04dccdf72d921c116fd974d436fbb7c2
source_last_modified: "2025-11-14T04:43:22.175122+00:00"
translation_last_reviewed: 2026-01-30
---

# דוח כיול למודרציה של AI - פברואר 2026

דוח זה מאגד את ארטיפקטי הכיול הראשונים עבור **MINFO-1**. ה-dataset, ה-manifest וה-scoreboard
יוצרו ב-2026-02-05, נבדקו על ידי מועצת המשרד ב-2026-02-10, ועוגנו ב-governance DAG בגובה
`912044`.

## Manifest של מערך הנתונים

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

ה-manifest המלא נמצא ב-`docs/examples/ai_moderation_calibration_manifest_202602.json`
וכולל את חתימת הממשל יחד עם ה-hash של ה-runner שנלכד בזמן השחרור.

## סיכום scoreboard

הכיולים רצו עם opset 17 ו-pipeline זרעים דטרמיניסטי. ה-JSON המלא של scoreboard
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) מתעד hashes ו-digests
של telemetry; הטבלה למטה מדגישה את המדדים החשובים ביותר.

| מודל (משפחה) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| ------------ | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

מדדים משולבים: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. חלוקת
הוורדיקטים בחלון הכיול הייתה pass 91.2%, quarantine 6.8%,
escalate 2.0%, בהתאם לציפיות המדיניות שנרשמו בסיכום ה-manifest. backlog
של false-positive נשאר באפס, וה-drift score (7.1%) היה נמוך בהרבה מסף ההתראה 20%.

## ספים ואישור

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

CI שמרה את ה-bundle החתום ב-`artifacts/ministry/ai_moderation/2026-02/`
לצד הבינארים של moderation runner. ה-digest של ה-manifest וה-hashes של scoreboard
לעיל חייבים להיות מוזכרים במהלך ביקורות וערעורים.

## דשבורדים והתראות

SREs של מודרציה צריכים לייבא את Grafana dashboard ב-
`dashboards/grafana/ministry_moderation_overview.json` ואת חוקי ההתראות של Prometheus
ב-`dashboards/alerts/ministry_moderation_rules.yml` (כיסוי הבדיקות נמצא ב-
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). ארטיפקטים אלה
מפיקים התראות על עצירות ingestion, קפיצות drift וגידול בתור quarantine, ועומדים
בדרישות הניטור המוזכרות ב-
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md).
