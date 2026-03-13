---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-11-15T05:16:44.667295+00:00"
translation_last_reviewed: 2026-01-30
---

# סביבת Try It

הפורטל למפתחים כולל קונסולת "Try it" אופציונלית כדי שתוכלו לקרוא נקודות קצה של Torii מבלי לעזוב את התיעוד. הקונסולה מעבירה בקשות דרך ה-proxy המובנה כך שהדפדפנים יכולים לעקוף מגבלות CORS ועדיין לאכוף מגבלות קצב ואימות.

## דרישות מקדימות

- Node.js 18.18 או חדש יותר (תואם לדרישות build של הפורטל)
- גישה לרשת לסביבת Torii staging
- bearer token שיכול לקרוא את נתיבי Torii שאתם מתכוונים לבדוק

כל תצורת ה-proxy נעשית באמצעות משתני סביבה. הטבלה הבאה מפרטת את ה-knobs העיקריים:

| משתנה | מטרה | ברירת מחדל |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | כתובת Torii בסיסית שה-proxy מעביר אליה בקשות | **Required** |
| `TRYIT_PROXY_LISTEN` | כתובת האזנה לפיתוח מקומי (פורמט `host:port` או `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | רשימה מופרדת בפסיקים של מקורות שמורשים לקרוא ל-proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | מזהה שמוכנס ל-`X-TryIt-Client` עבור כל בקשה upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | bearer token ברירת מחדל שנשלח ל-Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | מאפשר למשתמשים לספק token משלהם דרך `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | גודל גוף הבקשה המקסימלי (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | timeout upstream במילישניות | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | מספר בקשות מותרות לכל חלון קצב לכל IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | חלון מחליק להגבלת קצב (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | כתובת האזנה אופציונלית לנקודת metrics בסגנון Prometheus (`host:port` או `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | נתיב HTTP של נקודת metrics | `/metrics` |

ה-proxy חושף גם `GET /healthz`, מחזיר שגיאות JSON מובנות, ומסיר bearer tokens מפלט הלוגים.

הפעילו `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` כאשר אתם חושפים את ה-proxy למשתמשי docs כדי ש-Swagger ו-RapiDoc יוכלו להעביר bearer tokens שסופקו על ידי המשתמש. ה-proxy עדיין אוכף מגבלות קצב, מסיר פרטי זיהוי, ורושם אם בקשה השתמשה בטוקן ברירת המחדל או בעקיפה לפי בקשה. הגדירו את `TRYIT_PROXY_CLIENT_ID` עם התווית שתרצו לשלוח כ-`X-TryIt-Client`
(ברירת מחדל `docs-portal`). ה-proxy מקצץ ומוודא ערכי `X-TryIt-Client` שמסופקים על ידי הלקוח וחוזר לברירת המחדל הזו כדי ש-gateways של staging יוכלו לאמת מקור בלי לקשור מטא-דאטה של הדפדפן.

## הפעלת ה-proxy מקומית

התקינו תלותים בפעם הראשונה שאתם מקימים את הפורטל:

```bash
cd docs/portal
npm install
```

הפעילו את ה-proxy והצביעו אותו ל-Torii שלכם:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

הסקריפט לוגג את כתובת ה-bind ומעביר בקשות מ-`/proxy/*` אל מקור Torii שהוגדר.

לפני bind ל-socket הסקריפט מאמת ש-
`static/openapi/torii.json` תואם ל-digest שנרשם ב-
`static/openapi/manifest.json`. אם הקבצים סוטים, הפקודה תיכשל ותבקש להריץ
`npm run sync-openapi -- --latest`. ייצאו
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` רק למצבי חירום; ה-proxy ירשום אזהרה וימשיך כדי שתוכלו
להתאושש במהלך חלונות תחזוקה.

## חיבור ווידג'טים בפורטל

כאשר אתם בונים או מגישים את הפורטל, הגדירו את ה-URL שהווידג'טים צריכים להשתמש בו עבור ה-proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

הקומפוננטים הבאים קוראים את הערכים מ-`docusaurus.config.js`:

- **Swagger UI** - מוצג ב-`/reference/torii-swagger`; מבצע pre-authorize לסכימת bearer כאשר קיים token,
  מתייג בקשות עם `X-TryIt-Client`, מזריק `X-TryIt-Auth`, ומבצע rewrite לקריאות דרך ה-proxy כאשר
  `TRYIT_PROXY_PUBLIC_URL` מוגדר.
- **RapiDoc** - מוצג ב-`/reference/torii-rapidoc`; משקף את שדה ה-token,
  משתמש באותם headers של Swagger, ומכוון ל-proxy באופן אוטומטי כאשר ה-URL מוגדר.
- **Try it console** - מוטמע בדף overview של ה-API; מאפשר לשלוח בקשות מותאמות,
  לראות headers ולבחון גופי תגובה.

שני הפאנלים מציגים **snapshot selector** שקורא את
`docs/portal/static/openapi/versions.json`. מלאו את האינדקס עם
`npm run sync-openapi -- --version=<label> --mirror=current --latest` כדי שמבקרים יוכלו
לעבור בין specs היסטוריים, לראות את digest SHA-256 שנרשם, ולאשר אם snapshot של release
מכיל manifest חתום לפני שימוש בווידג'טים האינטראקטיביים.

שינוי token בווידג'ט כלשהו משפיע רק על ה-session הנוכחי בדפדפן; ה-proxy לא שומר ולא לוגג את ה-token שסופק.

## טוקני OAuth קצרי טווח

כדי להימנע מהפצת טוקנים ארוכי טווח של Torii למבקרים, חברו את קונסולת Try it לשרת OAuth שלכם. כאשר
משתני הסביבה הבאים קיימים, הפורטל מציג ווידג'ט device-code, מנפיק bearer tokens קצרי טווח,
ומחדיר אותם אוטומטית לטופס הקונסולה.

| משתנה | מטרה | ברירת מחדל |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | נקודת Device Authorization OAuth (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | נקודת token שמקבלת `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | מזהה לקוח OAuth שרשום עבור preview docs | _empty_ |
| `DOCS_OAUTH_SCOPE` | תחומי scope מופרדים ברווחים בעת הכניסה | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | audience אופציונלי לקישור token ל-API | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | מרווח polling מינימלי בזמן המתנה לאישור (ms) | `5000` (ערכים < 5000 ms נדחים) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | חלון תפוגה חלופי ל-device code (שניות) | `600` (חייב להיות בין 300 s ל-900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | זמן חיים חלופי ל-access token (שניות) | `900` (חייב להיות בין 300 s ל-900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | הגדירו `1` למיזמי preview מקומיים שמדלגים על OAuth בכוונה | _unset_ |

דוגמת קונפיגורציה:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

כאשר אתם מריצים `npm run start` או `npm run build`, הפורטל מטמיע את הערכים הללו ב-
`docusaurus.config.js`. במהלך preview מקומי, כרטיס Try it מציג כפתור
"Sign in with device code". המשתמשים מזינים את הקוד המוצג בעמוד OAuth; לאחר הצלחת ה-device flow הווידג'ט:

- מחדיר את ה-bearer token שהונפק לשדה הקונסולה של Try it,
- מתייג בקשות עם ה-headers הקיימים `X-TryIt-Client` ו-`X-TryIt-Auth`,
- מציג את הזמן שנותר, ו
- מנקה אוטומטית את ה-token כשפג תוקפו.

שדה ה-Bearer הידני נשאר זמין; השמיטו את משתני OAuth כאשר תרצו לאלץ מבקרים להדביק token זמני בעצמם,
או יצאו `DOCS_OAUTH_ALLOW_INSECURE=1` עבור previews מקומיים מבודדים שבהם גישה אנונימית מותרת.
Builds ללא OAuth מוגדר נכשלות כעת במהירות כדי לעמוד ב-gate של DOCS-1b.

הערה: עיינו ב-[Security hardening & pen-test checklist](./security-hardening.md)
לפני חשיפת הפורטל מחוץ למעבדה; המסמך מפרט את מודל האיום, פרופיל CSP/Trusted Types,
ואת שלבי ה-pen-test שחוסמים כעת את DOCS-1b.

## דוגמאות Norito-RPC

בקשות Norito-RPC משתמשות באותו proxy ו-OAuth plumbing כמו מסלולי JSON; הן פשוט מגדירות
`Content-Type: application/x-norito` ושולחות payload Norito מקודד מראש כפי שמופיע במפרט NRPC
(`docs/source/torii/nrpc_spec.md`).
במאגר יש payloads קנוניים תחת `fixtures/norito_rpc/` כדי שמחברי הפורטל, בעלי SDK ומבקרים
יוכלו לשחזר את ה-bytes המדויקים שבהם CI משתמש.

### שליחת payload Norito מתוך קונסולת Try It

1. בחרו fixture כגון `fixtures/norito_rpc/transfer_asset.norito`. הקבצים האלה הם
   מעטפות Norito גולמיות; **אל** תקודדו ב-base64.
2. ב-Swagger או RapiDoc, אתרו את endpoint ה-NRPC (לדוגמה
   `POST /v2/pipeline/submit`) והחליפו את בורר **Content-Type** ל-
   `application/x-norito`.
3. החליפו את עורך גוף הבקשה ל-**binary** (מצב "File" של Swagger או
   בורר "Binary/File" של RapiDoc) והעלו את קובץ ה-`.norito`. הווידג'ט
   מזרים את ה-bytes דרך ה-proxy ללא שינוי.
4. שלחו את הבקשה. אם Torii מחזיר `X-Iroha-Error-Code: schema_mismatch`,
   ודאו שאתם קוראים endpoint שמקבל payloads בינריים ואשרו שה-schema hash
   שנרשם ב-`fixtures/norito_rpc/schema_hashes.json`
   תואם ל-build של Torii שבו אתם משתמשים.

הקונסולה שומרת את הקובץ האחרון בזכרון כדי שתוכלו לשלוח מחדש את אותו payload תוך בדיקת
tokens שונים או hosts שונים של Torii. הוספת
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` ל-workflow שלכם מפיקה את bundle
הראיות שמוזכר בתוכנית האימוץ NRPC-4 (log + סיכום JSON), ומתאים לצילום מסך של תגובת Try It במהלך
reviews.

### דוגמת CLI (curl)

אותם fixtures ניתנים לשחזור מחוץ לפורטל באמצעות `curl`, וזה שימושי כאשר בודקים את ה-proxy
או מאתרים בעיות בתגובות gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

החליפו את ה-fixture בכל ערך שמופיע ב-`transaction_fixtures.manifest.json`
או קודדו payload משלכם עם `cargo xtask norito-rpc-fixtures`. כאשר Torii במצב canary
אפשר להצביע את `curl` אל ה-try-it proxy
(`https://docs.sora.example/proxy/v2/pipeline/submit`) כדי לבדוק את אותה תשתית
שהווידג'טים של הפורטל משתמשים בה.

## Observability ופעולות

כל בקשה נרשמת פעם אחת עם method, path, origin, סטטוס upstream ומקור האימות
(`override`, `default`, או `client`). Tokens אינם נשמרים לעולם - גם headers של bearer
וגם ערכי `X-TryIt-Auth` עוברים redaction לפני לוג, כך שניתן להעביר stdout ל-collector
מרכזי בלי לחשוש לדליפת סודות.

### Health probes ו-alerting

הריצו את ה-probe המובנה בזמן פריסות או לפי schedule:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Knobs של סביבה:

- `TRYIT_PROXY_SAMPLE_PATH` - נתיב Torii אופציונלי (ללא `/proxy`) לבדיקה.
- `TRYIT_PROXY_SAMPLE_METHOD` - ברירת מחדל `GET`; הגדירו `POST` לנתיבי כתיבה.
- `TRYIT_PROXY_PROBE_TOKEN` - מזריק bearer token זמני לקריאת הדוגמה.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - עוקף את timeout ברירת המחדל של 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - יעד טקסט Prometheus אופציונלי עבור `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - זוגות `key=value` מופרדים בפסיקים שמתווספים למטריקות (ברירת מחדל `job=tryit-proxy` ו-`instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - כתובת אופציונלית לנקודת metrics (למשל `http://localhost:9798/metrics`) שחייבת להגיב בהצלחה כאשר `TRYIT_PROXY_METRICS_LISTEN` מופעל.

הזינו את התוצאות ל-textfile collector על ידי הפניית ה-probe לנתיב כתיבה
(לדוגמה, `/var/lib/node_exporter/textfile_collector/tryit.prom`) והוספת labels מותאמים:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

הסקריפט כותב מחדש את קובץ המטריקות באופן אטומי כך שה-collector תמיד קורא payload מלא.

כאשר `TRYIT_PROXY_METRICS_LISTEN` מוגדר, הגדירו את
`TRYIT_PROXY_PROBE_METRICS_URL` לנקודת metrics כדי שה-probe ייכשל מהר אם משטח ה-scrape נעלם
(לדוגמה ingress שגוי או כללי firewall חסרים). הגדרה טיפוסית ל-production היא
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

להתראות קלות, חברו את ה-probe ל-monitoring stack שלכם. דוגמת Prometheus שמצלצלת
לאחר שתי כשלונות רצופים:

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

### Endpoint metrics ו-dashboards

הגדירו `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (או כל זוג host/port) לפני הפעלת ה-proxy
כדי לחשוף נקודת metrics בפורמט Prometheus. הנתיב ברירת מחדל הוא `/metrics` אך ניתן לשנותו דרך
`TRYIT_PROXY_METRICS_PATH=/custom`. כל scrape מחזיר מונים של סך בקשות לפי method,
דחיות rate limit, שגיאות/timeouts upstream, תוצאות proxy וסיכומי latency:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

כוונו את Collectors של Prometheus/OTLP לנקודת metrics והשתמשו מחדש בפאנלים הקיימים ב-
`dashboards/grafana/docs_portal.json` כדי ש-SRE יוכל לצפות ב-latencies של הזנב ובקפיצות דחייה
ללא parsing של לוגים. ה-proxy מפרסם אוטומטית `tryit_proxy_start_timestamp_ms` כדי לעזור
למפעילים לזהות אתחולים.

### אוטומצית rollback

השתמשו בעוזר הניהול כדי לעדכן או לשחזר את כתובת היעד של Torii. הסקריפט שומר את
התצורה הקודמת ב-`.env.tryit-proxy.bak` כך ש-rollbacks הם פקודה אחת.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

ניתן לעקוף את נתיב קובץ ה-env עם `--env` או `TRYIT_PROXY_ENV` אם הפריסה שלכם שומרת
את התצורה במקום אחר.
