---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/public-preview-invite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4f4183db0fcf98f0c2f8d88ef725c981278ebfcec335ac67ecaefaad766eca85
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# פלייבוק הזמנות לפריוויו ציבורי

## יעדי התוכנית

פלייבוק זה מסביר כיצד להכריז ולהפעיל את הפריוויו הציבורי לאחר שזרימת onboarding של הסוקרים פעילה.
הוא שומר על כנות מפת הדרכים DOCS-SORA בכך שהוא מבטיח שכל הזמנה נשלחת עם ארטיפקטים ניתנים לאימות,
הנחיות אבטחה ומסלול משוב ברור.

- **קהל יעד:** רשימה מסוננת של חברי קהילה, שותפים ומיינטיינרים שחתמו על מדיניות acceptable-use של הפריוויו.
- **תקרות:** גודל גל ברירת מחדל <= 25 סוקרים, חלון גישה של 14 ימים, תגובה לאירועים בתוך 24h.

## צ'קליסט gate לפני השקה

השלימו את המשימות הבאות לפני שליחת הזמנה:

1. ארטיפקטים אחרונים של preview הועלו ל-CI (`docs-portal-preview`,
   checksum manifest, descriptor, SoraFS bundle).
2. `npm run --prefix docs/portal serve` (checksum gate) נבדק באותו tag.
3. טיקטים של onboarding לסוקרים אושרו וקושרו לגל ההזמנות.
4. מסמכי security, observability ו-incident אומתו
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. טופס feedback או תבנית issue הוכנו (כולל שדות severity, צעדי שחזור, צילומי מסך ומידע על הסביבה).
6. נוסח ההכרזה נבדק על ידי Docs/DevRel + Governance.

## חבילת הזמנה

כל הזמנה חייבת לכלול:

1. **ארטיפקטים מאומתים** — ספקו קישורים ל-SoraFS manifest/plan או ל-GitHub artefact,
   יחד עם checksum manifest ו-descriptor. ציינו במפורש את פקודת האימות כדי שהסוקרים
   יוכלו להריץ אותה לפני העלאת האתר.
2. **הוראות serve** — כללו את פקודת הפריוויו עם checksum gate:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **תזכורות אבטחה** — ציינו שהטוקנים פוקעים אוטומטית, שאין לשתף קישורים,
   ושיש לדווח על אירועים מיד.
4. **ערוץ משוב** — קשרו לטופס/תבנית והבהירו את ציפיות זמני התגובה.
5. **תאריכי תוכנית** — ספקו תאריכי התחלה/סיום, office hours או syncs, וחלון refresh הבא.

הדוא"ל לדוגמה ב-
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
מכסה את הדרישות הללו. עדכנו את ה-placeholders (תאריכים, URLs, אנשי קשר)
לפני השליחה.

## חשיפת ה-host של הפריוויו

קדמו את ה-host של הפריוויו רק לאחר השלמת onboarding ואישור טיקט שינוי. ראו את
[מדריך חשיפת host של preview](./preview-host-exposure.md) עבור שלבי build/publish/verify מקצה לקצה
בשימוש בסעיף זה.

