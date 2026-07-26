---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: דוח כיול בינה מלאכותית (2026-02)
summary: MINFO-1 کے پہلے governance release کے لئے baseline calibration dataset، thresholds اور scoreboard۔
---

# דוח כיול בינה מלאכותית - פירורי 2026

یہ رپورٹ **MINFO-1** کے لئے ابتدائی calibration artefacts کو پیک کرتی ہے۔ מערך נתונים, מניפסט או לוח תוצאות
2026-02-05 کو تیار کیے گئے، 2026-02-10 کو Ministry council نے ریویو کیا، اور governance DAG میں height
`912044` پر anchor کیے گئے۔

## מניפסט מערך נתונים

- **הפניה למערך נתונים:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **שבלול:** `ai-moderation-calibration-202602`
- **כניסות:** מניפסט 480, נתח 12,800, מטא נתונים 920, אודיו 160
- **תערובת תווית:** בטוחה 68%, חשוד 19%, הסלמה 13%
- **תמצית חפצים:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **הפצה:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

مکمل manifest `docs/examples/ai_moderation_calibration_manifest_202602.json` میں موجود ہے
اور اس میں governance signature کے ساتھ release کے وقت captured runner hash شامل ہے۔

## סיכום לוח התוצאות

Calibrations opset 17 اور deterministic seed pipeline کے ساتھ چلائی گئیں۔ לוח תוצאות JSON
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hashes اور telemetry digests ریکارڈ کرتا ہے؛
نیچے دی گئی table اہم metrics دکھاتی ہے۔

| דוגמנית (משפחה) | ברייר | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| ------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 בטיחות (ראייה) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (מולטימודאלי) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| אנסמבל תפיסתי (תפיסתי) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

מדדים משולבים: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Calibration window میں verdict distribution
לעבור 91.2%, הסגר 6.8%, הסלמה 2.0%
False-positive backlog صفر رہا، اور drift score (7.1%) 20% alert threshold سے کافی نیچے تھا۔

## ספים או חתימה

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- הצעת ממשל: `MINFO-2026-02-07`
- חתום על ידי `ministry-council-seat-03` ב-`2026-02-10T11:33:12Z`

CI نے signed bundle کو `artifacts/ministry/ai_moderation/2026-02/` میں moderation runner binaries کے ساتھ محفوظ کیا۔
اوپر دیے گئے manifest digest اور scoreboard hashes کو audits اور appeals کے دوران refer کرنا ضروری ہے۔

## לוחות מחוונים והתראות

Moderation SREs کو Grafana dashboard
`dashboards/grafana/ministry_moderation_overview.json` אור Prometheus כללי התראה
`dashboards/alerts/ministry_moderation_rules.yml` امپورٹ کرنے چاہئیں
(test coverage `dashboards/alerts/tests/ministry_moderation_rules.test.yml` میں ہے)۔ 2 חפצים
ingest stalls، drift spikes اور quarantine queue کی growth کے لئے alerts emit کرتے ہیں، اور
[מפרט AI Moderation Runner](../../ministry/ai-moderation-runner.md) ניטור מיומן
requirements پوری کرتے ہیں۔