---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Rapport de calibration de modération IA (2026-02)
תקציר: מערך נתונים של כיול בסיס, סידורים ולוח תוצאות עבור שחרור בכורה של מינהל MINFO-1.
---

# Rapport de calibration de modération IA - Février 2026

Ce rapport regrope les artefacts de calibration inauguraux pour **MINFO-1**. לה
מערך נתונים, le manifest et le scoreboard on été produits le 2026-02-05, revus par
le conseil du ministère le 2026-02-10, et ancrés dans le DAG de governance à la
hauteur `912044`.

## Manifeste du dataset

- **הפניה למערך נתונים:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **שבלול:** `ai-moderation-calibration-202602`
- **כניסות:** מניפסט 480, נתח 12,800, מטא נתונים 920, אודיו 160
- **תערובת תווית:** בטוחה 68%, חשוד 19%, הסלמה 13%
- **תמצית חפצים:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **הפצה:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Le manifest complet se trouve dans `docs/examples/ai_moderation_calibration_manifest_202602.json`
et contient la signature de gouvernance ainsi que le hash du runner capturé au
רגע דה לה שחרור.

## לוח התוצאות של קורות חיים

Les calirations on tourné avec l'opset 17 et le pipeline de graines déterministes. לה
JSON complet du לוח התוצאות (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
consigne les hashes et digests de telemetry; le tableau ci-dessous met en avant les
métriques les plus importantes.

| דגם (famille) | ברייר | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 בטיחות (ראייה) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (מולטימודאלי) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| אנסמבל תפיסתי (תפיסתי) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

שילובים של מטריקס: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. להפצה
des verdicts sur la fenêtre de calibration était pass 91.2%, הסגר 6.8%,
הסלמה ב-2.0%, ce qui correspond aux attentes de politique indiquées dans le
קורות חיים דו מניפסט. ה-backlog de faux positifs est resté à séro and le drift score
(7.1%) est resté bien en dessous du seuil d'alerte de 20%.

## Seuils et validation

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- הצעת ממשל: `MINFO-2026-02-07`
- חתום על ידי `ministry-council-seat-03` ב-`2026-02-10T11:33:12Z`

CI a stocké le bundle signné dans `artifacts/ministry/ai_moderation/2026-02/`
aux côtés des binaires du moderation runner. Le digest du manifest et les hashes
לוח התוצאות ci-dessus doivent être référencés lors des audits et des recours.

## לוחות מחוונים והתראות

Les SRE de modération doivent יבואן le לוח המחוונים Grafana situé à
`dashboards/grafana/ministry_moderation_overview.json` et les règles d'alerte
Prometheus ב-`dashboards/alerts/ministry_moderation_rules.yml` (la couverture
de tests se trouve dans `dashboards/alerts/tests/ministry_moderation_rules.test.yml`).
Ces Artefacts émettent des alertes pour les blockages d'ingestion, les pics de drift
et la croissance de la קובץ הסגר, סיפוק les exigences de monitoring
mentionnées dans la [מפרט AI Moderation Runner](../../ministry/ai-moderation-runner.md).