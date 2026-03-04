---
lang: he
direction: rtl
source: docs/portal/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d9fe9e2c2763fd83237dc2343fe9b0c6682d662677c6217afa00552444f44283
source_last_modified: "2025-12-27T09:12:05.231822+00:00"
translation_last_reviewed: 2026-01-30
---

# פורטל המפתחים של SORA Nexus

תיקייה זו מארחת את סביבת העבודה של Docusaurus עבור פורטל המפתחים האינטראקטיבי. הפורטל מאגד מדריכי Norito, קוויקסטארטים ל‑SDK ורפרנס OpenAPI שנוצר באמצעות `cargo xtask openapi`, ומעטף אותם במיתוג SORA Nexus המשמש בתוכנית התיעוד.

## דרישות מוקדמות

- Node.js 18.18 ומעלה (קו בסיס של Docusaurus v3).
- Yarn 1.x או npm 9+ לניהול חבילות.
- שרשרת כלים של Rust (נדרשת לסקריפט סנכרון OpenAPI).

## אתחול

```bash
cd docs/portal
npm install    # or yarn install
```

## סקריפטים זמינים

| פקודה | תיאור |
|---------|-------------|
| `npm run start` / `yarn start` | מפעיל שרת פיתוח מקומי עם טעינה חמה (ברירת מחדל `http://localhost:3000`). |
| `npm run build` / `yarn build` | מייצר בילד production ב‑`build/`. |
| `npm run serve` / `yarn serve` | מגיש את הבילד האחרון מקומית (שימושי לבדיקות smoke). |
| `npm run docs:version -- <label>` | יוצר snapshot לתיעוד הנוכחי ב‑`versioned_docs/version-<label>` (עטיפה סביב `docusaurus docs:version`). |
| `npm run sync-openapi` / `yarn sync-openapi` | מייצר מחדש את `static/openapi/torii.json` באמצעות `cargo xtask openapi` (העבר `--mirror=<label>` כדי להעתיק את ה‑spec גם לסנפשוטים נוספים). |
| `npm run tryit-proxy` | מפעיל את פרוקסי ה‑staging שמניע את קונסולת “Try it” (ראו הגדרות בהמשך). |
| `npm run probe:tryit-proxy` | מריץ בדיקת `/healthz` ובקשת דוגמה מול הפרוקסי (עוזר ל‑CI/ניטור). |
| `npm run manage:tryit-proxy -- <update|rollback>` | מעדכן או משחזר את יעד ה‑`.env` של הפרוקסי עם גיבוי. |
| `npm run sync-i18n` | מבטיח שקיימים סטאבים לתרגומים יפנית, עברית, ספרדית, פורטוגזית, צרפתית, רוסית, ערבית ואורדו תחת `i18n/`. |
| `npm run sync-norito-snippets` | מייצר מחדש דוגמאות Kotodama נבחרות ותוספים להורדה (מופעל גם אוטומטית ע״י תוסף שרת הפיתוח). |
| `npm run test:tryit-proxy` | מריץ את בדיקות היחידה של הפרוקסי באמצעות Node (`node --test`). |

סקריפט סנכרון OpenAPI דורש ש‑`cargo xtask openapi` יהיה זמין מהרוט של הריפו. הוא מפיק JSON דטרמיניסטי אל `static/openapi/` וכעת מצפה שהנתב של Torii יחשוף spec חי (השתמשו ב‑`cargo xtask openapi --allow-stub` רק ליצירת פלט placeholder חירום).

## גרסאות תיעוד & Snapshots של OpenAPI

