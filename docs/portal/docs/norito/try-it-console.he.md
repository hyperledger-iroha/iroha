---
lang: he
direction: rtl
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d38bbaedc2d75fc67e3f027c46834fb5a94d5fccd845014f829f3defcbc782b5
source_last_modified: "2025-11-20T15:21:55.938588+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: מסוף Try-It של Norito
description: השתמשו בפרוקסי של פורטל המפתחים ובווידג'טים של Swagger ו-RapiDoc כדי לשלוח בקשות Torii / Norito-RPC אמיתיות ישירות מאתר התיעוד.
---

הפורטל מרכז שלוש חזיתות אינטראקטיביות שמעבירות תעבורה ל-Torii:

- **Swagger UI** ב-`/reference/torii-swagger` מציג את מפרט ה-OpenAPI החתום ומשכתב אוטומטית בקשות דרך הפרוקסי כאשר `TRYIT_PROXY_PUBLIC_URL` מוגדר.
- **RapiDoc** ב-`/reference/torii-rapidoc` מציג את אותה סכימה עם העלאות קבצים ובוררי סוג תוכן שעובדים טוב עם `application/x-norito`.
- **Try it sandbox** בדף הסקירה של Norito מספק טופס קל לבקשות REST אד-הוק ולהתחברות OAuth במצב מכשיר.

שלושת הווידג'טים שולחים בקשות אל **פרוקסי Try-It** המקומי (`docs/portal/scripts/tryit-proxy.mjs`). הפרוקסי בודק ש-`static/openapi/torii.json` תואם לדיג'סט החתום ב-`static/openapi/manifest.json`, אוכף מגביל קצב, מסיר את כותרות `X-TryIt-Auth` מהלוגים, ומסמן כל קריאת upstream עם `X-TryIt-Client` כדי שמפעילי Torii יוכלו לאתר מקורות תעבורה.

## הפעלת הפרוקסי

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` הוא כתובת הבסיס של Torii שברצונך לבדוק.
- `TRYIT_PROXY_ALLOWED_ORIGINS` צריך לכלול כל מקור של הפורטל (שרת מקומי, שם מארח של פרודקשן, כתובת תצוגה מקדימה) שאמור להטמיע את המסוף.
- `TRYIT_PROXY_PUBLIC_URL` נצרך על ידי `docusaurus.config.js` ומוזרק לווידג'טים דרך `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` נטען רק כאשר `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` מוגדר; אחרת המשתמשים חייבים לספק טוקן משלהם דרך המסוף או דרך זרימת OAuth במצב מכשיר.
- `TRYIT_PROXY_CLIENT_ID` מגדיר את תג `X-TryIt-Client` שמועבר בכל בקשה.
  שליחת `X-TryIt-Client` מהדפדפן מותרת אך הערכים נחתכים
  ונדחים אם הם כוללים תווי בקרה.

בעת ההפעלה הפרוקסי מריץ את `verifySpecDigest` ויוצא עם רמז לתיקון אם המניפסט מיושן. הריצו `npm run sync-openapi -- --latest` כדי להוריד את מפרט Torii העדכני ביותר או העבירו `TRYIT_PROXY_ALLOW_STALE_SPEC=1` לעקיפות חירום.

כדי לעדכן או להחזיר את יעד הפרוקסי ללא עריכת קבצי הסביבה ידנית, השתמשו בעוזר:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## חיבור הווידג'טים

הגישו את הפורטל אחרי שהפרוקסי מאזין:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` חושף את ההגדרות הבאות:

| משתנה | מטרה |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | כתובת URL שמוזרקת ל-Swagger, ל-RapiDoc ול-sandbox Try it. השאירו לא מוגדר כדי להסתיר את הווידג'טים במהלך תצוגות לא מורשות. |
| `TRYIT_PROXY_DEFAULT_BEARER` | טוקן ברירת מחדל אופציונלי הנשמר בזיכרון. דורש `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` והגנת CSP ב-HTTPS בלבד (DOCS-1b) אלא אם תעבירו `DOCS_SECURITY_ALLOW_INSECURE=1` מקומית. |
| `DOCS_OAUTH_*` | מפעיל את זרימת OAuth במצב מכשיר (`OAuthDeviceLogin` component) כדי שסוקרים יוכלו להנפיק טוקנים קצרי טווח בלי לעזוב את הפורטל. |

כאשר משתני OAuth קיימים, ה-sandbox מציג כפתור **Sign in with device code** שעובר דרך שרת ה-Auth המוגדר (ראו `config/security-helpers.js` לצורה המדויקת). הטוקנים שמונפקים דרך זרימת המכשיר נשמרים רק בסשן הדפדפן.

