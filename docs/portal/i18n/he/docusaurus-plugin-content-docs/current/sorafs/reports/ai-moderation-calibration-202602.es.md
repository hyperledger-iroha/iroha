---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Informe de calibración de moderatión de IA (2026-02)
תקציר: בסיס נתונים כיול, לוח תוצאות ולוח תוצאות עבור MINFO-1.
---

# Informe de calibración de moderatión de IA - פבררו 2026

Este informe empaqueta los artefactos de calibración inaugurales para **MINFO-1**. אל
מערך נתונים, מניפסט y לוח התוצאות se produjeron el 2026-02-05, fueron revisados por el
consejo del Ministerio el 2026-02-10 y se anclaron en el DAG de gobernanza en la altura
`912044`.

## מניפין של מערך הנתונים

- **הפניה למערך נתונים:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **שבלול:** `ai-moderation-calibration-202602`
- **כניסות:** מניפסט 480, נתח 12,800, מטא נתונים 920, אודיו 160
- **תערובת תווית:** בטוחה 68%, חשוד 19%, הסלמה 13%
- **תמצית חפצים:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **הפצה:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

El manifiesto completo vive en `docs/examples/ai_moderation_calibration_manifest_202602.json`
y contiene la firma de gobernanza más el hash del runner capturado en el momento del
לשחרר.

## קורות חיים של לוח התוצאות

Las calibraciones se ejecutaron con opset 17 y el pipeline de semillas determinísticas. אל
JSON שלם של לוח התוצאות (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
registra los hashes y digests de telemetry; la tabla siguiente destaca las métricas más
חשובים.

| Modelo (משפחה) | ברייר | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 בטיחות (ראייה) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (מולטימודאלי) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| אנסמבל תפיסתי (תפיסתי) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

מדדים משולבים: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. La distribución de
veredictos en la ventana de calibración fue pass 91.2%, הסגר 6.8%,
הסלמה ב-2.0%, coincidendo con las expectativas de política registradas en el
resumen del manifest. El backlog de falsos positivos se mantuvo en cero y el
ציון הסחף (7.1%) הוא תוצאה של 20%.

## Umbrales y aprobación

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- הצעת ממשל: `MINFO-2026-02-07`
- חתום על ידי `ministry-council-seat-03` ב-`2026-02-10T11:33:12Z`

CI almacenó el bundle firmado en `artifacts/ministry/ai_moderation/2026-02/`
junto con los binarios del mation runner. El digest del manifest y los hashes
del לוח התוצאות הקדמי ותן התייחסות לתקופות אודיטוריות y apelaciones.

## לוחות מחוונים והתראות

Los SRE de moderatión deben importar el לוח המחוונים de Grafana en
`dashboards/grafana/ministry_moderation_overview.json` y las reglas de alertas de
Prometheus en `dashboards/alerts/ministry_moderation_rules.yml` (la cobertura de tests
vive en `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). אסטוס
artefactos emiten alertas por bloqueos de ingesta, picos de drift y crecimiento de
la cola de quarantine, cumpliendo los requisitos de monitoreo indicados en la
[מפרט AI Moderation Runner](../../ministry/ai-moderation-runner.md).