- **יצירת גרסת תיעוד:** הריצו `npm run docs:version -- 2025-q3` (או תג מוסכם אחר). התחייבו לקבצים שנוצרו: `versioned_docs/version-<label>`, `versioned_sidebars`, ו‑`versions.json`. תפריט הגרסאות בניווט מציג את הסנפשוט החדש אוטומטית.
- **סנכרון OpenAPI:** לאחר יצירת גרסה, רעננו את ה‑spec וה‑manifest הקנוניים עם `cargo xtask openapi --sign <path-to-ed25519-key>`, ואז צרו snapshot תואם באמצעות `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest`. הסקריפט כותב `static/openapi/versions/2025-q3/torii.json`, ממראה את ה‑spec אל `versions/current/torii.json`, מעדכן את `versions.json`, מרענן את `/openapi/torii.json`, ומשכפל את `manifest.json` החתום לכל תיקיית גרסה כדי שהיסטוריית ה‑spec תישא את אותו metadata של מקוריות. ניתן להעביר מספר דגלי `--mirror=<label>` כדי להעתיק את ה‑spec הטרי לסנפשוטים היסטוריים נוספים.
- **ציפיות CI:** קומיטים שנוגעים בתיעוד צריכים לכלול גם את עדכון הגרסה (כשצריך) וגם snapshots מעודכנים של OpenAPI, כדי ש‑Swagger/RapiDoc/Redoc יוכלו להחליף בין specs היסטוריים ללא שגיאות fetch.
- **אכיפת manifest:** הסקריפט `sync-openapi` נכשל אם `manifest.json` החתום חסר, פגום או לא תואם ל‑spec החדש, כך שסנפשוטים לא חתומים לא מתפרסמים כברירת מחדל. הריצו שוב `cargo xtask openapi --sign <key>` כדי לרענן את המניפסט הקנוני והריצו את הסנכרון מחדש. השתמשו ב‑`--allow-unsigned` רק לתצוגות מקומיות (ה‑CI עדיין מריץ `ci/check_openapi_spec.sh` שמייצר מחדש את ה‑spec ומאמת את המניפסט).

## מבנה

```
docs/portal/
├── docs/                 # תוכן Markdown/MDX לפורטל
├── i18n/                 # דריסות לוקאליות (ja/he) שנוצרות ע"י sync-i18n
├── src/                  # דפי/רכיבי React (שלד placeholder)
├── static/               # נכסים סטטיים (כולל OpenAPI JSON)
├── scripts/              # סקריפטים מסייעים (סנכרון OpenAPI)
├── docusaurus.config.js  # תצורת האתר הראשית
└── sidebars.js           # מודל ניווט / סרגל צד
```

### הגדרת Try it proxy

סנדבוקס “Try it” מנתב בקשות דרך `scripts/tryit-proxy.mjs`. הגדירו את הפרוקסי באמצעות משתני סביבה לפני ההפעלה:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_BEARER="sora-dev-token"          # optional
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run tryit-proxy
```

- `TRYIT_PROXY_LISTEN` (ברירת מחדל `127.0.0.1:8787`) שולט בכתובת ההאזנה.
- `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` מגדירים את מגביל הקצב בזיכרון (ברירת מחדל 60 בקשות ל‑60 שניות).
- הגדירו `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` כדי להעביר את כותרת `Authorization` של הלקוח במקום להסתמך על Bearer ברירת המחדל.
- הפרוקסי מסרב לעלות אם `static/openapi/torii.json` חורג מהמניפסט החתום ב‑`static/openapi/manifest.json`. הריצו `npm run sync-openapi -- --latest` לרענון ה‑spec; ייצאו `TRYIT_PROXY_ALLOW_STALE_SPEC=1` רק כחריג חירום (נרשמת אזהרה והפרוקסי עולה בכל זאת).
- הריצו `npm run probe:tryit-proxy` לבדיקת smoke לפרוקסי, והכניסו את הפקודה למשימות ניטור; `npm run manage:tryit-proxy -- update` מפשט סבבי החלפת יעד Torii תוך שמירת גיבוי `.env` לרולבק.
- `TRYIT_PROXY_PROBE_METRICS_FILE` ו‑`TRYIT_PROXY_PROBE_LABELS` מאפשרים לפלט `probe_success`/`probe_duration_seconds` בפורמט textfile של Prometheus כדי לחבר את בדיקת הבריאות ל‑node_exporter או לכל אספן אצווה אחר.
- Prometheus חושף היסטוגרמות `tryit_proxy_request_duration_ms_bucket`/`_count`/`_sum`; דשבורד Grafana של הפורטל משתמש בהן למעקב SLO של p95/p99 (`dashboards/grafana/docs_portal.json`).

### התחברות OAuth באמצעות device-code

כאשר הפורטל חשוף מחוץ לרשתות אמון, הגדירו OAuth device authorization כדי שסוקרים לא יגעו בטוקנים ארוכי טווח של Torii. ייצאו את המשתנים שלמטה לפני `npm run start` או `npm run build`:

| משתנה | הערות |
|----------|-------|
| `DOCS_OAUTH_DEVICE_CODE_URL` / `DOCS_OAUTH_TOKEN_URL` | נקודות קצה של OAuth המיישמות Device Authorization Grant. |
| `DOCS_OAUTH_CLIENT_ID` | מזהה לקוח שנרשם לתצוגת התיעוד. |
| `DOCS_OAUTH_SCOPE` / `DOCS_OAUTH_AUDIENCE` | מחרוזות scope/audience אופציונליות להגבלת הטוקן המונפק. |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | מרווח מינימלי לפולינג שמאוכף ע״י הווידג׳ט (ברירת מחדל 5000ms; ערכים נמוכים יותר נדחים). |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` / `DOCS_OAUTH_TOKEN_TTL_SECONDS` | חלונות תפוגה חלופיים כאשר השרת לא מספק `expires_in`. |
| `DOCS_OAUTH_ALLOW_INSECURE` | הגדירו `1` רק לפיתוח מקומי כדי לעקוף את הגארד בגובה הבילד. בניית production נכשלת אם חסרים משתנים או אם ערכי ה‑TTL חורגים מהתקציב. |

