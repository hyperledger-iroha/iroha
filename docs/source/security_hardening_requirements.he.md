<!-- Hebrew translation of docs/source/security_hardening_requirements.md -->

---
lang: he
direction: rtl
source: docs/source/security_hardening_requirements.md
status: complete
translator: manual
---

<div dir="rtl">

# דרישות לחיזוק ה-Runtime

מסמך זה מפרט דרישות הנדסיות לסגירת סיכונים שנותרו במודל האיום. לכל סעיף: תחום, דרישות פונקציונליות, תצפיתיות ושיקולי פריסה.

## Telemetry של Hash חברים (View)

**תחום:** צמתים של Sumeragi חייבים לזהות ולדווח על פערים בין רשימת הוולידטורים המקומית לבין זו שמפורסמת ע"י עמיתים.

**דרישות**
- לחשב hash דטרמיניסטי של רשימת הוולידטורים (לפי סדר) לכל `(height, view)` בזמן הגדרת פרמטרי קונצנזוס חדשים.
- כאשר עמית מודיע על פרמטרים/Hash שונים – להגדיל מונה Prometheus עם תוויות ולהדפיס לוג מבני.
- לפרסם מדדים המאפשרים זיהוי פערים מתמשכים (תוויות `peer_id`, `height`, `view`).
- knob קונפיג לטריגר התראה (ברירת מחדל: כל פער במשך 5 דקות).

**תצפיתיות**
- `sumeragi_membership_mismatch_total{peer,height,view}`.
- אופציונלי: `sumeragi_membership_mismatch_active`.
- `/v1/sumeragi/status` יכלול מצב mismatch, מוני drop, hashes של Highest/Locked QC ו-`pacemaker_backpressure_deferrals_total`.

**מצב:** הוטמעו המדדים `sumeragi_membership_view_hash`, ‏`sumeragi_membership_height`, ‏`sumeragi_membership_view`, ‏`sumeragi_membership_epoch` יחד עם האובייקט `/v1/sumeragi/status.membership`, כך שניתן לקבל את ה-hash הדטרמיניסטי של הרוסטר וההקשר `(height, view, epoch)` לכל צומת.

**פריסה**
- לא לשבור אקספורטרים קיימים.
- לאחר Restart – מונים מונוטוניים ממשיכים, gauge מאופס ל-0.

## הגבלת חיבורים לפני Auth (Torii)

**תחום:** Torii מחייבת הגנה בכניסה (REST/WS) לפני אימות אפליקטיבי באמצעות `preauth_*`.

**דרישות**
- לאכוף מגבלות חיבורים גלובליות ול-IP בהתאם ל-config.
- להטמיע rate-limit מסוג token-bucket לקבלות TLS/TCP ו-upgrade ל-WS.
- Backoff אקספוננציאלי לעבריינים, עם BAN זמני וקונפיג מוקסט.
- לאפשר allowlist של CIDR לעקיפת המגבלות.
- Fail-closed בעת כשל בהערכת המגבלות.

**תצפיתיות**
- `torii_pre_auth_reject_total{reason}` (`ip_cap`, `global_cap`, `rate`, `ban`, `error`).
- `torii_active_connections_total{scheme}`.
- WARN log כאשר BAN מופעל.

**פריסה**
- אם ניתן – Reload דינמי; אחרת לתעד צורך בריסט.
- לעדכן בדיקות קיימות ולהוסיף בדיקות דחייה.
- לשמור אפשרות כיבוי בסביבות בדיקה.

## צינור סניטציה למצורפים

**תחום:** כל המצורפים (מניפסטים, הוכחות, blobs) חייבים לעבור בדיקות פורמט.

**דרישות**
- לזהות סוג קובץ via magic bytes; לדחות פורמטים אסורים.
- לאכוף גבולות התפיחה לפני כתיבה לדיסק (קבצים דחוסים – גודל כולל ועומק קינון).
- להריץ דה-קומפרסיה/סניטציה בתהליך מבודד עם `torii.attachments_sanitize_timeout_ms`, מגבלות CPU/Memory (rlimits), ומצב ריצה `torii.attachments_sanitizer_mode` (ברירת מחדל `subprocess`).
- ביצוא — לבצע סניטציה מחדש למצורפים ללא `provenance` לפני החזרת הבייטים.
- לשמור מטא-דאטה: hash, סוג, תוצאת סניטציה.

**תצפיתיות**
- `torii_attachment_reject_total{reason}` (`type`, `expansion`, `sandbox`, `checksum`).
- היסטוגרמת זמן `torii_attachment_sanitize_ms`.
- INFO logs עם מזהה מצורף וסיבה.

**פריסה**
- קונפיג per-format (הפעלה/ביטול, גבולות התפיחה).
- fallback דטרמיניסטי – לשתף תוצאות או לדחות באופן אחיד.
- בדיקות אינטגרציה עם קבצי דוגמה.

</div>
