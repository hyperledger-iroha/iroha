---
lang: he
direction: rtl
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f42888a06cb49f9fe53f424ef77c84e2fa3a305f558e202be0fbbd4b3b0ea1d7
source_last_modified: "2025-12-19T22:34:59+00:00"
translation_last_reviewed: 2026-01-01
---

# onboarding לסוקרי פריוויו

## סקירה כללית

DOCS-SORA עוקב אחר השקה מדורגת של פורטל המפתחים. בניות עם gate של checksum
(`npm run serve`) וזרימות Try it מוקשחות פותחות את האבן הבאה: onboarding של סוקרים מאומתים
לפני שהפריוויו הציבורי נפתח באופן רחב. מדריך זה מתאר כיצד לאסוף בקשות, לאמת זכאות,
להקצות גישה ולבצע offboarding בבטחה. ראו את
[preview invite flow](./preview-invite-flow.md) לתכנון קוהורטים, קצב הזמנות וייצואי טלמטריה;
השלבים למטה מתמקדים בפעולות שיש לבצע לאחר שסוקר נבחר.

- **בתחום:** סוקרים שזקוקים לגישה לפריוויו של המסמכים (`docs-preview.sora`,
  בניות GitHub Pages או חבילות SoraFS) לפני GA.
- **מחוץ לתחום:** מפעילי Torii או SoraFS (מכוסים בערכות onboarding משלהם) ופריסות פורטל ייצור
  (ראו [`devportal/deploy-guide`](./deploy-guide.md)).

## תפקידים ותנאים מוקדמים

| תפקיד | יעדים טיפוסיים | ארטיפקטים נדרשים | הערות |
| --- | --- | --- | --- |
| Core maintainer | לאמת מדריכים חדשים, להריץ smoke tests. | handle GitHub, איש קשר Matrix, CLA חתומה בתיק. | בדרך כלל כבר בצוות GitHub `docs-preview`; עדיין הגישו בקשה כדי שהגישה תהיה ניתנת לביקורת. |
| Partner reviewer | לאמת קטעי SDK או תוכן ממשל לפני שחרור ציבורי. | אימייל ארגוני, POC משפטי, תנאי preview חתומים. | חייבים לאשר דרישות טלמטריה + טיפול בנתונים. |
| Community volunteer | לספק פידבק שימושיות על מדריכים. | handle GitHub, איש קשר מועדף, אזור זמן, קבלת CoC. | שמרו על קוהורטים קטנים; תעדפו סוקרים שחתמו על הסכם תרומה. |

כל סוגי הסוקרים חייבים:

1. לאשר את מדיניות השימוש המותר בארטיפקטים של preview.
2. לקרוא את נספחי האבטחה/observability
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. להסכים להריץ `docs/portal/scripts/preview_verify.sh` לפני שמגישים כל snapshot מקומית.

## תהליך קליטה

1. לבקש מהמבקש למלא את הטופס
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   (או להעתיק/להדביק אותו ב-issue). יש לתעד לפחות: זהות, אופן קשר, handle GitHub, תאריכי ביקורת
   מתוכננים ואישור שהמסמכים בנושא אבטחה נקראו.
2. לתעד את הבקשה בטרקר `docs-preview` (GitHub issue או כרטיס ממשל) ולהקצות מאשר.
3. לוודא תנאים מוקדמים:
   - CLA / הסכם תורם בתיק (או הפניה לחוזה שותף).
   - אישור שימוש מותר שמור בבקשה.
   - הערכת סיכונים הושלמה (לדוגמה, סוקרי שותפים אושרו על ידי Legal).
4. המאשר מסמן אישור בבקשה ומקשר את ה-issue של המעקב לכל רשומת change-management
   (לדוגמה: `DOCS-SORA-Preview-####`).

## הקצאה וכלים

1. **שיתוף ארטיפקטים** — לספק את ה-descriptor + archive האחרון של preview מה-workflow של CI
   או מה-pin של SoraFS (ארטיפקט `docs-portal-preview`). להזכיר לסוקרים להריץ:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Serve עם enforcement של checksum** — להפנות את הסוקרים לפקודה עם gate של checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   זה עושה שימוש חוזר ב-`scripts/serve-verified-preview.mjs` כדי שלא יעלה build לא מאומת בטעות.