1. **Build ואריזה:** הטביעו את ה-release tag והפיקו ארטיפקטים דטרמיניסטיים.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   סקריפט ה-pin כותב `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   ו-`portal.dns-cutover.json` תחת `artifacts/sorafs/`. צרפו את הקבצים לגל ההזמנות כדי שכל סוקר
   יוכל לאמת את אותם ביטים.

2. **פרסום alias של preview:** הריצו שוב את הפקודה בלי `--skip-submit`
   (ציינו `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` והוכחת alias שהונפקה על ידי governance).
   הסקריפט יקשר את ה-manifest ל-`docs-preview.sora` ויוציא
   `portal.manifest.submit.summary.json` וגם `portal.pin.report.json` לחבילת ה-evidence.

3. **בדיקת הדפלוי:** ודאו שה-alias נפתר ושה-checksum תואם ל-tag לפני שליחת ההזמנות.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   החזיקו `npm run serve` (`scripts/serve-verified-preview.mjs`) בהיכון כ-fallback כדי שסוקרים
   יוכלו להפעיל עותק מקומי אם preview edge מקרטע.

## ציר זמן תקשורתי

| יום | פעולה | Owner |
| --- | --- | --- |
| D-3 | סיום נוסח ההזמנה, רענון ארטיפקטים, dry-run לאימות | Docs/DevRel |
| D-2 | אישור governance + טיקט שינוי | Docs/DevRel + Governance |
| D-1 | שליחת הזמנות לפי התבנית, עדכון tracker עם רשימת נמענים | Docs/DevRel |
| D | שיחת kickoff / office hours, ניטור dashboards של טלמטריה | Docs/DevRel + On-call |
| D+7 | digest משוב באמצע הגל, triage ל-issues חוסמות | Docs/DevRel |
| D+14 | סגירת הגל, ביטול גישה זמנית, פרסום סיכום ב-`status.md` | Docs/DevRel |

## מעקב גישה וטלמטריה

1. רשמו כל נמען, timestamp להזמנה ותאריך ביטול באמצעות preview feedback logger
   (ראו [`preview-feedback-log`](./preview-feedback-log)) כדי שכל גל ישתף את אותו נתיב הוכחות:

   ```bash
   # הוספת אירוע הזמנה חדש ל-artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   האירועים הנתמכים הם `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, ו-`access-revoked`. הלוג נמצא כברירת מחדל ב-
   `artifacts/docs_portal_preview/feedback_log.json`; צרפו אותו לטיקט של גל ההזמנות יחד עם
   טפסי ההסכמה. השתמשו ב-summary helper כדי להפיק roll-up שניתן לביקורת לפני הערת הסגירה:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   Summary JSON מונה הזמנות לפי גל, נמענים פתוחים, מוני feedback, וטיימסטמפ של האירוע האחרון.
   ה-helper נשען על
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   כך שאותו workflow יכול לרוץ מקומית או ב-CI. השתמשו בתבנית digest שב-
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   בעת פרסום recap הגל.
2. תייגו את dashboards הטלמטריה עם ה-`DOCS_RELEASE_TAG` ששימש לגל כדי שניתן יהיה לקשר
   קפיצות ל-cohorts של הזמנות.
3. הריצו `npm run probe:portal -- --expect-release=<tag>` אחרי ה-deploy כדי לוודא שהסביבה
   מפרסמת את release metadata הנכונים.
4. תעדו כל אירוע בתבנית runbook וקשרו אותו לקוהורטה.

## Feedback וסגירה

1. איספו feedback במסמך משותף או בלוח issues. סמנו פריטים ב-`docs-preview/<wave>` כדי שבעלי
   roadmap יוכלו לאתר אותם בקלות.
2. השתמשו בפלט summary של preview logger כדי למלא את דוח הגל, ולאחר מכן סַכמו את הקוהורטה ב-
   `status.md` (משתתפים, ממצאים מרכזיים, תיקונים מתוכננים) ועדכנו `roadmap.md` אם milestone
   DOCS-SORA השתנה.
3. פעלו לפי שלבי offboarding מתוך
   [`reviewer-onboarding`](./reviewer-onboarding.md): בטלו גישה, ארכבו בקשות והודו למשתתפים.
4. הכינו את הגל הבא על ידי רענון ארטיפקטים, הרצת checksum gates מחדש ועדכון תבנית ההזמנה
   עם תאריכים חדשים.

יישום עקבי של הפלייבוק הזה שומר על תוכנית הפריוויו auditable ומעניק ל-Docs/DevRel דרך
חוזרת להרחיב הזמנות ככל שהפורטל מתקרב ל-GA.
