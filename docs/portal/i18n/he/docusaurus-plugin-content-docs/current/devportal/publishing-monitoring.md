---
id: publishing-monitoring
lang: he
direction: rtl
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

פריט ה-roadmap **DOCS-3c** דורש יותר מרשימת בדיקה לאריזה: אחרי כל פרסום SoraFS אנו חייבים
להוכיח באופן רציף שהפורטל למפתחים, ה-proxy של Try it וה-bindings של ה-gateway נשארים תקינים.
דף זה מתעד את משטח הניטור שמלווה את [מדריך הפריסה](./deploy-guide.md) כך ש-CI ומהנדסי on call
יוכלו להריץ את אותן בדיקות ש-Ops משתמשת בהן לאכיפת ה-SLO.

## תקציר pipeline

1. **Build וחתימה** - עקבו אחרי [מדריך הפריסה](./deploy-guide.md) כדי להריץ
   `npm run build`, `scripts/preview_wave_preflight.sh`, ואת שלבי ההגשה של Sigstore + manifest.
   סקריפט ה-preflight מפיק `preflight-summary.json` כדי שכל preview יישא מטאדטה של build/link/probe.
2. **Pin ואימות** - `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   ותוכנית ה-DNS cutover מספקים artefacts דטרמיניסטיים ל-governance.
3. **ארכוב ראיות** - שמרו את סיכום ה-CAR, bundle Sigstore, הוכחת alias,
   פלט probe ו-snapshots של הדשבורד `docs_portal.json` תחת `artifacts/sorafs/<tag>/`.

## ערוצי ניטור

### 1. מוניטורים לפרסום (`scripts/monitor-publishing.mjs`)

הפקודה החדשה `npm run monitor:publishing` מאגדת את probe הפורטל, probe של Try it
ואת מאמת ה-bindings לבדיקה אחת ידידותית ל-CI. ספקו config JSON
(שמורה ב-CI secrets או `configs/docs_monitor.json`) והריצו:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

הוסיפו `--prom-out ../../artifacts/docs_monitor/monitor.prom` (ואופציונלית
`--prom-job docs-preview`) כדי להפיק metrics בפורמט טקסט Prometheus המתאים ל-Pushgateway
או scrapes ישירים ב-staging/production. המטריקות משקפות את סיכום ה-JSON כך שדשבורדי SLO
וכללי התראות יוכלו לעקוב אחרי בריאות הפורטל, Try it, bindings ו-DNS ללא parsing של חבילת הראיות.

דוגמת config עם knobs נדרשים ו-multiple bindings:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/soraカタカナ.../assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

המוניטור כותב סיכום JSON (friendly ל-S3/SoraFS) ויוצא עם קוד non-zero כאשר probe נכשל,
מה שהופך אותו מתאים ל-Cron jobs, steps ב-Buildkite או webhooks של Alertmanager.
העברת `--evidence-dir` שומרת את `summary.json`, `portal.json`, `tryit.json` ו-`binding.json`
יחד עם manifest בשם `checksums.sha256` כדי שמבקרי governance יוכלו לבצע diff
בלי להריץ מחדש את ה-probes.

> **TLS guardrail:** `monitorPortal` דוחה URLs מסוג `http://` אלא אם כן מוגדר
> `allowInsecureHttp: true` ב-config. שמרו על probes של production/staging ב-HTTPS;
> האפשרות קיימת רק ל-previews מקומיים.

Each binding entry runs `cargo xtask soradns-verify-binding` against the captured
`portal.gateway.binding.json` bundle (and optional `manifestJson`) so alias,
proof status, and content CID stay aligned with the published evidence. The
optional `hostname` guard confirms the alias-derived canonical host matches the
gateway host you intend to promote, preventing DNS cutovers that drift from the
recorded binding.


הבלוק האופציונלי `dns` מחבר את rollout של SoraDNS מ-DOCS-7 לאותו monitor.
כל entry פותר זוג hostname/record-type (לדוגמה ה-CNAME
`docs-preview.sora.link` -> `docs-preview.sora.link.gw.sora.name`) ומוודא
שהתשובות תואמות את `expectedRecords` או `expectedIncludes`. ה-entry השני בדוגמה
מקבע את ה-hostname הקנוני המגובב שמופק ע"י `cargo xtask soradns-hosts --name docs-preview.sora.link`;
המוניטור מוכיח כעת שגם ה-alias הידידותי וגם ההאש הקנוני (`igjssx53...gw.sora.id`)
נפתרים ל-pretty host המוצמד. זה הופך את ראיות קידום ה-DNS לאוטומטיות:
המוניטור יכשל אם אחד מה-hosts יסטה, אפילו כאשר ה-bindings של HTTP ממשיכים
להצמיד את ה-manifest הנכון.

