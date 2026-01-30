---
lang: ur
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/security-hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 21b96d7db3a5a215735b5951cb8a3bb16d6d490377bd6d05140b09a1cc74fdec
source_last_modified: "2025-11-08T11:41:06.507683+00:00"
translation_last_reviewed: 2026-01-30
---

# הקשחת אבטחה וצ'קליסט בדיקות חדירה

## סקירה כללית

פריט הרודמפ **DOCS-1b** דורש OAuth device-code login, מדיניות אבטחת תוכן חזקה,
ובדיקות חדירה חוזרות לפני שהפורטאל של preview יוכל לרוץ ברשתות מחוץ למעבדה. נספח זה
מסביר את מודל האיומים, הבקרות שממומשות בריפו, ואת צ'קליסט ה-go-live שמבדקי gate חייבים לבצע.

- **בתחום:** פרוקסי Try it, פאנלים מוטמעים של Swagger/RapiDoc, וקונסולת Try it מותאמת שמוצגת ע"י
  `docs/portal/src/components/TryItConsole.jsx`.
- **מחוץ לתחום:** Torii עצמו (מכוסה ב-Torii readiness reviews) ופרסום SoraFS
  (מכוסה ע"י DOCS-3/7).

## מודל איומים

| נכס | סיכון | מיתון |
| --- | --- | --- |
| Torii bearer tokens | גניבה או שימוש חוזר מחוץ ל-docs sandbox | device-code login (`DOCS_OAUTH_*`) מייצר טוקנים קצרים, הפרוקסי מסיר headers, והקונסולה מפיגה אוטומטית credentials שמורים. |
| פרוקסי Try it | שימוש לרעה כ-open relay או עקיפת מגבלות Torii | `scripts/tryit-proxy*.mjs` אוכף origin allowlists, rate limiting, health probes, ו-forwarding מפורש של `X-TryIt-Auth`; אין שמירת credentials. |
| runtime של הפורטל | cross-site scripting או embeds זדוניים | `docusaurus.config.js` מזריק headers של Content-Security-Policy, Trusted Types, ו-Permissions-Policy; inline scripts מוגבלים ל-runtime של Docusaurus. |
| נתוני observability | טלמטריה חסרה או שיבוש | `docs/portal/docs/devportal/observability.md` מתעד probes/dashboards; `scripts/portal-probe.mjs` רץ ב-CI לפני פרסום. |

היריבים כוללים משתמשים סקרנים הצופים ב-preview הציבורי, גורמים זדוניים שבוחנים קישורים גנובים,
ודפדפנים פגועים שמנסים לגרד credentials שמורים. כל הבקרות חייבות לעבוד בדפדפנים רגילים ללא רשתות אמון.

## בקרות נדרשות

1. **OAuth device-code login**
   - הגדירו `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` ו-knobs קשורים בסביבת ה-build.
   - כרטיס Try it מציג widget של sign-in (`OAuthDeviceLogin.jsx`) שמביא device code,
     מבצע polling ל-token endpoint, ומנקה טוקנים אוטומטית לאחר פקיעה. Overrides ידניים
     של Bearer נשארים זמינים כ-fallback חירום.
   - הבילדים נכשלים כאשר חסרה תצורת OAuth או כאשר TTLs של fallback חורגים מחלון 300-900 s
     הנדרש ע"י DOCS-1b; הגדירו `DOCS_OAUTH_ALLOW_INSECURE=1` רק עבור previews מקומיים חד-פעמיים.
2. **Guardrails של הפרוקסי**
   - `scripts/tryit-proxy.mjs` אוכף allowed origins, rate limits, caps על גודל בקשה
     ו-upstream timeouts תוך סימון התעבורה ב-`X-TryIt-Client` והסרת טוקנים מהלוגים.
   - `scripts/tryit-proxy-probe.mjs` יחד עם `docs/portal/docs/devportal/observability.md`
     מגדירים liveness probe וכללי dashboard; הריצו אותם לפני כל rollout.
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` מייצא כעת security headers דטרמיניסטיים:
     `Content-Security-Policy` (default-src self, רשימות מחמירות ל-connect/img/script,
     דרישות Trusted Types), `Permissions-Policy`, ו-`Referrer-Policy: no-referrer`.
   - רשימת ה-connect של CSP עושה whitelist ל-endpoints של OAuth device-code ו-token
     (HTTPS בלבד אלא אם `DOCS_SECURITY_ALLOW_INSECURE=1`) כדי ש-device login יעבוד
     בלי להקל את ה-sandbox עבור origins אחרים.
   - ה-headers משובצים ישירות ב-HTML שנוצר, כך ש-hosts סטטיים לא צריכים קונפיגורציה נוספת.
     שמרו inline scripts מוגבלים ל-bootstrap של Docusaurus.
4. **Runbooks, observability, ו-rollback**
   - `docs/portal/docs/devportal/observability.md` מתאר probes ו-dashboards שמנטרים
     כשלים ב-login, קודי תגובה של הפרוקסי, ו-request budgets.
   - `docs/portal/docs/devportal/incident-runbooks.md` מכסה את נתיב ההסלמה אם ה-sandbox
     מנוצל לרעה; שלבו עם
     `scripts/tryit-proxy-rollback.mjs` כדי להחליף endpoints בבטחה.

## צ'קליסט pen-test ו-release

השלימו רשימה זו עבור כל קידום preview (צרפו תוצאות לכרטיס release):

1. **אימות wiring של OAuth**
   - הריצו `npm run start` מקומית עם exports של `DOCS_OAUTH_*` מהפרודקשן.
   - מפרופיל דפדפן נקי, פתחו את קונסולת Try it ואשרו שהזרימה device-code מנפיקה טוקן,
     סופרת את זמן החיים, ומנקה את השדה לאחר פקיעה או sign-out.
2. **בדיקת הפרוקסי**
   - `npm run tryit-proxy` מול Torii staging, ואז הריצו
     `npm run probe:tryit-proxy` עם sample path שהוגדר.
   - בדקו בלוגים עבור `authSource=override` ואשרו שה-rate limiting
     מגדיל מונים כאשר חוצים את החלון.
3. **אימות CSP/Trusted Types**
   - הריצו `npm run build` ופתחו `build/index.html`. ודאו שתג `<meta
     http-equiv="Content-Security-Policy">` תואם את הדירקטיבות הצפויות
     וש-DevTools אינו מציג הפרות CSP בעת טעינת ה-preview.
   - השתמשו ב-`npm run probe:portal` (או curl) כדי למשוך את ה-HTML המוטמע; ה-probe
     נכשל כעת כאשר meta tags `Content-Security-Policy`, `Permissions-Policy` או
     `Referrer-Policy` חסרים או שונים מהערכים ב-`docusaurus.config.js`, כך שמבקרי governance
     יכולים לסמוך על exit code במקום לבדוק פלט curl.
4. **בדיקת observability**
   - ודאו שה-dashboard של Try it proxy ירוק (rate limits, error ratios,
     health probe metrics).
   - הריצו incident drill ב-`docs/portal/docs/devportal/incident-runbooks.md`
     אם ה-host השתנה (פריסת Netlify/SoraFS חדשה).
5. **תיעוד התוצאות**
   - צרפו screenshots/logs לכרטיס release.
   - תעדו כל finding בתבנית דוח remediation
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     כדי להקל על auditing של owners, SLAs, וראיות retest בהמשך.
   - קשרו חזרה לצ'קליסט זה כדי שפריט הרודמפ DOCS-1b יישאר auditable.

אם שלב כלשהו נכשל, עצרו את הקידום, פתחו issue חוסמת, וציינו את תוכנית ה-remediation ב-`status.md`.
