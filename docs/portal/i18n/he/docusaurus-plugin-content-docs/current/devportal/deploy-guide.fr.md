---
lang: fr
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c8cbcb556fc4a33d8395551eccad5fe13e6d71bf83abc9bcc43c7ab840d2c99
source_last_modified: "2025-11-08T11:41:06.506450+00:00"
translation_last_reviewed: 2026-01-30
---

## סקירה כללית

הפלייבוק הזה ממיר את פריטי מפת הדרכים **DOCS-7** (פרסום SoraFS) ו-**DOCS-8**
(אוטומציית pin ב-CI/CD) לנוהל מעשי עבור פורטל המפתחים.
הוא מכסה את שלב ה-build/lint, אריזת SoraFS, חתימת מניפסטים עם Sigstore,
קידום alias, אימות ותרגילי rollback כדי שכל ארטיפקט תצוגה מקדימה ושחרור יהיה
ניתן לשחזור וניתן לביקורת.

הזרימה מניחה שיש לך את הבינארי `sorafs_cli` (נבנה עם `--features cli`), גישה לנקודת
קצה Torii עם הרשאות pin-registry, ואישורי OIDC עבור Sigstore. אחסן סודות
ארוכי-חיים (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, טוקני Torii) בכספת ה-CI;
ריצות מקומיות יכולות לטעון אותם מ-exports של ה-shell.

## דרישות מקדימות

