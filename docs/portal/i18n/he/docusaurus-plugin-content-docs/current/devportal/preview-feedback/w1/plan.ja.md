---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a6df9b812fb02aa786f4f172b627c70bc015a632e1d4f2723245b3d77bcc3f6
source_last_modified: "2025-11-14T04:43:19.876724+00:00"
translation_last_reviewed: 2026-01-30
---

| פריט | פרטים |
| --- | --- |
| גל | W1 - שותפים ואינטגרטורים של Torii |
| חלון יעד | Q2 2025 שבוע 3 |
| תג ארטיפקט (מתוכנן) | `preview-2025-04-12` |
| כרטיס מעקב | `DOCS-SORA-Preview-W1` |

## יעדים

1. להשיג אישורים משפטיים וממשל עבור תנאי ה-preview של השותפים.
2. להכין את Try it proxy ואת צילומי הטלמטריה שמשתמשים בהם בחבילת ההזמנה.
3. לרענן את ארטיפקט ה-preview המאומת ב-checksum ואת תוצאות ה-probe.
4. לסיים את רשימת השותפים ותבניות הבקשה לפני שליחת ההזמנות.

## פירוט משימות

| ID | משימה | בעלים | יעד | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | לקבל אישור משפטי לתוספת תנאי ה-preview | Docs/DevRel lead -> Legal | 2025-04-05 | ✅ הושלם | כרטיס משפטי `DOCS-SORA-Preview-W1-Legal` אושר ב-2025-04-05; PDF מצורף ל-tracker. |
| W1-P2 | לתפוס חלון staging של Try it proxy (2025-04-10) ולאמת את בריאות הפרוקסי | Docs/DevRel + Ops | 2025-04-06 | ✅ הושלם | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` בוצע ב-2025-04-06; תמליל CLI ו-`.env.tryit-proxy.bak` נשמרו. |
| W1-P3 | לבנות ארטיפקט preview (`preview-2025-04-12`), להריץ `scripts/preview_verify.sh` + `npm run probe:portal`, ולשמור descriptor/checksums | Portal TL | 2025-04-08 | ✅ הושלם | ארטיפקט ולוגי אימות נשמרו תחת `artifacts/docs_preview/W1/preview-2025-04-12/`; פלט ה-probe צורף ל-tracker. |
| W1-P4 | לבדוק טפסי intake של שותפים (`DOCS-SORA-Preview-REQ-P01...P08`), לאשר אנשי קשר ו-NDAs | Governance liaison | 2025-04-07 | ✅ הושלם | כל שמונה הבקשות אושרו (שתי האחרונות ב-2025-04-11); האישורים מקושרים ב-tracker. |
| W1-P5 | לנסח נוסח הזמנה (מבוסס על `docs/examples/docs_preview_invite_template.md`), להגדיר `<preview_tag>` ו-`<request_ticket>` לכל שותף | Docs/DevRel lead | 2025-04-08 | ✅ הושלם | טיוטת הזמנה נשלחה ב-2025-04-12 15:00 UTC לצד קישורי ארטיפקט. |

## רשימת בדיקות preflight

> טיפ: הריצו `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` כדי לבצע את השלבים 1-5 אוטומטית (build, אימות checksum, probe של הפורטל, link checker, ועדכון Try it proxy). הסקריפט רושם לוג JSON שאפשר לצרף לכרטיס המעקב.

1. `npm run build` (עם `DOCS_RELEASE_TAG=preview-2025-04-12`) כדי ליצור מחדש `build/checksums.sha256` ו-`build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` ולארכב `build/link-report.json` לצד ה-descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (או להעביר יעד מתאים דרך `--tryit-target`); לבצע commit ל-`.env.tryit-proxy` המעודכן ולשמור את ה-`.bak` לצורך rollback.
6. לעדכן את כרטיס W1 עם נתיבי הלוגים (checksum של ה-descriptor, פלט probe, שינוי ב-Try it proxy, וצילומי Grafana).

## Checklist ראיות

- [x] אישור משפטי חתום (PDF או קישור לכרטיס) מצורף ל-`DOCS-SORA-Preview-W1`.
- [x] צילומי Grafana עבור `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor ולוג checksum של `preview-2025-04-12` נשמרו תחת `artifacts/docs_preview/W1/`.
- [x] טבלת roster של הזמנות עם `invite_sent_at` מלאים (ראו לוג W1 ב-tracker).
- [x] ארטיפקטים של feedback משוקפים ב-[`preview-feedback/w1/log.md`](./log.md) עם שורה לכל שותף (עודכן 2025-04-26 עם נתוני roster/telemetria/issues).

לעדכן את התוכנית הזו ככל שהמשימות מתקדמות; ה-tracker מפנה אליה כדי לשמור על auditability של ה-roadmap.

## זרימת משוב

1. עבור כל reviewer, לשכפל את התבנית ב-
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   למלא את המטא-דאטה, ולאחסן את העותק המושלם תחת
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. לסכם הזמנות, checkpoints של טלמטריה ו-issues פתוחים בתוך הלוג החי ב-
   [`preview-feedback/w1/log.md`](./log.md) כדי שמבקרי governance יוכלו לשחזר את הגל כולו
   מבלי לצאת מהמאגר.
3. כאשר מתקבלים exports של knowledge-check או סקרים, לצרף אותם בנתיב הארטיפקט שצוין בלוג
   ולקשר את כרטיס ה-tracker.
