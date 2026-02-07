---
lang: he
direction: rtl
source: docs/portal/docs/api/overview.mdx
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 293cedf8e24e3e711da383ae5152786614d1ea294b9232988dcf57784cf1a30d
source_last_modified: "2025-12-27T17:59:34+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

יבא את TryItConsole מ-'@site/src/components/TryItConsole.jsx';

# סקירת ממשק API של Torii

שער Torii חושף משטח REST + WebSocket לאינטראקציה עם Iroha
צמתים. מסמך OpenAPI הקנוני נוצר באמצעות:

```bash
cargo xtask openapi
```

הפעלת הפקודה למעלה כותבת `docs/portal/static/openapi/torii.json` ו
מזין את שילוב `docusaurus-plugin-openapi-docs` כך שסעיף זה מתעדכן
באופן אוטומטי. המחולל מעלה נתב Torii בזיכרון ומנסה
שאל את נקודת הקצה OpenAPI שלו; אם זה נכשל (לדוגמה, אתחול הנתב
שגיאות) במקום זה נפלט מפרט מציין מיקום דטרמיניסטי.

## מצב נוכחי

- **מקור מפרט:** `cargo xtask openapi` שואל את הנתב Torii ישירות; את
  בדל החזרה נפלט רק כאשר הנתב לא מצליח לחשוף OpenAPI
  מסמך.
- **דטרמיניזם:** פלט JSON עובר קנוניזציה כדי להבטיח הבדל יציב ב-CI.
- **אימות:** ארגז החול "נסה את זה" מעביר כעת בקשות דרך א
  פרוקסי בימוי מוגבל. המפעילים חייבים לספק Torii נקודת קצה ונושא
  אסימון לפני הוצאת שיחות.
- **בורר גרסאות:** ממשק המשתמש של Swagger, RapiDoc ודף Torii OpenAPI המלא
  חשוף תפריט נפתח שקורא `openapi/versions.json` כדי שתוכל להשוות
  תצלומים היסטוריים. האפשרות `Latest (tracked)` מצביעה על
  `/openapi/torii.json`, בעוד שכל שאר הערכים טוענים את הערכים המתאימים
  חפץ `openapi/versions/<version>/torii.json`.
- **זרימת עבודה של תמונת מצב:** לאחר הפעלת `npm run docs:version -- <label>`, רענן
  הקובץ התואם OpenAPI עם `npm run sync-openapi -- --version=<label> --latest`
  כך שהתפריט הנפתח יכול לטעון גם את תמונת המצב הקפואה וגם את המבנה המתגלגל "האחרון".

המחלץ מגובה הנתב פולט כעת את משטח Torii המלא. אם אתה רואה את
בדל מציין מיקום, התייחס אליו כאל כשל בזמן בנייה ובדוק את Torii
יומני ההפעלה של הנתב.

## נסה את זה בארגז חול (DOCS-3)

פורטל המפתחים מטמיע קונסולה קטנה המעבירה בקשות REST דרך a
פרוקסי מבוסס צומת. ה-proxy אוכף CORS, מגבלות תעריפים ונושא אופציונלי
אסימונים כדי שהווידג'ט יוכל לדבר עם שערים מבלי לחשוף אישורים.
ראה `docs/devportal/try-it.md` לרשימת הבדיקה המלאה של המפעיל.

הפעל את ה-proxy לצד שרת הפיתוח Docusaurus:

```bash
cd docs/portal
# Proxy configuration
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_BEARER="sora-dev-token"
# Front-end configuration so the widget knows where the proxy lives
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"

# Run the proxy and the docs site (parallel terminals recommended)
npm run tryit-proxy
npm run start
```

- ה-proxy מאזין ב-`TRYIT_PROXY_LISTEN` (ברירת מחדל `127.0.0.1:8787`) ו
  מעביר כל בקשות `/proxy/...` ל-`TRYIT_PROXY_TARGET`.
- ספק `TRYIT_PROXY_BEARER` עבור אסימוני נושאים סטטיים או תן למתקשרים לספק
  אחד לכל בקשה דרך השדה "אסימון נושא".
- התעריף מגביל כברירת מחדל ל-60 בקשות לדקה וניתן לכוון אותם
  `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS`.

> ⚠️ ה-proxy מיועד לגישה לשלב/הדגמה. פריסות ייצור
> צריך לרוץ מאחורי כניסה קשוחה ולהשתמש מחדש באותה מדיניות הגבלת תעריפים ב
> חזית Torii.

<TryItConsole />