דוגמה:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
```

לאחר שהבילד קולט את הערכים, קונסולת Try it מציגה פאנל כניסה שמציג את device code, מבצע פולינג ל‑token endpoint, ממלא את שדה ה‑Bearer אוטומטית ומנקה את הטוקן עם תפוגתו. הפורטל מסרב לעלות אם תצורת OAuth אינה מלאה או אם TTL של הטוקן/המכשיר חורג מתקציב האבטחה; השתמשו ב‑`DOCS_OAUTH_ALLOW_INSECURE=1` רק בתצוגות מקומיות חד‑פעמיות שבהן גישה אנונימית מקובלת. הפרוקסי עדיין מכבד override ידני דרך `X-TryIt-Auth`, כך שניתן להסיר את משתני OAuth כאשר מעדיפים להדביק טוקנים זמניים.

ראו [`docs/devportal/security-hardening.md`](docs/devportal/security-hardening.md) למודל האיומים המלא ולשערי הבדיקות.

### כותרות אבטחה

`docusaurus.config.js` מפיק כותרות אבטחה דטרמיניסטיות — CSP, Trusted Types, Permissions-Policy, ו‑Referrer-Policy — כך שמארחים סטטיים ירשו את אותם guard rails כמו שרת הפיתוח. ברירות המחדל מאפשרות סקריפטים רק ממקור הפורטל ומגבילות `connect-src` לתחום האנליטיקה שהוגדר ול‑Try it proxy. בניות production דוחות endpoints לא‑HTTPS. לתצוגות מקומיות מול יעדי `http://`, הגדירו `DOCS_SECURITY_ALLOW_INSECURE=1` כדי לאשר במפורש את ההנמכה.

### תהליך לוקליזציה

הפורטל משתמש באנגלית כשפת מקור ותומך ביפנית, עברית, ספרדית, פורטוגזית, צרפתית, רוסית, ערבית ואורדו. כאשר מסמכים חדשים נכנסים, הריצו:

```bash
npm run sync-i18n
```

הסקריפט מחקה את ההתנהגות של `scripts/sync_docs_i18n.py` ומייצר סטאבים תחת `i18n/<lang>/docusaurus-plugin-content-docs/current/...`. העורכים מחליפים את הטקסט הזמני ומעדכנים את המטא‑דאטה ב‑front matter (`status`, `translation_last_reviewed` וכו׳).

כברירת מחדל, בניות production כוללות רק את הלוקאלים המופיעים ב‑`docs/i18n/published_locales.json` (נכון לעכשיו: אנגלית בלבד), כך שסטאבים לא נשלחים. הגדירו `DOCS_I18N_INCLUDE_STUBS=1` כאשר צריך להציג לוקאלים בתהליך תרגום מקומי.

### מפת תוכן (יעדי MVP)

| תחום | מצב | הערות |
|---------|--------|-------|
| Norito Quickstart (`docs/norito/quickstart.md`) | 🟢 Published | מסלול מלא של Docker + Kotodama + CLI לשליחת payload של Norito. |
| Norito Ledger Walkthrough (`docs/norito/ledger-walkthrough.md`) | 🟢 Published | תהליך CLI של register → mint → transfer עם אימות טרנזקציה/סטטוס וקישורי התאמה ל‑SDK. |
| מתכוני SDK (`docs/sdk/recipes/`) | 🟡 Rolling out | מתכונים עבור Rust/Python/JS/Swift/Java פורסמו; שאר ה‑SDKs צריכים ליישר קו. |
| רפרנס API | 🟢 Automated | `yarn sync-openapi` מפרסם את `static/openapi/torii.json`; DevRel מאשר בכל ריליס. |
| Streaming roadmap | 🟢 Stubbed | ראו `docs/norito-streaming-roadmap.md` לשילוב עם הבקלוג. |
| SoraFS multi-source scheduling (`docs/sorafs/provider-advert-multisource.md`) | 🟢 Published | מסכם TLV ליכולות range, אימות governance, fixtures של CLI והפניות טלמטריה. |