- Node 18.18+ עם `npm` או `pnpm`.
- `sorafs_cli` מתוך `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- כתובת Torii שחושפת `/v1/sorafs/*` וכן חשבון/מפתח פרטי של רשות שיכולה להגיש
  מניפסטים ו-aliasים.
- מנפיק OIDC (GitHub Actions, GitLab, workload identity, וכו') כדי להנפיק
  `SIGSTORE_ID_TOKEN`.
- אופציונלי: `examples/sorafs_cli_quickstart.sh` להרצות dry run ו-
  `docs/source/sorafs_ci_templates.md` לתבניות workflows של GitHub/GitLab.
- הגדר את משתני OAuth של Try it (`DOCS_OAUTH_*`) והריץ את
  [security-hardening checklist](./security-hardening.md) לפני קידום build
  מחוץ למעבדה. בניית הפורטל נכשלת כעת כאשר משתנים אלו חסרים
  או כאשר כווני TTL/polling מחוץ לחלונות שנאכפים; ייצא
  `DOCS_OAUTH_ALLOW_INSECURE=1` רק עבור תצוגות מקדימות מקומיות זמניות. צרף את
  ראיות ה-pen-test לכרטיס השחרור.

## שלב 0 — לכידת חבילת פרוקסי Try it

לפני קידום תצוגה מקדימה ל-Netlify או ל-gateway, הטביעו את מקורות פרוקסי Try it ואת
דיגסט המניפסט OpenAPI החתום בחבילה דטרמיניסטית:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` מעתיק את עוזרי proxy/probe/rollback,
מאמת את חתימת OpenAPI וכותב `release.json` יחד עם
`checksums.sha256`. צרפו את החבילה הזו לכרטיס קידום Netlify/SoraFS gateway כדי
שהסוקרים יוכלו לשחזר את מקורות הפרוקסי המדויקים ואת רמזי היעד של Torii בלי
לבנות מחדש. החבילה גם מתעדת האם הותרו bearer tokens שסופקו על ידי הלקוח
(`allow_client_auth`) כדי לשמור את תוכנית ה-rollout וכללי ה-CSP מסונכרנים.

## שלב 1 — בנייה ולינט של הפורטל

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` מריץ אוטומטית את `scripts/write-checksums.mjs`, ומפיק:

- `build/checksums.sha256` — מניפסט SHA256 שמתאים ל-`sha256sum -c`.
- `build/release.json` — מטא-דאטה (`tag`, `generated_at`, `source`) שמוטמעת בכל
  CAR/manifest.

ארכבו את שני הקבצים לצד תקציר ה-CAR כדי שהסוקרים יוכלו להשוות ארטיפקטי תצוגה
מקדימה בלי לבנות מחדש.

## שלב 2 — אריזת הנכסים הסטטיים

הריצו את אורז ה-CAR על תיקיית הפלט של Docusaurus. הדוגמה למטה
כותבת את כל הארטיפקטים תחת `artifacts/devportal/`.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

קובץ ה-JSON הסיכומי מתעד ספירת chunkים, דיגסטים ורמזי תכנון הוכחות
ש-`manifest build` ולוחות המחוונים של CI משתמשים בהם בהמשך.

## שלב 2b — אריזת נלווים של OpenAPI ו-SBOM

DOCS-7 דורש לפרסם את אתר הפורטל, צילום OpenAPI ומטעני SBOM כמניפסטים נפרדים כדי
ש-gateways יוכלו להצמיד כותרות `Sora-Proof`/`Sora-Content-CID` לכל ארטיפקט. עוזר
השחרור (`scripts/sorafs-pin-release.sh`) כבר אורז את תיקיית OpenAPI
(`static/openapi/`) ואת ה-SBOMים שמופקים באמצעות `syft` ל-CARים נפרדים
`openapi.*`/`*-sbom.*` ומקליט את המטא-דאטה ב-
`artifacts/sorafs/portal.additional_assets.json`. כשמריצים את הזרימה הידנית,
חזרו על שלבים 2-4 לכל payload עם הקידומות ותוויות המטא-דאטה שלו
(לדוגמה `--car-out "$OUT"/openapi.car` יחד עם
`--metadata alias_label=docs.sora.link/openapi`). רשמו כל זוג manifest/alias
ב-Torii (אתר, OpenAPI, SBOM של הפורטל, SBOM של OpenAPI) לפני שינוי DNS כדי
שה-gateway יוכל להגיש הוכחות מוצמדות לכל הארטיפקטים שפורסמו.

## שלב 3 — בניית המניפסט

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

כוונו את דגלי מדיניות ה-pin לחלון השחרור שלכם (לדוגמה, `--pin-storage-class
hot` עבור קנריז). גרסת ה-JSON אופציונלית אך נוחה לביקורת קוד.

## שלב 4 — חתימה עם Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

ה-bundle מתעד את דיגסט המניפסט, דיגסטים של ה-chunks, וגיבוב BLAKE3 של טוקן
ה-OIDC בלי לשמור את ה-JWT. שמרו גם את ה-bundle וגם את החתימה הנפרדת; קידומי
production יכולים לעשות שימוש חוזר באותם ארטיפקטים במקום לחתום מחדש.
ריצות מקומיות יכולות להחליף את דגלי הספק ב-`--identity-token-env` (או להגדיר
`SIGSTORE_ID_TOKEN` בסביבה) כאשר עוזר OIDC חיצוני מנפיק את הטוקן.

## שלב 5 — הגשה לרישום ה-pin

הגישו את המניפסט החתום (ואת תוכנית ה-chunk) ל-Torii. בקשו תמיד תקציר כדי
שהרשומה/הוכחת alias שתתקבל תהיה ניתנת לביקורת.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority ih58... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

כאשר מקדמים alias של תצוגה מקדימה או canary (`docs-preview.sora`), חזרו על
ההגשה עם alias ייחודי כדי ש-QA יוכל לאמת את התוכן לפני קידום ל-production.

קישור alias דורש שלושה שדות: `--alias-namespace`, `--alias-name`, ו-
`--alias-proof`. הממשל מפיק את חבילת ההוכחה (base64 או bytes של Norito)
כאשר בקשת ה-alias מאושרת; אחסנו אותה בסודות ה-CI והציגו אותה כקובץ לפני
הפעלת `manifest submit`. השאירו את דגלי ה-alias לא מוגדרים כאשר אתם מתכוונים
רק להצמיד את המניפסט בלי לגעת ב-DNS.

## שלב 5b — יצירת הצעת ממשל

כל מניפסט צריך להגיע עם הצעה מוכנה לפרלמנט כדי שכל אזרח Sora יוכל להציג את
השינוי בלי ללוות אישורים מיוחסים. לאחר שלבי submit/sign, הריצו:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` מתעד את הוראת `RegisterPinManifest` הקנונית, דיגסט
ה-chunk, המדיניות ורמז ה-alias. צרפו אותו לכרטיס הממשל או לפורטל הפרלמנט כדי
שהנציגים יוכלו להשוות את ה-payload בלי לבנות מחדש את הארטיפקטים. מכיוון שהפקודה
אינה נוגעת במפתח סמכות Torii, כל אזרח יכול לנסח את ההצעה מקומית.

## שלב 6 — אימות הוכחות וטלמטריה

לאחר pinning, הריצו את צעדי האימות הדטרמיניסטיים:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- בדקו את `torii_sorafs_gateway_refusals_total` ואת
  `torii_sorafs_replication_sla_total{outcome="missed"}` לאיתור אנומליות.
- הריצו `npm run probe:portal` כדי לבדוק את פרוקסי Try-It והקישורים המתועדים
  מול התוכן שהוצמד זה עתה.
- אספו את ראיות הניטור המתוארות ב-[Publishing & Monitoring](./publishing-monitoring.md)
  כדי ששער הנראות של DOCS-3c יסופק לצד צעדי הפרסום. העוזר מקבל כעת מספר רשומות
  `bindings` (אתר, OpenAPI, SBOM של הפורטל, SBOM של OpenAPI) ומאכף
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` על ה-host היעד באמצעות הגנת
  `hostname` האופציונלית. הקריאה למטה כותבת גם תקציר JSON יחיד וגם את חבילת
  הראיות (`portal.json`, `tryit.json`, `binding.json`, ו-`checksums.sha256`)
  תחת תיקיית השחרור:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## שלב 6a — תכנון אישורי Gateway

גזרו את תוכנית TLS SAN/Challenge לפני יצירת חבילות GAR כדי שצוות ה-gateway
ומאשרי ה-DNS יבחנו את אותן הראיות. העוזר החדש משקף את קלטי האוטומציה של DG-3
בכך שהוא מונה wildcard hosts קנוניים, SANs ל-pretty-host, תוויות DNS-01, ואתגרי
ACME מומלצים:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

הכניסו את ה-JSON לצד חבילת השחרור (או העלו אותו עם כרטיס השינוי) כדי
שמפעילים יוכלו להדביק את ערכי ה-SAN לתצורת
`torii.sorafs_gateway.acme` של Torii ומבקרי GAR יוכלו לאשר את המיפויים
הקנוניים/pretty בלי להריץ מחדש גזירות host. הוסיפו ארגומנטים נוספים של
`--name` לכל סיומת שמקודמת באותו שחרור.

## שלב 6b — גזירת מיפויי מארחים קנוניים

לפני תבניות ה-GAR payloads, רשמו את מיפוי ה-host הדטרמיניסטי לכל alias.
`cargo xtask soradns-hosts` מבצע hash לכל `--name` לתווית הקנונית שלו
(`<base32>.gw.sora.id`), מפיק את ה-wildcard הנדרש (`*.gw.sora.id`), וגוזר את
ה-pretty host (`<alias>.gw.sora.name`). שימרו את הפלט בארטיפקטי השחרור כדי
שסוקרי DG-3 יוכלו להשוות את המיפוי לצד הגשת ה-GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

השתמשו ב-`--verify-host-patterns <file>` כדי להיכשל מהר בכל פעם ש-GAR או JSON
של binding gateway מחסירים אחד מה-hostים הדרושים. העוזר מקבל כמה קבצי אימות,
מה שמקל על lint גם לתבנית ה-GAR וגם ל-`portal.gateway.binding.json` המוצמד
באותה קריאה:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

צרפו את תקציר ה-JSON ואת לוג האימות לכרטיס שינוי ה-DNS/gateway כדי שמבקרים יוכלו
לאשר את ה-hostים הקנוניים, ה-wildcard וה-pretty בלי להריץ מחדש את סקריפטי
הגזירה. הריצו את הפקודה מחדש בכל פעם שנוספים aliases חדשים לחבילה כדי שעדכוני
GAR מאוחרים יותר יירשו את אותו נתיב ראיות.

## שלב 7 — יצירת מתאר מעבר DNS

קידומי production דורשים חבילת שינוי ניתנת לביקורת. לאחר הגשה מוצלחת
(קישור alias), העוזר מפיק `artifacts/sorafs/portal.dns-cutover.json`, הלוכד:

- מטא-דאטה של קישור alias (namespace/name/proof, דיגסט המניפסט, כתובת Torii,
  epoch שהוגש, סמכות);
- הקשר השחרור (tag, תווית alias, נתיבי manifest/CAR, תוכנית chunk, bundle של Sigstore);
- מצביעי אימות (פקודת probe, alias ונקודת קצה של Torii);
- שדות בקרת שינוי אופציונליים (מזהה כרטיס, חלון מעבר, איש קשר ops,
  שם host/zone של production);
- מטא-דאטה של קידום נתיב שמופקת מכותרת `Sora-Route-Binding` המוצמדת
  (host/CID קנוני, נתיבי header ו-binding, פקודות אימות), כך שתרגילי קידום GAR
  ו-fallback מתייחסים לאותן ראיות;
- ארטיפקטי תוכנית נתיב שנוצרו (`gateway.route_plan.json`, תבניות header,
  וכותרות rollback אופציונליות) כך שכרטיסי שינוי ו-hookים של lint ב-CI
  יוכלו לאמת שכל חבילת DG-3 מפנה לתוכניות הקידום/rollback הקנוניות לפני אישור;
- מטא-דאטה אופציונלית לביטול מטמון (נקודת purge, משתנה auth, payload JSON,
  ופקודת `curl` לדוגמה); ו-
- רמזי rollback שמצביעים על המתאר הקודם (תג שחרור ודיגסט מניפסט) כדי שכרטיסי
  שינוי יתעדו מסלול fallback דטרמיניסטי.

כאשר השחרור דורש ניקויי cache, צרו תוכנית קנונית לצד מתאר המעבר:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

צרפו את `portal.cache_plan.json` שהתקבל לחבילת DG-3 כדי שלמפעילים יהיו hosts
ונתיבים דטרמיניסטיים (והרמזים המותאמים לאימות) בעת שליחת בקשות `PURGE`. סעיף
מטא-דאטת ה-cache האופציונלי של המתאר יכול להפנות לקובץ הזה ישירות, וכך להשאיר
את מבקרי בקרת השינוי מיושרים על בדיוק אילו נקודות קצה נשטפות במהלך מעבר.

כל חבילת DG-3 צריכה גם רשימת בדיקה של קידום + rollback. הפיקו אותה באמצעות
`cargo xtask soradns-route-plan` כדי שמבקרי שינוי יוכלו לעקוב אחרי צעדי קדם,
מעבר ו-rollback לכל alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

הפלט `gateway.route_plan.json` כולל hosts קנוניים/pretty, תזכורות בדיקות
בריאות מדורגות, עדכוני GAR binding, ניקויי cache ופעולות rollback. צרפו אותו
עם ארטיפקטי ה-GAR/binding/cutover לפני הגשת כרטיס השינוי כדי שצוות Ops יוכל
לתרגל ולאשר את אותם צעדים מתוסרטים.

`scripts/generate-dns-cutover-plan.mjs` מניע את המתאר הזה ורץ אוטומטית מתוך
`sorafs-pin-release.sh`. כדי לחדש או להתאים אותו ידנית:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

אכלסו את המטא-דאטה האופציונלית דרך משתני סביבה לפני הרצת עוזר ה-pin:

| משתנה | מטרה |
|----------|---------|
| `DNS_CHANGE_TICKET` | מזהה כרטיס שנשמר במתאר. |
| `DNS_CUTOVER_WINDOW` | חלון מעבר ISO8601 (למשל `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | שם host production וה-zone הסמכותי. |
| `DNS_OPS_CONTACT` | כתובת on-call או איש קשר להסלמה. |
| `DNS_CACHE_PURGE_ENDPOINT` | נקודת קצה לניקוי cache שנרשמת במתאר. |
| `DNS_CACHE_PURGE_AUTH_ENV` | משתנה סביבה שמכיל את אסימון ה-purge (ברירת מחדל: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | נתיב למתאר מעבר קודם עבור מטא-דאטת rollback. |

צרפו את ה-JSON לבדיקת שינוי ה-DNS כדי שמאשרים יוכלו לאמת דיגסטים של מניפסט,
קישורי alias ופקודות probe בלי לגרד לוגים של CI. דגלי CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, ו-`--previous-dns-plan` מספקים את אותן דריסות
כאשר מריצים את העוזר מחוץ ל-CI.

## שלב 8 — הפקת שלד zonefile למיישב (אופציונלי)

כאשר חלון המעבר ל-production ידוע, סקריפט השחרור יכול להפיק אוטומטית את שלד
ה-zonefile של SNS ואת קטע ה-resolver. העבירו את רשומות ה-DNS והמטא-דאטה הרצויות
באמצעות משתני סביבה או דגלי CLI; העוזר יקרא את
`scripts/sns_zonefile_skeleton.py` מיד לאחר יצירת מתאר המעבר. ספקו לפחות ערך
A/AAAA/CNAME אחד ואת דיגסט ה-GAR (BLAKE3-256 של payload ה-GAR החתום). אם ה-zone
ושם ה-host ידועים ו-`--dns-zonefile-out` מושמט, העוזר כותב אל
`artifacts/sns/zonefiles/<zone>/<hostname>.json` וממלא את
`ops/soradns/static_zones.<hostname>.json` כקטע ה-resolver.

| משתנה / דגל | מטרה |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | נתיב לשלד ה-zonefile שנוצר. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | נתיב לקטע ה-resolver (ברירת מחדל: `ops/soradns/static_zones.<hostname>.json` כאשר מושמט). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL שמוחל על הרשומות שנוצרו (ברירת מחדל: 600 שניות). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | כתובות IPv4 (מופרדות בפסיקים במשתנה סביבה או דגל CLI חוזר). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | כתובות IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | יעד CNAME אופציונלי. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | פיני SPKI מסוג SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | רשומות TXT נוספות (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | דריסה של תווית גרסת ה-zonefile המחושבת. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | אילוץ חותמת הזמן `effective_at` (RFC3339) במקום תחילת חלון המעבר. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | דריסה של ה-proof שנרשם במטא-דאטה. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | דריסה של ה-CID שנרשם במטא-דאטה. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | מצב הקפאה של Guardian (soft, hard, thawing, monitoring, emergency). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | כרטיס Guardian/council עבור הקפאות. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | חותמת זמן RFC3339 להפשרה. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | הערות הקפאה נוספות (מופרדות בפסיקים במשתנה סביבה או דגל חוזר). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | דיגסט BLAKE3-256 (hex) של payload ה-GAR החתום. חובה כאשר קיימים gateway bindings. |

Workflow של GitHub Actions קורא את הערכים האלה מסודות המאגר כך שכל pin של production
מפיק אוטומטית את ארטיפקטי ה-zonefile. הגדרו את הסודות הבאים (מחרוזות יכולות
להכיל רשימות מופרדות בפסיקים עבור שדות מרובי ערכים):

| סוד | מטרה |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | שם host/zone של production שמועברים לעוזר. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | כתובת on-call שנשמרת במתאר. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | רשומות IPv4/IPv6 לפרסום. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | יעד CNAME אופציונלי. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | פיני SPKI בבסיס64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | רשומות TXT נוספות. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | מטא-דאטת הקפאה שנרשמת בשלד. |
| `DOCS_SORAFS_GAR_DIGEST` | דיגסט BLAKE3 hex-encoded של payload ה-GAR החתום. |

בעת הפעלת `.github/workflows/docs-portal-sorafs-pin.yml`, ספקו את הקלטים
`dns_change_ticket` ו-`dns_cutover_window` כדי שהמתאר/zonefile יירשו את מטא-דאטת
חלון השינוי הנכונה. השאירו אותם ריקים רק כאשר מריצים dry runs.

הרצה טיפוסית (תואמת לריצת SN-7):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  ...other flags...
```

העוזר נושא אוטומטית את כרטיס השינוי כרשומת TXT
ויורש את תחילת חלון המעבר כחותמת הזמן `effective_at` אלא אם כן נדרס.
לתהליך התפעולי המלא, ראו
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### תבנית כותרות Gateway

עוזר הפריסה מפיק גם את `portal.gateway.headers.txt` ואת
`portal.gateway.binding.json`, שני ארטיפקטים שמספקים את דרישת
gateway-content-binding של DG-3:

- `portal.gateway.headers.txt` מכיל את בלוק כותרות ה-HTTP המלא (כולל
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, ואת תיאור
  `Sora-Route-Binding`) שה-gateways בקצה חייבים להצמיד לכל תגובה.
- `portal.gateway.binding.json` מתעד את אותו מידע בצורה קריאה למכונה כך
  שכרטיסי שינוי ואוטומציה יוכלו להשוות host/CID bindings בלי לגרד פלט shell.

הם נוצרים אוטומטית באמצעות
`cargo xtask soradns-binding-template`
ומתעדים את ה-alias, דיגסט המניפסט ושם ה-gateway שסופקו ל-`sorafs-pin-release.sh`.
כדי ליצור מחדש או להתאים את בלוק הכותרות, הריצו:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

העבירו `--csp-template`, `--permissions-template`, או `--hsts-template` כדי
לדרוס את תבניות הכותרות ברירת המחדל כאשר פריסה מסוימת דורשת הנחיות נוספות;
שלבו אותן עם מתגי `--no-*` הקיימים כדי להסיר כותרת לחלוטין.

צרפו את קטע הכותרות לבקשת שינוי ה-CDN והזינו את מסמך ה-JSON לתוך צנרת האוטומציה
של ה-gateway כך שהקידום בפועל יתאים לראיות השחרור.

סקריפט השחרור מריץ את עוזר האימות אוטומטית כדי שכרטיסי DG-3 תמיד יכללו ראיות
עדכניות. הריצו אותו ידנית בכל פעם שאתם משנים את JSON ה-binding ביד:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

הפקודה מפענחת את payload של `Sora-Proof` המוצמד, מוודאת שמטא-דאטת
`Sora-Route-Binding` תואמת ל-CID של המניפסט ול-hostname, ונכשלת במהירות אם
כותרת כלשהי סוטה. ארכבו את פלט הקונסול לצד שאר ארטיפקטי הפריסה בכל פעם שאתם
מריצים את הפקודה מחוץ ל-CI כך שסוקרי DG-3 יקבלו הוכחה שה-binding אומת לפני
המעבר.

> **שילוב מתאר DNS:** `portal.dns-cutover.json` כעת מטמיע סעיף
> `gateway_binding` שמצביע על הארטיפקטים הללו (נתיבים, CID של תוכן,
> מצב הוכחה, ותבנית הכותרות הליטרלית) **וגם** סעיף `route_plan` שמפנה אל
> `gateway.route_plan.json` יחד עם תבניות הכותרות הראשיות וה-rollback.
> כללו את הבלוקים האלה בכל כרטיס שינוי DG-3 כדי שסוקרים יוכלו להשוות את
> ערכי `Sora-Name`/`Sora-Proof`/`CSP` המדויקים ולאשר שתוכניות הקידום/rollback
> תואמות לחבילת הראיות בלי לפתוח את ארכיון הבנייה.

## שלב 9 — הרצת מוניטורי פרסום

משימת מפת הדרכים **DOCS-3c** דורשת ראיות מתמשכות לכך שהפורטל, פרוקסי Try it
ו-gateway bindings נשארים בריאים לאחר שחרור. הריצו את המוניטור המרוכז מיד
לאחר שלבים 7-8 וחברו אותו לפרובי השגרה שלכם:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` טוען את קובץ הקונפיג (ראו
  `docs/portal/docs/devportal/publishing-monitoring.md` עבור הסכימה) ומריץ
  שלוש בדיקות: בדיקות נתיב פורטל + אימות CSP/Permissions-Policy,
  בדיקות פרוקסי Try it (עם אופציה לפגוע ב-endpoint `/metrics`), ומאמת
  gateway binding (`cargo xtask soradns-verify-binding`) שכעת מאכף נוכחות
  + ערך צפוי של Sora-Content-CID לצד בדיקות alias/manifest.
- הפקודה יוצאת בקוד לא אפס בכל פעם שבדיקה נכשלת כדי ש-CI, cron jobs, או
  מפעילי runbook יוכלו לעצור שחרור לפני קידום aliases.
- העברת `--json-out` כותבת payload JSON מסכם יחיד עם סטטוס לפי יעד;
  `--evidence-dir` מפיקה `summary.json`, `portal.json`, `tryit.json`,
  `binding.json`, ו-`checksums.sha256` כך שמבקרי ממשל יוכלו להשוות את
  התוצאות בלי להריץ מחדש את המוניטורים. ארכבו את התיקייה הזו תחת
  `artifacts/sorafs/<tag>/monitoring/` לצד bundle Sigstore ומתאר מעבר ה-DNS.
- כללו את פלט המוניטור, יצוא Grafana (`dashboards/grafana/docs_portal.json`),
  ומזהה תרגיל Alertmanager בכרטיס השחרור כדי שניתן יהיה לאמת את ה-SLO של
  DOCS-3c בהמשך. פלייבוק מוניטור הפרסום הייעודי נמצא ב-
  `docs/portal/docs/devportal/publishing-monitoring.md`.

בדיקות פורטל דורשות HTTPS ודוחות על בסיס `http://` אלא אם `allowInsecureHttp`
מוגדר בקונפיג המוניטור; שמרו יעדי production/staging על TLS ואפשרו את ההחרגה
רק עבור תצוגות מקדימות מקומיות.

הפעילו את המוניטור דרך `npm run monitor:publishing` ב-Buildkite/cron כאשר
הפורטל חי. אותה פקודה, המכוונת לכתובות production, מזינה את בדיקות הבריאות
השוטפות שעליהן נשענים SRE/Docs בין שחרורים.

## אוטומציה עם `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` מרכז את שלבים 2-6. הוא:

1. מארכב את `build/` ל-tarball דטרמיניסטי,
2. מריץ `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   ו-`proof verify`,
3. מריץ אופציונלית את `manifest submit` (כולל קישור alias) כאשר קיימות
   אישורי Torii, ו-
4. כותב את `artifacts/sorafs/portal.pin.report.json`, את
  `portal.pin.proposal.json` האופציונלי, את מתאר מעבר ה-DNS (לאחר submissions),
  ואת חבילת ה-gateway binding (`portal.gateway.binding.json` יחד עם בלוק הכותרות
  הטקסטואלי) כך שצוותי ממשל, רשת ו-ops יוכלו להשוות את חבילת הראיות בלי לגרד
  לוגי CI.

הגדירו `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, ו-(אופציונלי)
`PIN_ALIAS_PROOF_PATH` לפני הפעלת הסקריפט. השתמשו ב-`--skip-submit` להרצות dry
run; ה-workflow של GitHub שמפורט בהמשך מפעיל זאת דרך הקלט `perform_submit`.

## שלב 8 — פרסום מפרטי OpenAPI וחבילות SBOM

DOCS-7 דורש שה-build של הפורטל, מפרט OpenAPI וארטיפקטי SBOM יעברו דרך אותה
צנרת דטרמיניסטית. העוזרים הקיימים מכסים את שלושתם:

1. **ליצור מחדש ולחתום על המפרט.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   העבירו תווית שחרור דרך `--version=<label>` בכל פעם שאתם רוצים לשמר צילום
   היסטורי (לדוגמה `2025-q3`). העוזר כותב את הצילום ל-
   `static/openapi/versions/<label>/torii.json`, משקף אותו לתוך
   `versions/current`, ומקליט את המטא-דאטה (SHA-256, סטטוס מניפסט,
   וחותמת הזמן המעודכנת) ב-`static/openapi/versions.json`. פורטל המפתחים קורא
   את האינדקס הזה כדי שהפאנלים של Swagger/RapiDoc יוכלו להציג בורר גרסאות
   ולהציג את פרטי הדיגסט/חתימה המתאימים. השמטת `--version` משאירה את תוויות
   השחרור הקודמות ומרעננת רק את מצביעי `current` ו-`latest`.

   המניפסט מתעד דיגסטים של SHA-256/BLAKE3 כדי שה-gateway יוכל להצמיד כותרות
   `Sora-Proof` עבור `/reference/torii-swagger`.

2. **להפיק SBOMים של CycloneDX.** צינור השחרור כבר מצפה ל-SBOMים מבוססי syft
   לפי `docs/source/sorafs_release_pipeline_plan.md`. השאירו את הפלט ליד
   ארטיפקטי ה-build:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **לארוז כל payload ל-CAR.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   עקבו אחר אותם שלבי `manifest build` / `manifest sign` כמו באתר הראשי,
   והתאימו את ה-aliases לכל ארטיפקט (למשל `docs-openapi.sora` למפרט ו-
   `docs-sbom.sora` לחבילת ה-SBOM החתומה). שמירה על aliases נפרדים משאירה
   את הוכחות SoraDNS, GARים וכרטיסי rollback ממוקדים בדיוק ב-payload.

4. **להגיש ולקשר.** השתמשו מחדש באותה סמכות וב-bundle Sigstore, אבל תעדו את
   צמד ה-alias ברשימת הבדיקה של השחרור כדי שמבקרים יוכלו לעקוב איזו כתובת
   Sora ממופה לאיזה דיגסט מניפסט.

ארכוב מניפסטי המפרט/SBOM לצד build הפורטל מבטיח שכל כרטיס שחרור כולל את סט
הארטיפקטים המלא בלי להריץ מחדש את האורז.

### עוזר אוטומציה (סקריפט CI/אריזה)

`./ci/package_docs_portal_sorafs.sh` מממש את שלבים 1-8 כדי שפריט מפת הדרכים
**DOCS-7** יוכל לרוץ בפקודה אחת. העוזר:

- מריץ את הכנת הפורטל הנדרשת (`npm ci`, סנכרון OpenAPI/Norito, בדיקות widgets);
- מפיק את ה-CARים וה-manifest pairs של הפורטל, OpenAPI ו-SBOM דרך `sorafs_cli`;
- מריץ אופציונלית `sorafs_cli proof verify` (`--proof`) וחתימת Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- מפיל את כל הארטיפקטים תחת `artifacts/devportal/sorafs/<timestamp>/` וכותב
  `package_summary.json` כדי שכלי CI/שחרור יוכלו לצרוך את החבילה; ו-
- מרענן את `artifacts/devportal/sorafs/latest` כדי להצביע על הריצה האחרונה.

דוגמה (צינור מלא עם Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

דגלים שכדאי להכיר:

- `--out <dir>` – דריסת שורש הארטיפקטים (ברירת מחדל משאירה תיקיות עם חותמת זמן).
- `--skip-build` – שימוש חוזר ב-`docs/portal/build` קיים (שימושי כאשר CI לא יכול
  לבנות מחדש בגלל mirrors לא מקוונים).
- `--skip-sync-openapi` – דילוג על `npm run sync-openapi` כאשר `cargo xtask openapi`
  לא יכול להגיע ל-crates.io.
- `--skip-sbom` – הימנעות מקריאה ל-`syft` כאשר הבינארי אינו מותקן (הסקריפט מציג
  אזהרה במקום).
- `--proof` – להריץ `sorafs_cli proof verify` עבור כל זוג CAR/manifest. payloads
  מרובי קבצים עדיין דורשים תמיכת chunk-plan ב-CLI, לכן השאירו את הדגל הזה כבוי
  אם אתם מקבלים שגיאות `plan chunk count` ואמתו ידנית ברגע שה-gate upstream נוחת.
- `--sign` – להפעיל `sorafs_cli manifest sign`. ספקו טוקן עם
  `SIGSTORE_ID_TOKEN` (או `--sigstore-token-env`) או תנו ל-CLI לאחזר אותו
  באמצעות `--sigstore-provider/--sigstore-audience`.

כאשר משגרים ארטיפקטים ל-production השתמשו ב-`docs/portal/scripts/sorafs-pin-release.sh`.
הוא כעת אורז את הפורטל, OpenAPI ו-SBOMs, חותם על כל מניפסט, ומקליט מטא-דאטה של
ארטיפקטים נוספים ב-`portal.additional_assets.json`. העוזר מבין את אותן אפשרויות
אופציונליות שמשמשות את אורז ה-CI בנוסף למתגים החדשים
`--openapi-*`, `--portal-sbom-*`, ו-`--openapi-sbom-*` כך שתוכלו להקצות tuples
של alias לכל ארטיפקט, לדרוס את מקור ה-SBOM דרך `--openapi-sbom-source`, לדלג
על payloads מסוימים (`--skip-openapi`/`--skip-sbom`), ולהצביע על בינארי `syft`
שאינו ברירת מחדל באמצעות `--syft-bin`.

הסקריפט מציף כל פקודה שהוא מריץ; העתיקו את הלוג לכרטיס השחרור לצד
`package_summary.json` כדי שמבקרים יוכלו להשוות digests של CAR, מטא-דאטת plan,
ו-hashes של bundle Sigstore בלי לחפש פלט shell אד-הוק.

## שלב 9 — אימות Gateway + SoraDNS

לפני הכרזה על מעבר, הוכיחו שה-alias החדש נפתר דרך SoraDNS וש-gateways מצמידים
הוכחות עדכניות:

1. **הריצו את שער ה-probe.** `ci/check_sorafs_gateway_probe.sh` מפעיל
   `cargo xtask sorafs-gateway-probe` מול הדמו fixtures שב-
   `fixtures/sorafs_gateway/probe_demo/`. לפריסות אמיתיות, הצביעו על hostname
   היעד:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   ה-probe מפענח את `Sora-Name`, `Sora-Proof`, ו-`Sora-Proof-Status` לפי
   `docs/source/sorafs_alias_policy.md` ונכשל כאשר דיגסט המניפסט, TTLים או
   GAR bindings סוטים.

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **תעדו ראיות לתרגיל.** עבור תרגילי מפעילים או dry runs של PagerDuty,
   עטפו את ה-probe ב-`scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   devportal-rollout -- …`. העוטף שומר כותרות/לוגים תחת
   `artifacts/sorafs_gateway_probe/<stamp>/`, מעדכן את `ops/drill-log.md`,
   ו-(אופציונלית) מפעיל hooks ל-rollback או payloads של PagerDuty. הגדירו
   `--host docs.sora` כדי לאמת את מסלול SoraDNS במקום לקבע IP.

3. **אמתו DNS bindings.** כאשר הממשל מפרסם את הוכחת ה-alias, תעדו את קובץ ה-GAR
   שמופיע ב-probe (`--gar`) וצרפו אותו לראיות השחרור. בעלי resolver יכולים
   לשקף את אותו קלט דרך `tools/soradns-resolver` כדי לוודא שרשומות cache מכבדות
   את המניפסט החדש. לפני צירוף ה-JSON, הריצו
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   כדי שמיפוי ה-host הדטרמיניסטי, מטא-דאטת המניפסט ותוויות הטלמטריה יאומתו
   אופליין. העוזר יכול להפיק סיכום `--json-out` לצד ה-GAR החתום כך שלמבקרים יהיו
   ראיות ניתנות לאימות בלי לפתוח את הבינארי.
  כאשר מנסחים GAR חדש, העדיפו
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (עברו ל-`--manifest-cid <cid>` רק כאשר קובץ מניפסט אינו זמין). העוזר כעת
  גוזר את ה-CID **וגם** את דיגסט ה-BLAKE3 ישירות מ-JSON המניפסט, מקצץ רווחים,
  מסיר כפילויות של דגלי `--telemetry-label`, ממיין את התוויות, ומפיק את
  תבניות CSP/HSTS/Permissions-Policy כברירת מחדל לפני כתיבת ה-JSON כך שה-payload
  נשאר דטרמיניסטי גם כאשר מפעילים לוכדים תוויות מקונכיות שונות.

4. **עקבו אחרי מדדי alias.** שמרו על
   `torii_sorafs_alias_cache_refresh_duration_ms`
   ו-`torii_sorafs_gateway_refusals_total{profile="docs"}` על המסך בזמן שה-probe
   רץ; שתי הסדרות מתוארות ב-
   `dashboards/grafana/docs_portal.json`.

## שלב 10 — ניטור ואיגוד ראיות

- **דאשבורדים.** ייצאו את `dashboards/grafana/docs_portal.json` (SLOs של הפורטל),
  `dashboards/grafana/sorafs_gateway_observability.json` (השהיית gateway +
  בריאות הוכחות), ו-`dashboards/grafana/sorafs_fetch_observability.json`
  (בריאות orchestrator) עבור כל שחרור. צרפו את יצואי ה-JSON לכרטיס השחרור כדי
  שמבקרים יוכלו לשחזר את שאילתות Prometheus.
- **ארכיוני probe.** שמרו על `artifacts/sorafs_gateway_probe/<stamp>/` ב-git-annex
  או ב-bucket הראיות שלכם. כללו את תקציר ה-probe, הכותרות, ו-payload של
  PagerDuty שנתפס ע"י סקריפט הטלמטריה.
- **חבילת שחרור.** אחסנו את תקצירי ה-CAR של הפורטל/SBOM/OpenAPI, bundles של
  המניפסט, חתימות Sigstore, `portal.pin.report.json`, לוגים של Try-It,
  ודוחות בדיקת קישורים תחת תיקייה אחת עם חותמת זמן (למשל
  `artifacts/sorafs/devportal/20260212T1103Z/`).
- **יומן תרגילים.** כאשר probes הם חלק מתרגיל, תנו ל-
  `scripts/telemetry/run_sorafs_gateway_probe.sh` לצרף ל-`ops/drill-log.md`
  כך שאותן ראיות יספקו את דרישת הכאוס SNNet-5.
- **קישורי כרטיסים.** הפנו לפאנלים של Grafana או ליצואי PNG מצורפים בכרטיס
  השינוי, יחד עם נתיב דו"ח ה-probe, כדי שמבקרי שינוי יוכלו להצליב את ה-SLOs
  בלי גישה ל-shell.

## שלב 11 — תרגיל אחזור רב-מקור וראיות scoreboard

פרסום ל-SoraFS דורש כעת ראיות אחזור רב-מקור (DOCS-7/SF-6) לצד הוכחות ה-DNS/
Gateway למעלה. לאחר הצמדת המניפסט:

1. **הריצו `sorafs_fetch` מול המניפסט החי.** השתמשו באותם ארטיפקטי plan/manifest
   שהופקו בשלבים 2-3 יחד עם אישורי ה-gateway שהונפקו לכל ספק. שמרו כל פלט כך
   שמבקרים יוכלו לשחזר את נתיב קבלת ההחלטות של ה-orchestrator:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - משכו תחילה את adverts של הספקים שאליהם המניפסט מפנה (למשל
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     והעבירו אותם דרך `--provider-advert name=path` כדי שה-scoreboard יוכל
     להעריך חלונות יכולת בצורה דטרמיניסטית. השתמשו ב-
     `--allow-implicit-provider-metadata` **רק** כאשר משחזרים fixtures ב-CI;
     תרגילי production חייבים לצטט את adverts החתומים שנחתו עם ה-pin.
   - כאשר המניפסט מפנה לאזורים נוספים, חזרו על הפקודה עם tuples הספק המתאימות
     כך שלכל cache/alias תהיה תוצאת fetch תואמת.

2. **ארכבו את הפלטים.** אחסנו את `scoreboard.json`, `providers.ndjson`,
   `fetch.json`, ו-`chunk_receipts.ndjson` תחת תיקיית ראיות השחרור. קבצים אלו
   מתעדים את משקלי העמיתים, תקציב הנסיונות החוזרים, EWMA של זמן השהיה,
   וקבלות לפי chunk שהחבילה הממשלית צריכה לשמור עבור SF-7.

3. **עדכנו טלמטריה.** ייבאו את פלטי ה-fetch לדאשבורד **SoraFS Fetch
   Observability** (`dashboards/grafana/sorafs_fetch_observability.json`),
   תוך מעקב אחרי `torii_sorafs_fetch_duration_ms`/`_failures_total` ופאנלי
   הטווחים של הספקים כדי לאתר אנומליות. קשרו את צילומי הפאנלים של Grafana
   לכרטיס השחרור לצד נתיב ה-scoreboard.

4. **בצעו smoke לכללי ההתראות.** הריצו
   `scripts/telemetry/test_sorafs_fetch_alerts.sh` כדי לאמת את חבילת ההתראות
   של Prometheus לפני סגירת השחרור. צרפו את פלט promtool לכרטיס כדי שסוקרי
   DOCS-7 יוכלו לאשר שהתראות עצירה וספק איטי נשארות דרוכות.

5. **חברו ל-CI.** Workflow ה-pin של הפורטל שומר שלב `sorafs_fetch` מאחורי
   הקלט `perform_fetch_probe`; הפעילו אותו עבור ריצות staging/production כדי
   שראיות ה-fetch יופקו לצד חבילת המניפסט בלי התערבות ידנית. תרגילים מקומיים
   יכולים להשתמש באותו סקריפט על ידי ייצוא טוקני gateway והגדרת
   `PIN_FETCH_PROVIDERS` לרשימת ספקים מופרדת בפסיקים.

## קידום, נראות ו-rollback

1. **קידום:** שמרו על aliases נפרדים ל-staging ול-production. קדמו על ידי הרצה
   מחודשת של `manifest submit` עם אותו מניפסט/bundle והחלפת
   `--alias-namespace/--alias-name` כדי להצביע על ה-alias של production. כך
   נמנעת בנייה מחדש או חתימה מחדש לאחר ש-QA מאשר את ה-pin של staging.
2. **ניטור:** ייבאו את לוח המחוונים של pin-registry
   (`docs/source/grafana_sorafs_pin_registry.json`) יחד עם probes ספציפיים
   לפורטל (ראו `docs/portal/docs/devportal/observability.md`). הגדירו התראות על
   סטיית checksum, כשלי probes או קפיצות ניסיונות הוכחה.
3. **Rollback:** כדי להחזיר לאחור, הגישו מחדש את המניפסט הקודם (או פרשו את
   ה-alias הנוכחי) באמצעות `sorafs_cli manifest submit --alias ... --retire`.
   שמרו תמיד על החבילה והתקציר האחרון הידוע כטוב כדי שניתן יהיה לשחזר הוכחות
   rollback אם לוגי ה-CI מתחלפים.

## תבנית זרימת עבודה ב-CI

לפחות, הצנרת שלכם צריכה:

1. בנייה ולינט (`npm ci`, `npm run build`, יצירת checksum).
2. אריזה (`car pack`) וחישוב מניפסטים.
3. חתימה באמצעות טוקן OIDC של ה-job (`manifest sign`).
4. העלאת ארטיפקטים (CAR, מניפסט, bundle, plan, תקצירים) לצורך ביקורת.
5. הגשה לרישום ה-pin:
   - Pull requests → `docs-preview.sora`.
   - Tags / ענפים מוגנים → קידום alias production.
6. הרצת probes ושערי אימות הוכחות לפני יציאה.

`.github/workflows/docs-portal-sorafs-pin.yml` מחבר את כל הצעדים האלו יחד
לשחרורים ידניים. ה-workflow:

- בונה ובודק את הפורטל,
- אורז את ה-build דרך `scripts/sorafs-pin-release.sh`,
- חותם ומאמת את bundle המניפסט באמצעות GitHub OIDC,
- מעלה את תקצירי CAR/manifest/bundle/plan/proof כ-artifacts, ו-
- (אופציונלית) מגיש את המניפסט + קישור alias כאשר סודות קיימים.

הגדירו את סודות/משתני המאגר הבאים לפני הפעלת ה-job:

| שם | מטרה |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | שרת Torii שחושף `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | מזהה epoch שנרשם עם submissions. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | סמכות חתימה להגשת המניפסט. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | צמד alias שנקשר למניפסט כאשר `perform_submit` הוא `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | חבילת הוכחת alias בקידוד Base64 (אופציונלי; השמיטו כדי לדלג על קישור alias). |
| `DOCS_ANALYTICS_*` | נקודות קצה אנליטיקה/פרובים קיימות שממוחזרות ב-workflows אחרים. |

הפעילו את ה-workflow דרך ממשק Actions:

1. ספקו `alias_label` (למשל `docs.sora.link`), `proposal_alias` אופציונלי,
   ודריסת `release_tag` אופציונלית.
2. השאירו את `perform_submit` לא מסומן כדי להפיק ארטיפקטים בלי לגעת ב-Torii
   (שימושי להרצות dry run) או סמנו אותו כדי לפרסם ישירות ל-alias המוגדר.

`docs/source/sorafs_ci_templates.md` עדיין מתעד את עוזרי ה-CI הכלליים
לפרויקטים מחוץ לריפו הזה, אך ה-workflow של הפורטל צריך להיות מועדף לשחרורים
שגרתיים.

## רשימת בדיקה

- [ ] `npm run build`, `npm run test:*`, ו-`npm run check:links` ירוקים.
- [ ] `build/checksums.sha256` ו-`build/release.json` נשמרו בארטיפקטים.
- [ ] CAR, plan, מניפסט ותקציר נוצרו תחת `artifacts/`.
- [ ] Bundle Sigstore + חתימה מנותקת נשמרו עם הלוגים.
- [ ] `portal.manifest.submit.summary.json` ו-`portal.manifest.submit.response.json`
      נשמרו כאשר מתבצעות submissions.
- [ ] `portal.pin.report.json` (ו-`portal.pin.proposal.json` האופציונלי)
      נשמרו לצד ארטיפקטי CAR/manifest.
- [ ] לוגים של `proof verify` ו-`manifest verify-signature` נשמרו.
- [ ] לוחות Grafana מעודכנים + פרובי Try-It הצליחו.
- [ ] הערות rollback (מזהה מניפסט קודם + דיגסט alias) צורפו לכרטיס השחרור.
