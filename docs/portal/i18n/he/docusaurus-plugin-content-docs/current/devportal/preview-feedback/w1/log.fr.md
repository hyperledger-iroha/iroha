---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-log
כותרת: Journal feedback et telemetrie W1
sidebar_label: Journal W1
תיאור: הסגלים מסכימים, מחסומים טלמטריים ומבקרים הערות יוצקים שותפים מעורפלים בבכורה.
---

כתב העת Ce conserve le roster des invitations, les checkpoints de telemetrie ו-le feedback reviewers pour le
**שותפים מקדימים W1** qui accompagne les taches d'acceptation dans
[`preview-feedback/w1/plan.md`](./plan.md) et l'entree du tracker de vague dans
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Mettez-le a jour quand une invitation est avoyee,
qu'un snapshot de telemetrie est registre, ou qu'un item de feedback est trie afin que les reviewers governance puissent
rejouer les preuves sans courir apres des tickets externes.

## סגל הקבוצה

| מזהה שותף | כרטיס דה דרישה | NDA recu | הזמנת שליח (UTC) | Ack/כניסה ראשונית (UTC) | סטטוט | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | בסדר 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | טרמין 2025-04-26 | soraps-op-01; מתזמר concentre sur les preuves de parite de docs. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | בסדר 2025-04-03 | 12-04-2025 15:03 | 12-04-2025 15:15 | טרמין 2025-04-26 | soraps-op-02; a valide les cross-links Norito/telemetrie. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | בסדר 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | טרמין 2025-04-26 | soraps-op-03; a execute des drills de failover multi-source. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | בסדר 2025-04-04 | 12-04-2025 15:09 | 2025-04-12 15:21 | טרמין 2025-04-26 | torii-int-01; revue du cookbook Torii `/v2/pipeline` + נסה את זה. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | בסדר 2025-04-05 | 12-04-2025 15:12 | 2025-04-12 15:23 | טרמין 2025-04-26 | torii-int-02; a accompagne la mise a jour de capture נסה את זה (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | בסדר 2025-04-05 | 12-04-2025 15:15 | 2025-04-12 15:26 | טרמין 2025-04-26 | sdk-partner-01; משוב ספרי בישול JS/Swift + בדיקות שפיות גשר ISO. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | בסדר 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | טרמין 2025-04-26 | sdk-partner-02; תאימות תקף 2025-04-11, מוקד על הערות Connect/telemetrie. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | בסדר 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | טרמין 2025-04-26 | gateway-ops-01; audit du guide ops gateway + Flux proxy נסה זאת אנונימי. |

Renseignez **הזמן נשלח** et **Ack** des que l'email sortant est emis.
Ancrez les heures au תכנון UTC מוגדר בתוכנית W1.

## טלמטריה של מחסומים| Horodatage (UTC) | לוחות מחוונים / בדיקות | אחראי | תוצאות | חפץ |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Tout vert | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06-04-2025 18:20 | תמלול `npm run manage:tryit-proxy -- --stage preview-w1` | אופס | מבוים | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | לוחות מחוונים ci-dessus + `probe:portal` | Docs/DevRel + Ops | הזמנה מראש של תמונת מצב, רגרסיה אקווני | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | לוחות מחוונים ci-dessus + proxy diff de latence נסה זאת | Docs/DevRel lead | מחסום milieu valide (0 התראות; אחזור נסה את זה p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | לוחות מחוונים ci-dessus + probe de sortie | Docs/DevRel + קשר ממשל | תמונת מצב של מיון, אפס התראות תגובות | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Les echantillons quotidiens d'office hours (2025-04-13 -> 2025-04-25) sont regropes en ייצוא NDJSON + PNG sous
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` עם שמות תקדים
`docs-preview-integrity-<date>.json` et les לוכדת מתכתבים.

## יומן משוב ובעיות

Utilisez ce tableau pour resumer les constats des reviewers. Liez chaque entree au ticket GitHub/discuss
ainsi qu'au formulaire מבנה לכידת באמצעות
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| הפניה | חמור | אחראי | סטטוט | הערות |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | נמוך | Docs-core-02 | רזולוציה 2025-04-18 | הבהרה של ניסוח הניווט נסה את זה + סרגל צד נוסף (`docs/source/sorafs/tryit.md` mis a jour avec le nouveau label). |
| `docs-preview/w1 #2` | נמוך | Docs-core-03 | Resolu 2025-04-19 | Capture Try it + legende rafraichies selon la demande; חפץ `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | מידע | Docs/DevRel lead | פרמה | Les commentaires restants etaient ייחודיות שאלות ותשובות; לוכדת dans chaque formulaire partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## בדיקת ידע וסקרים

1. רשם את התוצאות של הבוחן (קביל >=90%) pour chaque reviewer; joindre le CSV לייצא הזמנה של חפצי אמנות.
2. אוסף התשובות האיכותיות של הסקר תופסים באמצעות le template de feedback et les copier sous
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Planifier des appels de remediation pour toute personne sous le seuil et les consigner ici.

Les huit reviewers ont obtenu >=94% au בדיקת ידע (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Aucun appel de remediation
n'a ete necessaire; les exports de survey pour chaque partenaire sont sous
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventaire des Artefacts

- מתאר/סיכום בדיקה מקדימה של חבילה: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- בדיקת קורות חיים + בדיקת קישור: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de changement du proxy נסה את זה: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- טלמטריית יצוא: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- חבילת טלמטריה שעות משרד יומיות: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- יצוא משוב + סקר: placer des dossiers par reviewer sous
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- בדיקת ידע וקורות חיים ב-CSV: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Garder l'inventaire סינכרון עם מעקב אחר העניין. Joindre des hashes lors de la copie d'artefacts vers
ניהול הכרטיסים afin que les auditeurs puissent verifier les fichiers sans acces shell.