> 📌 נקודת בקרה 2026-03-05: לאחר שה‑quickstart ולפחות מתכון אחד פורסמו, הסירו את הטבלה וקשרו למדריך התורמים של הפורטל.

## CI והפצה

- `ci/check_docs_portal.sh` מריץ תחילה `node scripts/check-sdk-recipes.mjs` כדי לוודא שמתכוני ה‑SDK תואמים לקבצי המקור הקנוניים, ואז מפעיל `npm ci|install`, `npm run build`, ומוודא שסעיפים מרכזיים (דף הבית, SoraFS, Norito, publishing checklist) קיימים ב‑HTML שנוצר.
- `ci/check_openapi_spec.sh` מייצר מחדש את spec של Torii באמצעות `cargo xtask openapi`, משווה מול `static/openapi/torii.json` ו‑`versions/current/torii.json`, ומאמת את מניפסט ה‑checksum כדי לגרום לכישלון PR אם ה‑spec או ההאש אינם עדכניים.
- `.github/workflows/check-docs.yml` מריץ בניית פורטל לכל PR כדי ללכוד רגרסיות תוכן.
- `.github/workflows/docs-portal-preview.yml` בונה את האתר ל‑PRים, כותב `build/checksums.sha256`, מאמת עם `sha256sum -c`, אורז את הפלט כ‑`artifacts/preview-site.tar.gz`, מייצר descriptor באמצעות `scripts/generate-preview-descriptor.mjs`, ומעלה גם את האתר הסטטי וגם את מטא‑הנתונים כדי שסוקרים יוכלו לאמת את ה‑snapshot המדויק בלי להריץ את הבילד מחדש. הוורקפלואו גם מפרסם תגובת PR עם סיכום ה‑digests של המניפסט/הארכיון (וגם bundle של SoraFS כשזמין) כדי שסוקרים יקבלו את תוצאת האימות בלי לפתוח את לוגי Actions.
- `.github/workflows/docs-portal-deploy.yml` מפרסם את הבילד הסטטי ל‑GitHub Pages בעת push ל‑`main`/`master`, ומספק תצוגה ב‑URL של סביבת `github-pages`. הוורקפלואו מריץ בדיקת smoke לדף הנחיתה לפני ההפצה.
- `scripts/sorafs-package-preview.sh` ממיר את אתר הפריוויו לחבילת SoraFS דטרמיניסטית (CAR, plan, manifest), וכאשר מספקים אישורים באמצעות קונפיג JSON (ראו `docs/examples/sorafs_preview_publish.json`), יכול לשלוח את המניפסט ל‑Torii staging.
- `scripts/preview_verify.sh` מבצע את אימות checksum + descriptor שבעבר בוצע ידנית על‑ידי הסוקרים. הפנו אותו לתיקיית `build/` מחולצת (ולנתיבי descriptor/archive אופציונליים) כדי לאמת שה‑artefacts לא שונו.
- `scripts/sorafs-pin-release.sh` יחד עם `.github/workflows/docs-portal-sorafs-pin.yml` מאוטמים את צינור SoraFS ל‑production: build/test, יצירת CAR + manifest, חתימות Sigstore, אימות, קישור alias אופציונלי והעלאת artefacts לביקורת governance. הפעילו את הוורקפלואו באמצעות `workflow_dispatch` בעת קידום ריליס.
- `cargo xtask soradns-verify-gar --gar <path> --name <fqdn> [...]` מאמת Gateway Authorization Records לפני חתימה או משלוח ל‑ops. העוזר מאשר שה‑hosts הקנוניים/pretty, metadata של המניפסט ותוויות הטלמטריה תואמים למדיניות הדטרמיניסטית, ויכול להפיק סיכום JSON לעדות DG-3 באמצעות `--json-out`.
- בעת הגשה, עוזר ה‑pin מפעיל גם את `scripts/generate-dns-cutover-plan.mjs` כדי ליצור `artifacts/sorafs/portal.dns-cutover.json`. הגדירו `DNS_CHANGE_TICKET`, `DNS_CUTOVER_WINDOW`, `DNS_HOSTNAME`, `DNS_ZONE`, ו‑`DNS_OPS_CONTACT` (או העבירו את דגלי `--dns-*`) כדי שה‑descriptor יכלול את המטא‑נתונים הנחוצים ל‑Ops עבור מעבר DNS. כאשר נדרשים hooks לביטול cache ול‑rollback, הוסיפו `DNS_CACHE_PURGE_ENDPOINT`, `DNS_CACHE_PURGE_AUTH_ENV`, ו‑`DNS_PREVIOUS_PLAN` (או דגלי `--cache-purge-*` / `--previous-dns-plan`) כך שה‑descriptor יתעד את קריאת ה‑purge ואת ה‑descriptor הקודם לשחזורים. ה‑descriptor גם מתעד את `Sora-Route-Binding` המצורף (host, CID, נתיבי header/binding, פקודות אימות) כדי שביקורות קידום GAR ותוכניות fallback יתייחסו לכותרות המדויקות שמוגשות בקצה.
- אותו workflow יכול להפיק שלד zonefile/קטע resolver של SNS דרך `scripts/sns_zonefile_skeleton.py`. ספקו metadata של IPv4/IPv6/CNAME/SPKI/TXT (דרך משתני סביבה כגון `DNS_ZONEFILE_IPV4` או דגלי CLI כמו `--dns-zonefile-ipv4`), העבירו את digest ה‑GAR ב‑`DNS_GAR_DIGEST`, והעוזר יכתוב אוטומטית `artifacts/sns/zonefiles/<zone>/<hostname>.json` (וגם resolver snippet) כדי שעדי SN-7 יישמרו לצד ה‑cutover descriptor.
- ה‑descriptor של DNS מטמיע כעת מקטע `gateway_binding` שמפנה ל‑artefacts הנ״ל (נתיבים, CID תוכן, סטטוס הוכחה ותבנית כותרות מילולית) כך שאישורי שינוי DG-3 כוללים את חבילת `Sora-Name/Sora-Proof/CSP/HSTS` המדויקת הצפויה בשער.

