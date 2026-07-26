---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
כותרת: Plan d'intake communautaire W2
sidebar_label: תוכנית W2
תיאור: קליטה, אישורים ורשימת בדיקה מקדימה של האיחוד האירופי.
---

| אלמנט | פרטים |
| --- | --- |
| מעורפל | W2 - סוקרים קהילתיים |
| Fenetre cible | Q3 2025 semaine 1 (tentatif) |
| Tag d'artefact (planifie) | `preview-2025-06-15` |
| גשש נושאים | `DOCS-SORA-Preview-W2` |

## אובייקטים

1. הגדר את הקריטריונים לכניסת האיחוד האירופי ואת זרימת העבודה של הבדיקה.
2. ממשל אישור להצעת הסגל ותוספת השימוש מקובלת.
3. Rafraichir l'artefact preview contrôle par checksum et le bundle de telemetrie pour la nouvelle fenetre.
4. מכין את ה-proxy נסה את זה ואת לוחות המחוונים של הזמנות.

## מגזרת דקואז'

| תעודת זהות | טאצ'ה | אחראי | Echeance | סטטוט | הערות |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Rediger les criteres d'intake communautaire (זכאות, משבצות מקסימליות, דרישות CoC) ומפזר ממשל | Docs/DevRel lead | 2025-05-15 | טרמין | La politique d'intake a ete mergee dans `DOCS-SORA-Preview-W2` et endossee lors de la reunion du conseil 2025-05-20. |
| W2-P2 | Mettre a jour le template de demande avec des question communautaires (מוטיבציה, חוסר אחריות, חוץ מלוקליזציה) | Docs-core-01 | 2025-05-18 | טרמין | `docs/examples/docs_preview_request_template.md` כולל תחזוקה של סעיף קהילה, רפרנס בנוסח הצריכה. |
| W2-P3 | קבל אישור ממשל לכניסה לתוכנית (הצבעה באיחוד + נרשמים דקות) | קשר ממשל | 22-05-2025 | טרמין | הצביעו לאמץ ל-2025-05-20; דקות + מסדר שקרים dans `DOCS-SORA-Preview-W2`. |
| W2-P4 | Planifier le staging du proxy נסה את זה + לכידת טלמטריה ל-W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | טרמין | כרטיס שינוי `OPS-TRYIT-188` אישור וביצוע 2025-06-09 02:00-04:00 UTC; צילומי מסך Grafana ארכיון עם כרטיס. |
| W2-P5 | Construire/verifier le nouveau tag d'artefact preview (`preview-2025-06-15`) ו-archiver descriptor/checksum/probe logs | פורטל TL | 2025-06-07 | טרמין | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` לבצע 2025-06-10; פלט מלאי sous `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | מאסף את רשימת ההזמנות של האיחוד האירופי (<=25 סוקרים, הרבה דרג) עם אנשי קשר מאשרים על פי ממשל | מנהל קהילה | 2025-06-10 | טרמין | קבוצת בכורה של 8 סוקרים האיחוד האירופי; IDs de requete `DOCS-SORA-Preview-REQ-C01...C08` יומני ב-Le Tracker. |

## רשימת רשימת קודים- [x] ממשל רישום אישור (הערות איחוד + עיקול הצבעה) לצרף `DOCS-SORA-Preview-W2`.
- [x] Template de demande mis a jour commite sous `docs/examples/`.
- [x] מתאר `preview-2025-06-15`, סכום בדיקת יומן, פלט בדיקה, דוח קישור ופרוקסי תמלול נסה את זה במלאי של `artifacts/docs_preview/W2/`.
- [x] צילומי מסך Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) לוכדות pour la fenetre preflight W2.
- [x] טבלאות לוח הזמנות עם מבקרים מזהים, כרטיסים מבוקשים וחותמות זמן של אישור החזרה (voir section W2 du tracker).

גרדר ce plan a jour; le tracker le reference pour que le roadmap DOCS-SORA voie exactement ce qu'il reste avant l'envoi des invitations W2.