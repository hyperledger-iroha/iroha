---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/preview-host-exposure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 82a2a94c8c958c5ee7816a8d7f2531cc5117a9192c87cec940074b22a0c68980
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# מדריך חשיפת מארח תצוגה מקדימה

מפת הדרכים DOCS-SORA דורשת שכל תצוגה מקדימה ציבורית תתבסס על אותו bundle המאומת ב-checksum שהסוקרים מריצים מקומית. השתמשו ב-runbook זה לאחר השלמת onboarding של סוקרים (וכרטיס אישור הזמנות) כדי להעלות את מארח הבטא לאוויר.

## דרישות מקדימות

- גל onboarding של סוקרים אושר ותועד במעקב התצוגות המקדימות.
- הבילד האחרון של הפורטל נמצא תחת `docs/portal/build/` וה-checksum אומת (`build/checksums.sha256`).
- אישורי SoraFS preview (כתובת Torii, authority, מפתח פרטי, epoch שנשלח) נשמרים במשתני סביבה או בקובץ JSON כגון [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- כרטיס שינוי DNS פתוח עם שם המארח הרצוי (`docs-preview.sora.link`, `docs.iroha.tech`, וכו') ופרטי קשר on-call.

## שלב 1 - בנייה ואימות ה-bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

סקריפט האימות מסרב להמשיך כאשר מניפסט ה-checksum חסר או עבר שינוי, וכך שומר כל ארטיפקט תצוגה מקדימה מבוקר.

## שלב 2 - אריזת ארטיפקטים של SoraFS

המירו את האתר הסטטי לזוג CAR/manifest דטרמיניסטי. ברירת המחדל של `ARTIFACT_DIR` היא `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

צרפו `portal.car`, `portal.manifest.*`, את ה-descriptor ואת מניפסט ה-checksum לכרטיס גל התצוגה המקדימה.

## שלב 3 - פרסום alias של preview

הריצו מחדש את helper ה-pin **ללא** `--skip-submit` כאשר אתם מוכנים לחשוף את המארח. ספקו את קובץ ה-JSON או flags מפורשים:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

הפקודה כותבת את `portal.pin.report.json`, `portal.manifest.submit.summary.json`, ו-`portal.submit.response.json`, וחייבת להישלח עם bundle הראיות להזמנה.

## שלב 4 - יצירת תוכנית מעבר DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

שתפו את ה-JSON שנוצר עם Ops כך שמעבר ה-DNS יתייחס ל-digest המדויק של המניפסט. כאשר משתמשים ב-descriptor קודם כמקור rollback, הוסיפו `--previous-dns-plan path/to/previous.json`.

## שלב 5 - בדיקת המארח שהועלה

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

ה-probe מאשר את תגית ה-release, כותרות CSP ומטא-דאטה חתימה. חזרו על הפקודה משני אזורים (או צרפו פלט curl) כדי שהבודקים יראו שה-edge cache חם.

## Bundle ראיות

כללו את הארטיפקטים הבאים בכרטיס גל התצוגה המקדימה והתייחסו אליהם באימייל ההזמנה:

| ארטיפקט | מטרה |
|---------|------|
| `build/checksums.sha256` | מוכיח שה-bundle תואם את הבילד של CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | מטען SoraFS קנוני + מניפסט. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | מציג שהגשת המניפסט וקישור ה-alias הצליחו. |
| `artifacts/sorafs/portal.dns-cutover.json` | מטא-דאטה DNS (כרטיס, חלון, קשרים), סיכום קידום נתיב (`Sora-Route-Binding`), מצביע `route_plan` (תוכנית JSON + תבניות header), מידע purge של cache והוראות rollback ל-Ops. |
| `artifacts/sorafs/preview-descriptor.json` | descriptor חתום שקושר בין ה-archive ל-checksum. |
| פלט `probe` | מאשר שהמארח החי מפרסם את תגית ה-release המצופה. |