כל הרצת `npm run build` מפעילה את ה‑hook של `postbuild` שמייצר `build/checksums.sha256`. אחרי בנייה (או הורדת artefacts מ‑CI), הריצו `./docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build` כדי לאמת את המניפסט. הוסיפו `--descriptor <path>`/`--archive <path>` כאשר מאמתים את חבילת המטא‑דאטה מ‑CI כדי שהסקריפט יצליב את ה‑digests ואת שמות הקבצים.
- לשיתוף פריוויו מקומי בצורה בטוחה, הריצו `npm run serve`. הפקודה עוטפת את `scripts/serve-verified-preview.mjs`, שמריצה את `preview_verify.sh` לפני הפעלת `docusaurus serve` ומבטלת את ההרצה אם בדיקת המניפסט נכשלת או חסרה, כך שפריוויו נשאר מגודר ב‑checksums גם מחוץ ל‑CI. עדיין ניתן להשתמש ב‑`npm run serve:verified` להרצות מפורשות.

### כתובות פריוויו & הערות שחרור

- פריוויו ציבורי: `https://docs.iroha.tech/`
- GitHub חושפת את הבילד גם דרך סביבת **github-pages** לכל פריסה.
- PRים שנוגעים בתוכן הפורטל כוללים artefacts של Actions (`docs-portal-preview`, `docs-portal-preview-metadata`) עם האתר הבנוי, מניפסט checksum, ארכיון דחוס ו‑descriptor; סוקרים יכולים להוריד, לפתוח את `index.html` מקומית ולאמת checksums לפני שיתוף הפריוויו. ה‑workflow מוסיף תגובת סיכום (hashes של manifest/archive יחד עם סטטוס SoraFS) לכל PR כדי לתת אינדיקציה מהירה שהאימות עבר.
- לאחר הורדת חבילת פריוויו, השתמשו ב‑`./docs/portal/scripts/preview_verify.sh --build-dir <extracted build> --descriptor <descriptor> --archive <archive>` כדי לוודא שה‑artefacts תואמים ל‑CI לפני שיתוף חיצוני.
- כאשר מכינים הערות שחרור או עדכוני סטטוס, ציינו את כתובת הפריוויו כדי שסוקרים חיצוניים יוכלו לעיין בסנפשוט האחרון בלי לשכפל את הריפו.
- תיאום גלי הפריוויו נעשה דרך `docs/portal/docs/devportal/preview-invite-flow.md` ובשילוב עם `docs/portal/docs/devportal/reviewer-onboarding.md`, כך שכל הזמנה, יצוא טלמטריה ותהליך offboarding ישתמשו באותה שרשרת ראיות.
