---
lang: he
direction: rtl
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-11-15T05:30:33.582253+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/devportal/try-it.md (Try It Sandbox Guide) -->

---
title: מדריך סנדבוקס "Try It"
summary: כיצד להפעיל את פרוקסי Torii לסביבת staging ואת סנדבוקס פורטל המפתחים.
---

פורטל המפתחים כולל קונסולת “Try it” עבור Torii REST API. מדריך זה מסביר כיצד
להפעיל את הפרוקסי התומך ולחבר את הקונסולה לשער staging מבלי לחשוף פרטי גישה.

## דרישות מקדימות

- Checkout של מאגר Iroha (שורש ה-workspace).
- Node.js 18.18+ (תואם לבסיס הפורטל).
- נקודת קצה של Torii שנגישה מתחנת העבודה (staging או מקומי).

## 1. יצירת snapshot של OpenAPI (אופציונלי)

הקונסולה משתמשת באותו payload של OpenAPI כמו דפי הרפרנס של הפורטל. אם שינית
נתיבי Torii, צור מחדש את ה-snapshot:

```bash
cargo xtask openapi
```

המשימה כותבת `docs/portal/static/openapi/torii.json`.

## 2. הפעלת פרוקסי Try It

מהשורש של הריפו:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### משתני סביבה

| משתנה | תיאור |
|--------|-------|
| `TRYIT_PROXY_TARGET` | כתובת הבסיס של Torii (חובה). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | רשימת מקורות מופרדת בפסיקים שמותר להשתמש בפרוקסי (ברירת מחדל: `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | טוקן bearer אופציונלי שמוחל כברירת מחדל על כל הבקשות המועברות דרך הפרוקסי. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | הגדירו `1` כדי להעביר את כותרת `Authorization` של הלקוח כפי שהיא. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | הגדרות rate limiter בזיכרון (ברירת מחדל: 60 בקשות לכל 60 שניות). |
| `TRYIT_PROXY_MAX_BODY` | גודל payload מקסימלי שמתקבל (בבתים, ברירת מחדל 1 MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | timeout לפניות upstream ל-Torii (ברירת מחדל 10 000 ms). |

הפרוקסי חושף:

- `GET /healthz` — בדיקת מוכנות.
- `/proxy/*` — בקשות פרוקסי, תוך שמירת ה-path וה-query string.

## 3. הפעלת הפורטל

בטרמינל נפרד:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

גשו ל-`http://localhost:3000/api/overview` והשתמשו בקונסולת Try It. אותם משתני
סביבה מגדירים את ההטמעות של Swagger UI ו-RapiDoc.

## 4. הרצת בדיקות יחידה

הפרוקסי כולל חבילת בדיקות מהירה מבוססת Node:

```bash
npm run test:tryit-proxy
```

הבדיקות מכסות ניתוח כתובות, טיפול במקורות, rate limiting והזרקת bearer.

## 5. אוטומציית probe ומדדים

השתמשו בכלי ה-probe המובנה כדי לאמת את `/healthz` ואת נקודת הקצה לדוגמה:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

אפשרויות סביבה:

- `TRYIT_PROXY_SAMPLE_PATH` — נתיב Torii אופציונלי (ללא `/proxy`) לבדיקה.
- `TRYIT_PROXY_SAMPLE_METHOD` — ברירת מחדל `GET`; הגדירו `POST` עבור נתיבי כתיבה.
- `TRYIT_PROXY_PROBE_TOKEN` — מזריק bearer זמני לקריאה לדוגמה.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — עוקף את ברירת המחדל של 5 שניות.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — יעד textfile של Prometheus עבור `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` — זוגות `key=value` מופרדים בפסיקים שמתווספים למדדים (ברירת מחדל: `job=tryit-proxy` ו-`instance=<proxy URL>`).

כאשר `TRYIT_PROXY_PROBE_METRICS_FILE` מוגדר, הסקריפט משכתב את הקובץ בצורה
אטומית כדי ש-node_exporter/textfile collector תמיד יקבל payload שלם. דוגמה:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

העבירו את המדדים המתקבלים ל-Prometheus והשתמשו בדוגמת ה-alert במסמכי
developer-portal כדי לקרוא כאשר `probe_success` יורד ל-`0`.

## 6. רשימת הקשחה לייצור

לפני פרסום הפרוקסי מעבר לפיתוח מקומי:

- סיימו TLS לפני הפרוקסי (reverse proxy או gateway מנוהל).
- הגדירו לוגים מובנים והעבירו אותם לפייפליינים של observability.
- החליפו טוקני bearer ושמרו אותם במנהל הסודות שלכם.
- נטרו את נקודת הקצה `/healthz` ואספו מדדי השהיה.
- יישרו את מגבלות הקצב עם מכסות ה-staging של Torii; התאימו את התנהגות
  `Retry-After` כדי לתקשר חסימות ללקוחות.

</div>