### 2. Guard למניפסט גרסאות OpenAPI

הדרישה של DOCS-2b ל"manifest OpenAPI חתום" מספקת כעת guard אוטומטי:
`ci/check_openapi_spec.sh` מפעיל `npm run check:openapi-versions`, שקורא ל-
`scripts/verify-openapi-versions.mjs` כדי להצליב
`docs/portal/static/openapi/versions.json` מול מפרטי Torii וה-manifests בפועל.
ה-guard מאמת כי:

- לכל גרסה ב-`versions.json` יש תיקיה תואמת תחת `static/openapi/versions/`.
- השדות `bytes` ו-`sha256` תואמים לקובץ ה-spec בדיסק.
- ה-alias `latest` משקף את הערך `current` (metadata של digest/size/signature)
  כך שה-download הדיפולטי לא יסטה.
- רשומות חתומות מפנות למניפסט שה-`artifact.path` שלו מצביע חזרה לאותו spec,
  וערכי חתימה/מפתח ציבורי ב-hex תואמים למניפסט.

הריצו את ה-guard מקומית בכל פעם שממפים spec חדשה:

```bash
cd docs/portal
npm run check:openapi-versions
```

הודעות כשל כוללות את רמז הקובץ המיושן (`npm run sync-openapi -- --latest`) כדי
שמתרימי הפורטל ידעו כיצד לרענן את ה-snapshots. שמירת ה-guard ב-CI מונעת
שחרורי פורטל שבהם ה-manifest החתום וה-digest המפורסם יוצאים מסינכרון.

### 2. Dashboards והתראות

- **`dashboards/grafana/docs_portal.json`** - לוח ראשי עבור DOCS-3c. ה-panels
  עוקבים אחרי `torii_sorafs_gateway_refusals_total`, כשלי SLA בשכפול, שגיאות
  proxy Try it, ו-latency של probes (overlay `docs.preview.integrity`). ייצאו את
  הלוח אחרי כל release וצרפו אותו לטיקט התפעול.
- **התראות proxy Try it** - כלל Alertmanager `TryItProxyErrors` מופעל על ירידות
  מתמשכות ב-`probe_success{job="tryit-proxy"}` או קפיצות ב-
  `tryit_proxy_requests_total{status="error"}`.
- **Gateway SLO** - `DocsPortal/GatewayRefusals` מבטיח שה-bindings של alias
  ימשיכו לפרסם את digest של המניפסט המוצמד; ההסלמות מפנות ל-transcript של CLI
  `cargo xtask soradns-verify-binding` שנלכד בזמן הפרסום.

### 3. נתיב ראיות

כל הרצת ניטור צריכה לצרף:

- חבילת ראיות `monitor-publishing` (`summary.json`, קבצים לפי סעיף, ו-`checksums.sha256`).
- screenshots מ-Grafana עבור הלוח `docs_portal` במהלך חלון ה-release.
- transcripts של שינוי/rollback של proxy Try it (לוגים של `npm run manage:tryit-proxy`).
- פלט אימות alias מ-`cargo xtask soradns-verify-binding`.

שמרו אותם תחת `artifacts/sorafs/<tag>/monitoring/` וקשרו אותם ב-issue של ה-release
כדי שמסלול הביקורת ישרוד גם אחרי שתוקף לוגי ה-CI יפוג.

## Checklist תפעולי

1. הריצו את מדריך הפריסה עד שלב 7.
2. הריצו `npm run monitor:publishing` עם קונפיג production; ארכבו את פלט ה-JSON.
3. צלמו panels של Grafana (`docs_portal`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`)
   וצרפו לטיקט ה-release.
4. תזמנו מוניטורים חוזרים (מומלץ: כל 15 דקות) מול כתובות production עם אותה config
   כדי לעמוד בשער ה-SLO של DOCS-3c.
5. בזמן אינצידנטים, הריצו מחדש את פקודת המוניטור עם `--json-out` כדי לתעד ראיות
   לפני/אחרי ולצרף אותן ל-postmortem.

מעקב אחר הלופ הזה סוגר את DOCS-3c: זרימת הבנייה של הפורטל, pipeline הפרסום,
וערימת הניטור חיים כעת ב-playbook אחד עם פקודות משחזרות, configs לדוגמה ו-hooks לטלמטריה.
