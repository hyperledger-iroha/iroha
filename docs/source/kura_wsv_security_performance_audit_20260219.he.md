<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Kura / WSV ביקורת אבטחה וביצועים (2026-02-19)

## היקף

ביקורת זו כיסה:

- מסלולי התמדה ותקציב של Kura: `crates/iroha_core/src/kura.rs`
- ייצור WSV/נתיבי מחויבות/שאילתה: `crates/iroha_core/src/state.rs`
- IVM משטחי מארח מדומים WSV (היקף בדיקה/פיתוח): `crates/ivm/src/mock_wsv.rs`

מחוץ לתחום: ארגזים לא קשורים ושידורים חוזרים של מערכת ביצועים חוזרת.

## סיכום סיכונים

- קריטי: 0
- גבוה: 4
- בינוני: 6
- נמוך: 2

## ממצאים (מסודרים לפי חומרה)

### גבוה

1. **כותב Kura נבהל מכשלים ב-I/O (סיכון זמינות צמתים)**
- רכיב: קורה
- סוג: אבטחה (DoS), אמינות
- פירוט: לולאת כותב נכנסת לפאניקה על שגיאות append/index/fsync במקום החזרת שגיאות הניתנות לשחזור, כך שתקלות דיסק חולפות יכולות לסיים את תהליך הצומת.
- עדות:
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- השפעה: עומס מרחוק + לחץ דיסק מקומי יכולים לגרום ללולאות קריסה/הפעלה מחדש.2. **פינוי Kura עושה שכתוב נתונים/אינדקס מלאים תחת `block_store` mutex**
- רכיב: קורה
- סוג: ביצועים, זמינות
- פירוט: `evict_block_bodies` משכתב את `blocks.data` ו-`blocks.index` באמצעות קבצים זמניים תוך החזקת מנעול `block_store`.
- עדות:
  - רכישת מנעול: `crates/iroha_core/src/kura.rs:834`
  - לולאות שכתוב מלא: `crates/iroha_core/src/kura.rs:921`, `crates/iroha_core/src/kura.rs:942`
  - החלפה/סנכרון אטומי: `crates/iroha_core/src/kura.rs:956`, `crates/iroha_core/src/kura.rs:960`
- השפעה: אירועי פינוי יכולים לעכב כתיבה/קריאה לתקופות ממושכות על היסטוריות גדולות.

3. **התחייבות המדינה תופסת `view_lock` גס על פני עבודת התחייבות כבדה**
- רכיב: ייצור WSV
- סוג: ביצועים, זמינות
- פירוט: block commit מחזיק בבלעדיות של `view_lock` תוך ביצוע עסקאות, חסימות גיבוב ומצב עולמי, מה שיוצר הרעבה של קוראים תחת בלוקים כבדים.
- עדות:
  - נעילת החזקת מתחילה: `crates/iroha_core/src/state.rs:17456`
  - עבודה בתוך מנעול: `crates/iroha_core/src/state.rs:17466`, `crates/iroha_core/src/state.rs:17476`, `crates/iroha_core/src/state.rs:17483`
- השפעה: התחייבויות כבדות מתמשכות עלולות לפגוע בהיענות לשאילתות/קונצנזוס.4. **IVM כינויי אדמין של JSON מאפשרים מוטציות מורשות ללא בדיקות מתקשרים (מארח בדיקה/מפתח)**
- רכיב: IVM מארח מדומה WSV
- סוג: אבטחה (הסלמה של הרשאות בסביבות בדיקה/פיתוח)
- פירוט: מטפלי כינוי JSON מנתבים ישירות לשיטות מוטציה של תפקיד/הרשאה/עמית שאינן דורשות אסימוני הרשאה בהיקף מתקשר.
- עדות:
  - כינויים של מנהל מערכת: `crates/ivm/src/mock_wsv.rs:4274`, `crates/ivm/src/mock_wsv.rs:4371`, `crates/ivm/src/mock_wsv.rs:4448`
  - מוטטורים לא מאושרים: `crates/ivm/src/mock_wsv.rs:1035`, `crates/ivm/src/mock_wsv.rs:1055`, `crates/ivm/src/mock_wsv.rs:855`
  - הערת היקף במסמכי הקובץ (כוונת בדיקה/פיתוח): `crates/ivm/src/mock_wsv.rs:295`
- השפעה: חוזי בדיקה/כלים יכולים להעלות את עצמם ולבטל את הנחות האבטחה ברתמות האינטגרציה.

### בינוני

5. **בדיקות תקציב של Kura מקודדות מחדש בלוקים ממתינים בכל תור (O(n) לכל כתיבה)**
- רכיב: קורה
- סוג: ביצועים
- פירוט: כל תור מחשב מחדש בתים של תור ממתינים על ידי איטרציה של בלוקים ממתינים והסדרה של כל אחד מהם באמצעות נתיב גודל חוט קנוני.
- עדות:
  - סריקת תור: `crates/iroha_core/src/kura.rs:2509`
  - נתיב קידוד לכל בלוק: `crates/iroha_core/src/kura.rs:2194`, `crates/iroha_core/src/kura.rs:2525`
  - נקרא בבדיקת תקציב על תור: `crates/iroha_core/src/kura.rs:2580`, `crates/iroha_core/src/kura.rs:2050`