3. **מתן גישת GitHub (אופציונלי)** — אם הסוקרים צריכים ענפים לא מפורסמים, להוסיף אותם לצוות
   GitHub `docs-preview` למשך תקופת הביקורת ולתעד את שינוי החברות בבקשה.

4. **שיתוף ערוצי תמיכה** — לשתף איש קשר on-call (Matrix/Slack) ואת נוהל האירועים מתוך
   [`incident-runbooks`](./incident-runbooks.md).

5. **טלמטריה + פידבק** — להזכיר שהאנליטיקות האנונימיות נאספות (ראו [`observability`](./observability.md)).
   לספק טופס פידבק או תבנית issue שמופיעה בהזמנה, ולתעד את האירוע עם העוזר
   [`preview-feedback-log`](./preview-feedback-log) כדי שסיכום הגל יישאר עדכני.

## צ'קליסט לסוקר

לפני גישה ל-preview, הסוקרים חייבים להשלים:

1. לאמת את הארטיפקטים שהורדו (`preview_verify.sh`).
2. להפעיל את הפורטל דרך `npm run serve` (או `serve:verified`) כדי לוודא שה-guard של checksum פעיל.
3. לקרוא את הערות האבטחה וה-observability שקושרו לעיל.
4. לבדוק את קונסולת OAuth/Try it באמצעות device-code login (אם רלוונטי) ולהימנע ממחזור tokens של production.
5. לרשום ממצאים בטרקר המוסכם (issue, מסמך משותף או טופס) ולתייג אותם בתג השחרור של preview.

## אחריות המיינטיינרים ו-offboarding

| שלב | פעולות |
| --- | --- |
| Kickoff | לוודא שצ'קליסט intake מצורף לבקשה, לשתף ארטיפקטים + הוראות, להוסיף רשומת `invite-sent` דרך [`preview-feedback-log`](./preview-feedback-log), ולקבוע sync באמצע אם הביקורת נמשכת יותר משבוע. |
| Monitoring | לנטר טלמטריית preview (תנועה חריגה של Try it, כשלי probe) ולפעול לפי נוהל האירועים אם משהו נראה חשוד. לתעד אירועים `feedback-submitted`/`issue-opened` עם הגעת הממצאים כדי לשמור על מדדי גל מדויקים. |
| Offboarding | לבטל גישה זמנית ל-GitHub או SoraFS, לתעד `access-revoked`, לארכב את הבקשה (כולל סיכום פידבק + פעולות פתוחות), ולעדכן את רישום הסוקרים. לבקש מהסוקר למחוק builds מקומיים ולצרף את ה-digest שנוצר מ- [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

השתמשו באותו תהליך בעת רוטציה של סוקרים בין גלים. שמירת העקבות במאגר (issue + תבניות) מסייעת
ל-DOCS-SORA להישאר ניתנת לביקורת ומאפשרת ל-governance לוודא שגישת ה-preview
עמדה בבקרות המתועדות.

## תבניות הזמנה ומעקב

- להתחיל כל outreach עם הקובץ
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  הוא כולל את השפה המשפטית המינימלית, הוראות checksum ל-preview והציפייה שהסוקרים יאשרו
  את מדיניות השימוש המותר.
- בעת עריכת התבנית, להחליף את ה-placeholders עבור `<preview_tag>`, `<request_ticket>` וערוצי הקשר.
  לשמור עותק של ההודעה הסופית בכרטיס intake כדי שסוקרים, מאשרים ומבקרים יוכלו לעיין בנוסח המדויק שנשלח.
- לאחר שליחת ההזמנה, לעדכן את גיליון המעקב או ה-issue עם חותמת הזמן `invite_sent_at` ותאריך סיום צפוי
  כדי שדוח [preview invite flow](./preview-invite-flow.md) יוכל לקלוט את הקוהורט אוטומטית.
