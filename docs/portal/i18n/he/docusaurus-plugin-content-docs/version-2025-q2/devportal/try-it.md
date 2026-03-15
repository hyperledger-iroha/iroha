---
lang: he
direction: rtl
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# נסה את זה בארגז חול

פורטל המפתחים שולח קונסולת "נסה את זה" אופציונלית כך שתוכל להתקשר ל-Torii
נקודות קצה מבלי להשאיר את התיעוד. המסוף מעביר בקשות
דרך ה-proxy המצורף כך שדפדפנים יוכלו לעקוף את מגבלות ה-CORS כשהם עדיין
אכיפת מגבלות תעריפים ואימות.

## דרישות מוקדמות

- Node.js 18.18 ומעלה (תואם את דרישות בניית הפורטל)
- גישה לרשת לסביבת בימוי Torii
- אסימון נושא שיכול לקרוא למסלולי Torii שאתה מתכנן לממש

כל תצורת ה-proxy נעשית באמצעות משתני סביבה. הטבלה למטה
מפרט את הכפתורים החשובים ביותר:

| משתנה | מטרה | ברירת מחדל |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | כתובת אתר בסיס Torii שהפרוקסי מעביר בקשות אל | **חובה** |
| `TRYIT_PROXY_LISTEN` | כתובת האזנה לפיתוח מקומי (פורמט `host:port` או `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | רשימה מופרדת בפסיקים של מקורות שעשויים לקרוא ל-proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | מזהה הוצב ב-`X-TryIt-Client` עבור כל בקשה במעלה הזרם | `docs-portal` |
| `TRYIT_PROXY_BEARER` | אסימון נושא ברירת המחדל הועבר ל-Torii | _ריק_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | אפשר למשתמשי קצה לספק אסימון משלהם דרך `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | גודל גוף בקשה מרבי (בתים) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | פסק זמן במעלה הזרם באלפיות שניות | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | בקשות מותרות לכל חלון תעריף לכל IP של לקוח | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | חלון הזזה להגבלת קצב (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | כתובת האזנה אופציונלית עבור נקודת הקצה המטרית בסגנון Prometheus (`host:port` או `[ipv6]:port`) | _ריק (מושבת)_ |
| `TRYIT_PROXY_METRICS_PATH` | נתיב HTTP מוגש על ידי נקודת הקצה של מדדים | `/metrics` |

ה-proxy גם חושף את `GET /healthz`, מחזיר שגיאות JSON מובנות, ו
מסיר אסימוני נושא מפלט יומן.

הפעל את `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` בעת חשיפת ה-proxy למשתמשי מסמכים, כך שה-Swagger ו-
לוחות RapiDoc יכולים להעביר אסימוני נושא שסופקו על ידי המשתמש. ה-proxy עדיין אוכף מגבלות תעריפים,
מסיר אישורים ומתעד אם בקשה השתמשה באסימון ברירת המחדל או בעקיפה לכל בקשה.
הגדר את `TRYIT_PROXY_CLIENT_ID` לתווית שברצונך לשלוח בתור `X-TryIt-Client`
(ברירת המחדל היא `docs-portal`). ה-proxy חותך ומאמת את המתקשר שסופק
ערכי `X-TryIt-Client`, נופלים חזרה לברירת המחדל הזו כדי שערי הבמה יוכלו
ביקורת מקור ללא מתאם מטא נתונים של הדפדפן.

## הפעל את ה-proxy באופן מקומי

התקן תלות בפעם הראשונה שאתה מגדיר את הפורטל:

```bash
cd docs/portal
npm install
```

הפעל את ה-proxy והפנה אותו למופע Torii שלך:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

הסקריפט רושם את הכתובת המחוברת ומעביר בקשות מ-`/proxy/*` ל-
מקור Torii מוגדר.לפני כריכת השקע, הסקריפט מאמת את זה
`static/openapi/torii.json` תואם לתקציר שנרשם ב
`static/openapi/manifest.json`. אם הקבצים נסחפים, הפקודה יוצאת עם
שגיאה ומורה לך להפעיל את `npm run sync-openapi -- --latest`. ייצוא
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` רק לעקיפות חירום; ה-proxy יעשה זאת
רשום אזהרה והמשך כדי שתוכל להתאושש במהלך חלונות תחזוקה.

## חוט את הווידג'טים של הפורטל

כאשר אתה בונה או משרת את פורטל המפתחים, הגדר את כתובת האתר שהווידג'טים
צריך להשתמש עבור ה-proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

הרכיבים הבאים קוראים ערכים אלה מ-`docusaurus.config.js`:

- **ממשק המשתמש של Swagger** - מוצג ב-`/reference/torii-swagger`; מאשר מראש את
  סכימת נושא כאשר קיים אסימון, מתייג בקשות עם `X-TryIt-Client`,
  מזריק `X-TryIt-Auth`, ומשכתב שיחות דרך ה-proxy כאשר
  `TRYIT_PROXY_PUBLIC_URL` מוגדר.
- **RapiDoc** - מוצג ב-`/reference/torii-rapidoc`; משקף את שדה האסימון,
  עושה שימוש חוזר באותן כותרות כמו חלונית Swagger, וממקדת ל-proxy
  אוטומטית כאשר כתובת האתר מוגדרת.
- **מסוף לנסות את זה** - מוטבע בדף סקירת ה-API; מאפשר לשלוח מותאם אישית
  בקשות, הצג כותרות ובדוק גופי תגובה.

שני הלוחות משטחים **בורר תמונת מצב** שקורא
`docs/portal/static/openapi/versions.json`. תאכלס את המדד הזה עם
`npm run sync-openapi -- --version=<label> --mirror=current --latest` כך
סוקרים יכולים לקפוץ בין מפרטים היסטוריים, ראה את תקציר SHA-256 המוקלט,
ואשר אם תמונת מצב של שחרור נושאת מניפסט חתום לפני השימוש
הווידג'טים האינטראקטיביים.

שינוי האסימון בווידג'ט כלשהו משפיע רק על הפעלת הדפדפן הנוכחית; את
פרוקסי לעולם לא ממשיך או רושם את האסימון שסופק.

## אסימוני OAuth קצרי מועד

כדי להימנע מהפצת אסימוני Torii ארוכי טווח לבודקים, חבר את ה-Try it
המסוף לשרת ה-OAuth שלך. כאשר משתני הסביבה להלן קיימים
הפורטל מציג ווידג'ט לכניסה לקוד מכשיר, מטבע אסימוני נושאים קצרי מועד,
ומזריק אותם אוטומטית לטופס המסוף.

| משתנה | מטרה | ברירת מחדל |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | נקודת קצה של הרשאת מכשיר OAuth (`/oauth/device/code`) | _ריק (מושבת)_ |
| `DOCS_OAUTH_TOKEN_URL` | נקודת קצה אסימון שמקבלת `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _ריק_ |
| `DOCS_OAUTH_CLIENT_ID` | מזהה לקוח OAuth רשום עבור התצוגה המקדימה של מסמכים | _ריק_ |
| `DOCS_OAUTH_SCOPE` | היקפים מופרדים ברווחים שהתבקשו במהלך הכניסה | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | קהל API אופציונלי לאגד את האסימון אל | _ריק_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | מרווח סקר מינימלי בעת המתנה לאישור (ms) | `5000` (ערכים <5000ms נדחים) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | חלון תפוגה של קוד התקן (שניות) | `600` (חייב להישאר בין 300 ל-900) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | משך חיים של אסימון גישה חלופי (שניות) | `900` (חייב להישאר בין 300 ל-900) |
| `DOCS_OAUTH_ALLOW_INSECURE` | הגדר ל-`1` עבור תצוגות מקדימות מקומיות המדלגות בכוונה על אכיפת OAuth | _מושבת_ |

תצורה לדוגמה:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```כאשר אתה מפעיל את `npm run start` או `npm run build`, הפורטל מטמיע ערכים אלה
ב-`docusaurus.config.js`. במהלך תצוגה מקדימה מקומית, כרטיס Try it מציג את א
כפתור "היכנס עם קוד מכשיר". משתמשים מזינים את הקוד המוצג ב-OAuth שלך
דף אימות; ברגע שזרימת המכשיר מצליחה הווידג'ט:

- מזריק את אסימון הנושא שהונפק לשדה Try it console,
- תיוג בקשות עם הכותרות הקיימות `X-TryIt-Client` ו-`X-TryIt-Auth`,
- מציג את משך החיים שנותר, וכן
- מנקה אוטומטית את האסימון כשהוא יפוג.

הקלט הידני של Bearer נשאר זמין - השמיט את משתני OAuth בכל פעם שאתה
רוצים לאלץ את הסוקרים להדביק בעצמם אסימון זמני, או לייצא
`DOCS_OAUTH_ALLOW_INSECURE=1` עבור תצוגות מקדימות מקומיות מבודדות שבהן גישה אנונימית
מקובל. בנייה ללא תצורת OAuth נכשלת במהירות כדי לספק את
שער מפת הדרכים DOCS-1b.

📌 סקור את [רשימת התיוג של הקשחת אבטחה ובדיקת עט](./security-hardening.md)
לפני חשיפת הפורטל מחוץ למעבדה; הוא מתעד את מודל האיום,
פרופיל CSP/Trusted Types, ושלבי בדיקת החדירה שמכניסים כעת את DOCS-1b.

## דוגמאות Norito-RPC

בקשות Norito-RPC חולקות את אותם פרוקסי וצנרת OAuth כמו מסלולי JSON,
הם פשוט מגדירים את `Content-Type: application/x-norito` ושולחים את
מטען מקודד מראש Norito המתואר במפרט NRPC
(`docs/source/torii/nrpc_spec.md`).
המאגר שולח מטענים קנוניים תחת `fixtures/norito_rpc/` כך פורטל
מחברים, בעלי SDK ומבקרים יכולים להפעיל מחדש את הבייטים המדויקים שבהם משתמש CI.

### שלח מטען Norito ממסוף Try It

1. בחר מתקן כגון `fixtures/norito_rpc/transfer_asset.norito`. אלה
   הקבצים הם מעטפות Norito גולמיות; **לא** קודד אותם ב-base64.
2. ב-Swagger או RapiDoc, אתר את נקודת הקצה של NRPC (לדוגמה
   `POST /v2/pipeline/submit`) והעבר את בורר **סוג התוכן** ל
   `application/x-norito`.
3. החלף את עורך גוף הבקשה ל-**בינארי** (מצב "קובץ" של סוואגר או
   בורר ה"בינארי/קובץ" של RapiDoc) והעלה את הקובץ `.norito`. הווידג'ט
   מזרים את הבתים דרך ה-proxy ללא שינוי.
4. הגש את הבקשה. אם Torii מחזירה `X-Iroha-Error-Code: schema_mismatch`,
   ודא שאתה קורא לנקודת קצה שמקבלת מטענים בינאריים ו
   אשר שה-hash של הסכימה נרשם ב-`fixtures/norito_rpc/schema_hashes.json`
   תואם למבנה Torii שאתה מבצע.

המסוף שומר את הקובץ העדכני ביותר בזיכרון כך שתוכל לשלוח אותו מחדש
מטען בעת הפעלת אסימוני הרשאה שונים או מארחי Torii. מוסיף
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` לזרימת העבודה שלך מייצרת
צרור הראיות אליו מתייחסים בתוכנית האימוץ של NRPC-4 (לוג + סיכום JSON),
שמשתלב יפה עם צילום מסך של תגובת Try It במהלך ביקורות.

### דוגמה CLI (תלתל)

ניתן להפעיל מחדש את אותם משחקים מחוץ לפורטל באמצעות `curl`, וזה שימושי
בעת אימות ה-proxy או איתור באגים בתגובות שער:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v2/pipeline/submit"
```החלף את המתקן לכל ערך המופיע ב-`transaction_fixtures.manifest.json`
או לקודד את המטען שלך עם `cargo xtask norito-rpc-fixtures`. כאשר Torii
הוא במצב קנרי אתה יכול להצביע על `curl` על פרוקסי לנסות
(`https://docs.sora.example/proxy/v2/pipeline/submit`) לממש אותו
תשתית שבה משתמשים ווידג'טים של הפורטל.

## צפיות ופעולות

כל בקשה מתועדת פעם אחת עם שיטה, נתיב, מוצא, מצב במעלה הזרם וה-
מקור אימות (`override`, `default`, או `client`). אסימונים הם אף פעם
מאוחסן - הן כותרות הנושא והן ערכי `X-TryIt-Auth` נמחקים לפני
רישום - כך שתוכל להעביר סטדout לאספן מרכזי מבלי לדאוג
סודות דולפים.

### בדיקות בריאות והתראה

הפעל את הבדיקה המצורפת במהלך פריסות או לפי לוח זמנים:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

כפתורי סביבה:

- `TRYIT_PROXY_SAMPLE_PATH` — מסלול Torii אופציונלי (ללא `/proxy`) למימוש.
- `TRYIT_PROXY_SAMPLE_METHOD` - ברירת המחדל היא `GET`; מוגדר ל-`POST` עבור מסלולי כתיבה.
- `TRYIT_PROXY_PROBE_TOKEN` - מזריק אסימון נושא זמני לשיחה לדוגמה.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - עוקף את פסק הזמן המוגדר כברירת מחדל של 5 שניות.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — יעד קובץ טקסט אופציונלי Prometheus עבור `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - צמדי `key=value` מופרדים בפסיקים המצורפים למדדים (ברירת המחדל היא `job=tryit-proxy` ו-`instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - כתובת אתר של מדדים אופציונליים (לדוגמה, `http://localhost:9798/metrics`) שחייבת להגיב בהצלחה כאשר `TRYIT_PROXY_METRICS_LISTEN` מופעל.

הזן את התוצאות לאספן קבצי טקסט על ידי הפניית הבדיקה לעבר חומר כתיבה
נתיב (לדוגמה, `/var/lib/node_exporter/textfile_collector/tryit.prom`) ו
הוספת תוויות מותאמות אישית:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

הסקריפט משכתב את קובץ המדדים בצורה אטומית כך שהאספן שלך תמיד קורא א
מטען מלא.

כאשר `TRYIT_PROXY_METRICS_LISTEN` מוגדר, הגדר
`TRYIT_PROXY_PROBE_METRICS_URL` לנקודת הקצה של המדדים כך שהבדיקה נכשלת במהירות
אם משטח הגרידה נעלם (לדוגמה, כניסת תצורה שגויה או חסר
חוקי חומת האש). מסגרת ייצור טיפוסית היא
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

להתראה קלת משקל, חוט את הגשושית לערימת הניטור שלך. A Prometheus
דוגמה שדפים אחרי שני כשלים רצופים:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### מדדים נקודת קצה ולוחות מחוונים

הגדר את `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (או כל זוג מארח/יציאות) לפני
הפעלת ה-proxy כדי לחשוף נקודת קצה של מדדים בפורמט Prometheus. השביל
ברירת המחדל היא `/metrics` אך ניתן לעקוף באמצעות
`TRYIT_PROXY_METRICS_PATH=/custom`. כל גרידה מחזירה מונים לכל שיטה
סיכומי בקשות, דחיות של מגבלת תעריף, שגיאות/פסקי זמן במעלה הזרם, תוצאות פרוקסי,
וסיכומי חביון:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

כוון את אספני Prometheus/OTLP שלך לנקודת הקצה של המדדים ועשה שימוש חוזר
לוחות `dashboards/grafana/docs_portal.json` קיימים כך ש-SRE יוכל לצפות בזנב
זמן השהייה ודחייה ללא ניתוח יומנים. ה-proxy באופן אוטומטי
מפרסם את `tryit_proxy_start_timestamp_ms` כדי לעזור למפעילים לזהות אתחול מחדש.

### אוטומציה לאחור

השתמש בעוזר הניהול כדי לעדכן או לשחזר את כתובת האתר היעד Torii. התסריט
מאחסן את התצורה הקודמת ב-`.env.tryit-proxy.bak` כך שהחזרות הן א
פקודה בודדת.```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

עוקף את נתיב קובץ ה-env עם `--env` או `TRYIT_PROXY_ENV` אם הפריסה שלך
מאחסן תצורה במקום אחר.