---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Relatorio de calibracao de moderacao de IA (2026-02)
תקציר: בסיס נתונים של קליברקאו, ספים ולוח תוצאות עבור שחרור ראשוני של MINFO-1.
---

# Relatorio de calibracao de moderacao de IA - Fevereiro 2026

Este relatorio empacota os artefatos de calibracao iniciais para **MINFO-1**. O
מערך נתונים, או מניפסט או לוח התוצאות פורם מוצרים ב-2026-02-05, revisados pelo
conselho do Ministerio em 2026-02-10 e ancorados no DAG de governanca na altura
`912044`.

## מניפסט לעשות מערך נתונים

- **הפניה למערך נתונים:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **שבלול:** `ai-moderation-calibration-202602`
- **כניסות:** מניפסט 480, נתח 12,800, מטא נתונים 920, אודיו 160
- **תערובת תווית:** בטוחה 68%, חשוד 19%, הסלמה 13%
- **תמצית חפצים:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **הפצה:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

O manifest completo fica em `docs/examples/ai_moderation_calibration_manifest_202602.json`
e contem a assinatura de governanca e o hash do runner capturado no momento do
לשחרר.

## לוח התוצאות של Resumo do

כמו calibracoes rodaram com opset 17 e o pipeline de seed deterministica. O
לוח התוצאות של JSON completo do (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
registra os hashes e digests de telemetry; a tabela abaixo destaca as metricas mais
חשובים.

| Modelo (משפחה) | ברייר | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 בטיחות (ראייה) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (מולטימודאלי) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| אנסמבל תפיסתי (תפיסתי) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

מדדים משולבים: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. A distribuicao de
vereditos na janela de calibracao foi pass 91.2%, הסגר 6.8%,
הסלמה ב-2.0%, alinhada com כצפוי לרישום פוליטיקה ללא חידוש
מתגלה. O backlog de falsos positiveos permaneceu em אפס, ו ציון הסחף (7.1%)
ficou bem abaixo do limiar de alerta de 20%.

## ספי חתימה אלקטרונית

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- הצעת ממשל: `MINFO-2026-02-07`
- חתום על ידי `ministry-council-seat-03` ב-`2026-02-10T11:33:12Z`

CI armazenou o bundle assinado em `artifacts/ministry/ai_moderation/2026-02/`
junto com os binarios do mation runner. O digest do manifest e os hashes do
לוח התוצאות acima devem ser referenciados durante auditorias e apelacoes.

## לוחות מחוונים והתראות

SREs de moderacao devem importer o לוח המחוונים Grafana em
`dashboards/grafana/ministry_moderation_overview.json` e as regras de alerta do
Prometheus em `dashboards/alerts/ministry_moderation_rules.yml` (a cobertura de
בדיקות Fica em `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Esses
artefatos emitem alertas עבור בליעת דוכנים, סחף קוצים e crescimento da fila de
הסגר
[מפרט AI Moderation Runner](../../ministry/ai-moderation-runner.md).