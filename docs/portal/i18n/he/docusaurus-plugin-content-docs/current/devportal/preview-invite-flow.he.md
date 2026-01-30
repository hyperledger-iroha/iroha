---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/preview-invite-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0ffb27e9caaf9e492ce534d3845f4da28d7bf0ec8c60c8e91f8319de157b27af
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# זרימת הזמנות לפריוויו

## מטרה

פריט הרודמפ **DOCS-SORA** מציין כי onboarding של סוקרים ותוכנית הזמנות לפריוויו ציבורי הם החסמים האחרונים לפני שהפורטל יוצא מ-beta. עמוד זה מתאר כיצד לפתוח כל גל הזמנות, אילו ארטיפקטים חייבים להישלח לפני שליחת ההזמנות, וכיצד להוכיח שהזרימה ניתנת לביקורת. השתמשו בו לצד:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) לטיפול לפי סוקר.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) לערבויות checksum.
- [`devportal/observability`](./observability.md) לייצואי טלמטריה ול-hooks של התראות.

## תוכנית גלים

| גל | קהל | קריטריוני כניסה | קריטריוני יציאה | הערות |
| --- | --- | --- | --- | --- |
| **W0 - Maintainers core** | Maintainers של Docs/SDK שמאמתים תוכן יום-אחד. | צוות GitHub `docs-portal-preview` מאוכלס, שער checksum ב-`npm run serve` ירוק, Alertmanager שקט 7 ימים. | כל מסמכי P0 נסקרו, backlog תויג, אין אירועים חוסמים. | משמש לאימות הזרימה; אין אימייל הזמנה, רק שיתוף ארטיפקטי הפריוויו. |
| **W1 - Partners** | מפעילי SoraFS, אינטגרטורים של Torii, וסוקרי ממשל תחת NDA. | W0 הסתיים, תנאים משפטיים אושרו, פרוקסי Try-it ב-staging. | התקבל sign-off מהשותפים (issue או טופס חתום), טלמטריה מציגה <=10 סוקרים במקביל, ללא רגרסיות אבטחה במשך 14 ימים. | להחיל תבנית הזמנה + טיקט בקשה. |
| **W2 - קהילה** | תורמים נבחרים מרשימת ההמתנה הקהילתית. | W1 הסתיים, תרגילי אירועים בוצעו, FAQ ציבורי עודכן. | משוב עובּד, >=2 ריליסי תיעוד שוחררו דרך pipeline הפריוויו ללא rollback. | להגביל הזמנות במקביל (<=25) ולבצע באצ'ים שבועיים. |

תעדו איזו גל פעיל ב-`status.md` ובמתעד בקשות הפריוויו כדי שהממשל יראה את המצב במבט אחד.

## רשימת בדיקות preflight

השלימו את הפעולות **לפני** תזמון הזמנות לגל:

1. **ארטיפקטים של CI זמינים**
   - `docs-portal-preview` האחרון + descriptor הועלה ע"י `.github/workflows/docs-portal-preview.yml`.
   - Pin של SoraFS מצוין ב-`docs/portal/docs/devportal/deploy-guide.md` (descriptor של cutover קיים).
2. **אכיפת checksum**
   - `docs/portal/scripts/serve-verified-preview.mjs` מופעל דרך `npm run serve`.
   - הוראות `scripts/preview_verify.sh` נבדקו ב-macOS + Linux.
3. **בסיס טלמטריה**
   - `dashboards/grafana/docs_portal.json` מציג תעבורת Try it תקינה וההתראה `docs.preview.integrity` ירוקה.
   - נספח עדכני של `docs/portal/docs/devportal/observability.md` עודכן עם קישורי Grafana.
4. **ארטיפקטים של ממשל**
   - issue של invite tracker מוכנה (issue אחת לכל גל).
   - תבנית רישום סוקרים הועתקה (ראו [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - אישורים משפטיים ו-SRE הנדרשים מצורפים ל-issue.

רשמו את השלמת ה-preflight ב-invite tracker לפני שליחת כל דוא"ל.

## שלבי הזרימה

1. **בחירת מועמדים**
   - לשלוף מרשימת ההמתנה או תור השותפים.
   - לוודא שלכל מועמד יש תבנית בקשה מלאה.
2. **אישור גישה**
   - להקצות מאשר ל-issue של invite tracker.
   - לאמת דרישות מקדימות (CLA/חוזה, שימוש מקובל, תדריך אבטחה).
3. **שליחת הזמנות**
   - להשלים את ה-placeholders ב-[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, אנשי קשר).
   - לצרף descriptor + hash של archive, כתובת Try it staging, וערוצי תמיכה.
   - לשמור את המייל הסופי (או תמליל Matrix/Slack) ב-issue.
4. **מעקב onboarding**
   - לעדכן את invite tracker עם `invite_sent_at`, `expected_exit_at`, והסטטוס (`pending`, `active`, `complete`, `revoked`).
   - לקשר את בקשת הקבלה של הסוקר לצורכי auditability.
5. **מעקב טלמטריה**
   - לעקוב אחרי `docs.preview.session_active` והתראות `TryItProxyErrors`.
   - לפתוח אירוע אם הטלמטריה חורגת מה-baseline ולתעד את התוצאה ליד רישום ההזמנה.
6. **איסוף משוב וסיום**
   - לסגור הזמנות כאשר המשוב מגיע או ש-`expected_exit_at` חלף.
   - לעדכן את issue הגל בסיכום קצר (ממצאים, אירועים, צעדים הבאים) לפני מעבר לקוהורט הבא.

## ראיות ודיווח

| ארטיפקט | היכן לשמור | תדירות עדכון |
| --- | --- | --- |
| issue של invite tracker | פרויקט GitHub `docs-portal-preview` | עדכון אחרי כל הזמנה. |
| ייצוא roster של סוקרים | רישום מקושר ב-`docs/portal/docs/devportal/reviewer-onboarding.md` | שבועי. |
| צילומי טלמטריה | `docs/source/sdk/android/readiness/dashboards/<date>/` (להשתמש בחבילת הטלמטריה) | לכל גל + לאחר אירועים. |
| digest משוב | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (ליצור תיקיה לכל גל) | בתוך 5 ימים מיציאת הגל. |
| הערת ישיבת ממשל | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | למלא לפני כל sync של DOCS-SORA. |

הריצו `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
לאחר כל אצווה כדי להפיק digest קריא למכונה. צרפו את ה-JSON המופק ל-issue של הגל כדי שסוקרי הממשל יוכלו לאשר את ספירות ההזמנות בלי לשחזר את כל הלוג.

צרפו את רשימת הראיות ל-`status.md` בכל פעם שגל מסתיים כדי שניתן יהיה לעדכן את פריט הרודמפ במהירות.

## קריטריוני rollback והשהיה

השעו את זרימת ההזמנות (והודיעו לממשל) כאשר אחד מהבאים מתרחש:

- אירוע proxy של Try it שדרש rollback (`npm run manage:tryit-proxy`).
- עייפות התראות: יותר מ-3 עמודי התראה עבור endpoints של preview בלבד בתוך 7 ימים.
- פער ציות: הזמנה נשלחה ללא תנאים חתומים או ללא רישום תבנית הבקשה.
- סיכון שלמות: mismatch ב-checksum שנמצא ע"י `scripts/preview_verify.sh`.

חזרו לפעילות רק לאחר תיעוד remediation ב-invite tracker ואישור שדשבורד הטלמטריה יציב לפחות 48 שעות.