## שליחת מטעני Norito-RPC

1. צרו מטען `.norito` עם ה-CLI או באמצעות הקטעים המתוארים ב-[quickstart של Norito](./quickstart.md). הפרוקסי מעביר גופי `application/x-norito` ללא שינוי, כך שתוכלו להשתמש באותו ארטיפקט שהייתם שולחים עם `curl`.
2. פתחו `/reference/torii-rapidoc` (מועדף למטענים בינאריים) או `/reference/torii-swagger`.
3. בחרו את ה-snapshot הרצוי של Torii מהרשימה הנפתחת. ה-snapshots חתומים; הפאנל מציג את הדיג'סט של המניפסט שנרשם ב-`static/openapi/manifest.json`.
4. בחרו את סוג התוכן `application/x-norito` במגירת "Try it", לחצו **Choose File**, ובחרו את המטען שלכם. הפרוקסי משכתב את הבקשה ל-`/proxy/v1/pipeline/submit` ומסמן אותה עם `X-TryIt-Client=docs-portal-rapidoc`.
5. כדי להוריד תגובות Norito הגדירו `Accept: application/x-norito`. Swagger/RapiDoc מציגים את בורר הכותרות באותה מגירה ומזרימים את הבינארי בחזרה דרך הפרוקסי.

לנתיבים של JSON בלבד ה-sandbox המובנה של Try it לרוב מהיר יותר: הזינו את הנתיב (לדוגמה `/v1/accounts/<i105-account-id>/assets`), בחרו את שיטת ה-HTTP, הדביקו גוף JSON כשצריך, ולחצו **Send request** כדי לבדוק כותרות, משך זמן ומטענים במקום.

## פתרון תקלות

| תסמין | סיבה אפשרית | פתרון |
| --- | --- | --- |
| הקונסול של הדפדפן מציג שגיאות CORS או שה-sandbox מזהיר שה-URL של הפרוקסי חסר. | הפרוקסי לא רץ או שהמקור לא ברשימת ההיתרים. | הפעילו את הפרוקסי, ודאו ש-`TRYIT_PROXY_ALLOWED_ORIGINS` מכסה את מארח הפורטל שלכם, והריצו מחדש `npm run start`. |
| `npm run tryit-proxy` מסתיים עם “digest mismatch”. | חבילת OpenAPI של Torii השתנתה במעלה הזרם. | הריצו `npm run sync-openapi -- --latest` (או `--version=<tag>`) ונסו שוב. |
| הווידג'טים מחזירים `401` או `403`. | טוקן חסר, פג תוקף או בעל הרשאות לא מספיקות. | השתמשו בזרימת OAuth במצב מכשיר או הדביקו bearer token תקין ב-sandbox. עבור טוקנים סטטיים יש לייצא `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` מהפרוקסי. | חרגתם ממגבלת הקצב לכל IP. | העלו את `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` עבור סביבות אמון או האטו את סקריפטי הבדיקה. כל דחייה בגלל מגבלת קצב מעלה את `tryit_proxy_rate_limited_total`. |

## תצפיתיות

- `npm run probe:tryit-proxy` (מעטפת סביב `scripts/tryit-proxy-probe.mjs`) קורא ל-`/healthz`, בודק אופציונלית נתיב לדוגמה, ומפיק קבצי טקסט של Prometheus עבור `probe_success` / `probe_duration_seconds`. הגדירו את `TRYIT_PROXY_PROBE_METRICS_FILE` לשילוב עם node_exporter.
- הגדירו `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` כדי לחשוף מונים (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) והיסטוגרמות זמן השהיה. לוח המחוונים `dashboards/grafana/docs_portal.json` קורא את המדדים האלה כדי לאכוף SLOs של DOCS-SORA.
- לוגי הריצה נמצאים ב-stdout. כל רשומה כוללת את מזהה הבקשה, הסטטוס upstream, מקור האימות (`default`, `override` או `client`), והמשך; סודות מוסתרים לפני הפליטה.

אם עליכם לוודא ש-payloads של `application/x-norito` מגיעים ל-Torii ללא שינוי, הריצו את חבילת Jest (`npm test -- tryit-proxy`) או בדקו את ה-fixtures תחת `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. בדיקות הרגרסיה מכסות בינאריים דחוסים של Norito, מניפסטים חתומים של OpenAPI ומסלולי הורדת דרגה של הפרוקסי כדי שהשקות NRPC ישמרו על נתיב ראיות קבוע.
