---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/observability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1aaba0a8673b2479df647391a9db3a0f64e84169303b3b6b51ed7f1090e419c5
source_last_modified: "2025-11-07T10:33:21.912384+00:00"
translation_last_reviewed: 2026-01-30
---

# Observability ו-Analytics של הפורטל

מפת הדרכים DOCS-SORA דורשת analytics, synthetic probes ואוטומציית קישורים שבורים לכל preview build.
מסמך זה מתאר את ה-plumbing שמגיע עם הפורטל כדי שאופרטורים יוכלו לחבר ניטור בלי לדלוף נתוני מבקרים.

## תיוג ריליס

- הגדירו `DOCS_RELEASE_TAG=<identifier>` (fallback ל-`GIT_COMMIT` או `dev`) בעת בניית הפורטל.
  הערך מוזרק אל `<meta name="sora-release">` כדי ש-probes ו-dashboards יוכלו להבחין בין פריסות.
- `npm run build` מייצר `build/release.json` (נכתב על ידי
  `scripts/write-checksums.mjs`) המתאר את התג, ה-timestamp ואת `DOCS_RELEASE_SOURCE` האופציונלי.
  אותו קובץ נארז בארטיפקטים של preview ומוזכר בדוח ה-link checker.

## Analytics עם שמירה על פרטיות

- הגדירו `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` כדי לאפשר את ה-tracker הקל.
  ה-payloads כוללים `{ event, path, locale, release, ts }` בלי metadata של referrer או IP,
  ו-`navigator.sendBeacon` משמש ככל האפשר כדי להמנע מחסימת ניווטים.
- שלטו בדגימה באמצעות `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). ה-tracker שומר את ה-path האחרון שנשלח
  ואינו שולח אירועים כפולים עבור אותו ניווט.
- המימוש נמצא ב-`src/components/AnalyticsTracker.jsx` ומותקן גלובלית דרך `src/theme/Root.js`.

## Synthetic probes

- `npm run probe:portal` שולח בקשות GET נגד נתיבים נפוצים
  (`/`, `/norito/overview`, `/reference/torii-swagger`, וכו') ומוודא שתג המטא
  `sora-release` תואם ל-`--expect-release` (או `DOCS_RELEASE_TAG`). דוגמה:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

כשלים מדווחים לכל path, מה שמקל על gate של CD לפי הצלחת probes.

## אוטומציה לקישורים שבורים

- `npm run check:links` סורק את `build/sitemap.xml`, מוודא שכל entry ממופה לקובץ מקומי
  (עם בדיקת fallbacks של `index.html`), וכותב `build/link-report.json` שמכיל metadata של ריליס,
  סיכומים, כשלים, ו-SHA-256 fingerprint של `checksums.sha256` (חשוף כ-`manifest.id`) כדי לקשר
  כל דוח למניפסט של הארטיפקט.
- הסקריפט מסתיים בקוד לא-אפס כאשר דף חסר, כך ש-CI יכול לחסום ריליסים על נתיבים ישנים או שבורים.
  הדוחות מציינים את נתיבי המועמדים שנבדקו, מה שעוזר לאתר רגרסיות ניתוב חזרה לעץ docs.

## דשבורד Grafana והתראות

- `dashboards/grafana/docs_portal.json` מפרסם את לוח Grafana **Docs Portal Publishing**.
  הוא כולל את הפאנלים הבאים:
  - *Gateway Refusals (5m)* משתמש ב-`torii_sorafs_gateway_refusals_total` עם סקופ
    `profile`/`reason` כדי לאפשר ל-SRE לזהות policy pushes גרועים או כשלי tokens.
  - *Alias Cache Refresh Outcomes* ו-*Alias Proof Age p90* עוקבים אחר
    `torii_sorafs_alias_cache_*` כדי להוכיח שקיימים proofs טריים לפני DNS cut over.
  - *Pin Registry Manifest Counts* וסטט *Active Alias Count* משקפים את backlog ה-pin-registry ואת
    סך ה-aliases כדי שה-governance תוכל לבצע audit לכל ריליס.
  - *Gateway TLS Expiry (hours)* מדגיש מתי cert ה-TLS של ה-publishing gateway מתקרב לפקיעה
    (סף התראה 72 h).
  - *Replication SLA Outcomes* ו-*Replication Backlog* עוקבים אחר הטלמטריה
    `torii_sorafs_replication_*` כדי לוודא שכל הרפליקות עומדות ברף GA אחרי הפרסום.
- השתמשו במשתני התבנית המובנים (`profile`, `reason`) כדי להתמקד בפרופיל publishing `docs.sora`
  או לחקור קפיצות בכל ה-gateways.
- הניתוב של PagerDuty משתמש בפאנלים של הדשבורד כראיה: ההתראות
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, ו-`DocsPortal/TLSExpiry`
  מופעלות כאשר הסדרה המתאימה עוברת את הסף. קשרו את ה-runbook של ההתראה לעמוד זה כדי
  שה-on-call יוכל לשחזר את שאילתות Prometheus המדויקות.

## חיבור הכל יחד

1. במהלך `npm run build`, הגדירו את משתני הסביבה של release/analytics ותנו לשלב ה-post-build
   להפיק `checksums.sha256`, `release.json`, ו-`link-report.json`.
2. הריצו `npm run probe:portal` מול ה-hostname של preview עם
   `--expect-release` שמחובר לאותו תג. שמרו את ה-stdout עבור checklist של publishing.
3. הריצו `npm run check:links` כדי להיכשל מהר על רשומות שבורות ב-sitemap ולארכב את דוח ה-JSON
   יחד עם ארטיפקטים של preview. ה-CI שם את הדוח האחרון ב-`artifacts/docs_portal/link-report.json`
   כך שה-governance תוכל להוריד את חבילת הראיות ישירות מלוגי הבילד.
4. כוונו את endpoint ה-analytics ל-collector שומר פרטיות (Plausible, self-hosted OTEL ingest וכו')
   ודאו ששיעורי הדגימה מתועדים לכל ריליס כדי שהדשבורדים יפרשו את הספירות נכון.
5. ה-CI כבר מחווט את השלבים האלה דרך workflows של preview/deploy
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), כך ש-dry runs מקומיים צריכים לכסות
   רק התנהגות ספציפית לסודות.