- השפעה: כתוב ירידה בתפוקה תחת פיגור.6. **בדיקות התקציב של Kura מבצעות קריאות חוזרות ונשנות של מטא נתונים בחנות בלוק בכל תור**
- רכיב: קורה
- סוג: ביצועים
- פירוט: כל צ'ק קורא ספירת אינדקס עמיד ואורכי קבצים תוך נעילת `block_store`.
- עדות:
  - `crates/iroha_core/src/kura.rs:2538`
  - `crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- השפעה: תקורה של קלט/פלט/נעילה ניתנת להימנעות על נתיב תור חם.

7. **פינוי Kura מופעל בתוך שורה מנתיב תקציב התור**
- רכיב: קורה
- סוג: ביצועים, זמינות
- פירוט: נתיב התור יכול לקרוא באופן סינכרוני לפינוי לפני קבלת בלוקים חדשים.
- עדות:
  - שרשרת שיחות תור: `crates/iroha_core/src/kura.rs:2050`
  - שיחת פינוי מוטבעת: `crates/iroha_core/src/kura.rs:2603`
- השפעה: עליות זנב-השהייה על הטמעת עסקה/חסימה כאשר קרוב לתקציב.

8. **`State::view` עשוי לחזור מבלי לרכוש נעילה גסה תחת מחלוקת**
- רכיב: ייצור WSV
- סוג: פשרה בין עקביות/ביצועים
- פירוט: בטענה לנעילת כתיבה, `try_read` מחזיר תצוגה ללא הגנה גסה לפי עיצוב.
- עדות:
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
- השפעה: חיוניות משופרת, אך המתקשרים חייבים לסבול אטומיות חלשה יותר בין רכיבים בוויכוח.9. **`apply_without_execution` משתמש ב-`expect` קשה בהתקדמות סמן DA**
- רכיב: ייצור WSV
- סוג: אבטחה (DoS באמצעות panic-on-invariant-break), אמינות
- פירוט: חסימה מחויבת להחיל פאניקה בנתיב אם אינוריאנטים של קידום סמן DA נכשלים.
- עדות:
  - `crates/iroha_core/src/state.rs:17621`
  - `crates/iroha_core/src/state.rs:17625`
- השפעה: באגים סמויים של אימות/אינדקס יכולים להפוך לכשלים הורגים של צמתים.

10. **IVM IVM TLV publish syscall חסרה גודל מעטפה מפורש הקשור לפני ההקצאה (מארח בדיקה/פיתוח)**
- רכיב: IVM מארח מדומה WSV
- סוג: אבטחה (זיכרון DoS), ביצועים
- פירוט: קורא את אורך הכותרת ואז מקצה/מעתיק מטען TLV מלא ללא מכסה ברמת המארח בנתיב זה.
- עדות:
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- השפעה: עומסי בדיקות זדוניים יכולים לכפות הקצאות גדולות.

### נמוך

11. **ערוץ ההודעות של Kura אינו מוגבל (`std::sync::mpsc::channel`)**
- רכיב: קורה
- סוג: היגיינת ביצועים/זיכרון
- פירוט: ערוץ הודעות יכול לצבור אירועי התעוררות מיותרים במהלך לחץ מתמשך של המפיק.
- עדות:
  - `crates/iroha_core/src/kura.rs:552`
- השפעה: הסיכון לצמיחת זיכרון נמוך לפי גודל אירוע אך ניתן להימנע.12. **תור הצד של הצינור אינו מוגבל בזיכרון עד לריקון הסופר**
- רכיב: קורה
- סוג: היגיינת ביצועים/זיכרון
- פרט: לתור `push_back` אין מכסה/לחץ גב מפורש.
- עדות:
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- השפעה: גידול פוטנציאלי בזיכרון במהלך עיכובים ממושכים בכתיבה.

## כיסוי מבחן קיים ופערים

### קורה

- כיסוי קיים:
  - התנהגות תקציב אחסון: `store_block_rejects_when_budget_exceeded`, `store_block_rejects_when_pending_blocks_exceed_budget`, `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`, `crates/iroha_core/src/kura.rs:6949`, `crates/iroha_core/src/kura.rs:6984`)
  - תקינות פינוי והידרדרציה: `evict_block_bodies_does_not_truncate_unpersisted`, `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`, `crates/iroha_core/src/kura.rs:8126`)
- פערים:
  - אין כיסוי הזרקת תקלות לטיפול בכשלים של append/index/fsync ללא פאניקה
  - אין מבחן רגרסיה של ביצועים עבור תורים גדולים ממתינים ועלות בדיקת תקציב לתור
  - אין מבחן חביון פינוי בעל היסטוריה ארוכה תחת טענה של נעילה

### הפקת WSV

- כיסוי קיים:
  - התנהגות ניצול מחלוקת: `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - בטיחות בהזמנה של נעילה סביב קצה אחורי מדורג: `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- פערים:
  - אין מבחן מחלוקת כמותי הקובע את זמן החזקה מרבי מקובל תחת התחייבויות עולמיות כבדות
  - אין מבחן רגרסיה לטיפול נטול פאניקה אם אינוריאנטי התקדמות סמן DA נשברים באופן בלתי צפוי

### IVM מארח מדומה WSV- כיסוי קיים:
  - הרשאה סמנטיקה של מנתח JSON וניתוח עמיתים (`crates/ivm/src/mock_wsv.rs:5234`, `crates/ivm/src/mock_wsv.rs:5332`)
  - syscall בדיקות עשן סביב פענוח TLV ופענוח JSON (`crates/ivm/src/mock_wsv.rs:5962`, `crates/ivm/src/mock_wsv.rs:6078`)
- פערים:
  - אין בדיקות דחיית admin-כינוי לא מורשה
  - אין בדיקות דחיית מעטפת TLV בגודל גדול ב-`INPUT_PUBLISH_TLV`
  - אין בדיקות בנצ'מרק/מעקה בטיחות סביב עלות שיבוט/שחזור

## תוכנית תיקון עם עדיפות

### שלב 1 (הקשחה בעלת השפעה גבוהה)

1. החלף את ענפי Kura writer `panic!` עם התפשטות שגיאה ניתנת לשחזור + איתות בריאות פגועה.
- קבצי יעד: `crates/iroha_core/src/kura.rs`
- קבלה:
  - כשלים שהוזרקו append/index/fsync אינם נכנסים לפאניקה
  - שגיאות מופיעות באמצעות טלמטריה/רישום והכותב נשאר ניתן לשליטה

2. הוסף בדיקות מעטפה מוגבלות עבור IVM פרסום מדומה-מארח TLV ונתיבי מעטפת JSON.
- קבצי יעד: `crates/ivm/src/mock_wsv.rs`
- קבלה:
  - מטענים גדולים מדי נדחים לפני עיבוד כבד בהקצאות
  - בדיקות חדשות מכסות גם מקרים גדולים של TLV וגם JSON

3. לאכוף בדיקות הרשאות מפורשות של מתקשר עבור כינויים של מנהל מערכת של JSON (או כינויים של שער מאחורי דגלים קפדניים של תכונות בדיקה בלבד ומסמך ברור).
- קבצי יעד: `crates/ivm/src/mock_wsv.rs`
- קבלה:
  - מתקשר לא מורשה אינו יכול לשנות תפקיד/הרשאה/מצב עמית באמצעות כינויים

### שלב 2 (ביצועי נתיב חם)4. הפוך את החשבונאות התקציבית של Kura למצטברת.
- החלף חישוב מלא של תור ממתין לכל תור עם מונים מתוחזקים המעודכנים על עמידה/התמשכות/ירידה.
- קבלה:
  - עלות תור ליד O(1) לחישוב בתים בהמתנה
  - רף רגרסיה מראה זמן אחזור יציב ככל שהעומק הממתין גדל

5. צמצם את זמן החזקת נעילת הפינוי.
- אפשרויות: דחיסה מפולחת, עותק חתוך עם גבולות שחרור נעילה, או מצב תחזוקה ברקע עם חסימת חזית מוגבלת.
- קבלה:
  - השהיית פינוי בהיסטוריה גדולה פוחתת ופעולות החזית נשארות מגיבות

6. קצר קטע קריטי גס `view_lock` היכן שניתן.
- הערכת שלבי התחייבות לפיצול או צילום דלתות מבוימות כדי למזער חלונות החזקה בלעדיים.
- קבלה:
  - מדדי מחלוקת מדגימים זמן החזקה מופחת של 99p תחת התחייבויות חסימות כבדות

### שלב 3 (מעקות בטיחות תפעוליים)

7. הכנס איתות ערות מוגבל/מתלכד ללחץ אחורי/כובעים בתור סופר Kura ותור צד.
8. הרחב את לוחות המחוונים של טלמטריה עבור:
- `view_lock` הפצות המתנה/המתנה
- משך פינוי ובייטים שנגבו לריצה
- זמן תור לבדיקת תקציב

## תוספות מבחן מוצעות1. `kura_writer_io_failures_do_not_panic` (יחידה, הזרקת תקלה)
2. `kura_budget_check_scales_with_pending_depth` (רגרסיית ביצועים)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (שילוב/פרפ)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (רגרסיית טענה)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (חוסן)
6. `mock_wsv_admin_alias_requires_permissions` (רגרסיית אבטחה)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (שומר DoS)
8. `mock_wsv_checkpoint_restore_cost_regression` (השוואת ביצועים)

## הערות על היקף וביטחון

- ממצאים עבור `crates/iroha_core/src/kura.rs` ו-`crates/iroha_core/src/state.rs` הם ממצאי נתיב ייצור.
- הממצאים עבור `crates/ivm/src/mock_wsv.rs` הם במפורש בהיקף מארח בדיקה/פיתוח, לפי תיעוד ברמת הקובץ.
- לא נדרשים שינויים בגירסת ABI בביקורת זו עצמה.