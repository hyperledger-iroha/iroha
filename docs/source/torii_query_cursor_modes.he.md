<!-- Hebrew translation of docs/source/torii_query_cursor_modes.md -->

---
lang: he
direction: rtl
source: docs/source/torii_query_cursor_modes.md
status: complete
translator: manual
---

<div dir="rtl">

## מצבי Cursor לשאילתות Torii (נתיב Snapshot)

עמוד זה מסביר כיצד שאילתות בצד השרת בוחרות התנהגות Cursor בעת הרצה על צילום מצב, וכיצד לקוחות יכולים לעקוף זאת לכל בקשה.

### סקירה

Iroha מריצה שאילתות לקריאה על צילום מצב (`StateView`) שנלכד מראש כדי לשמר דטרמיניזם. שאילתות איטרביליות פועלות בשני מצבים:

- `ephemeral`: מחזירה רק את האצווה הראשונה. השרת אינו שומר Cursor. הלקוח משיג עמודים נוספים על ידי שליחת בקשת Start חדשה עם פרמטרי עימוד מעודכנים.
- `stored`: מחזירה את האצווה הראשונה יחד עם Cursor בשרת. הלקוח יכול להמשיך את אותו צילום מצב באמצעות ה-Cursor.

בחירת המצב ניתנת להגדרה וניתנת לעקיפה בבקשה בודדת.

### תצורה

- `pipeline.query_default_cursor_mode`: מצב Cursor ברירת מחדל לשאילתות איטרביליות. ערכים: `ephemeral` (ברירת מחדל) או `stored`.
- `pipeline.query_stored_min_gas_units`: מספר יחידות הגז המינימלי עבור מצב `stored` (0 מבטל את הדרישה). כאשר הערך גדול מ-0, הלקוח חייב לספק `gas_units` מספקים בבקשה.

### עקיפות לכל בקשה

נקודת הקצה `/query` מקבלת פרמטרים (ב-query string) השמורים לשליטת המצב:

- `cursor_mode`: `ephemeral` | `stored`
- `gas_units`: מספר שלם; נדרש כאשר `pipeline.query_stored_min_gas_units > 0` ו-`cursor_mode=stored`. בהיעדר כמות מספקת, השרת ידחה את הבקשה עם שגיאת ולידציה.
- בקשות `Continue` מאחסנות את תקציב הגז ב-Norito באמצעות `ForwardCursor.gas_budget`, כך שהשרת יכול לאמת את ה-Cursor המאוחסן.

הערות:
- אם `cursor_mode` לא סופק, השרת משתמש בערך ברירת המחדל מ-`pipeline.query_default_cursor_mode`.
- במצב `ephemeral`, השרת מחזיר תמיד `cursor=null`. בקשות Continue נדחות.

### טלמטריה

כאשר הטלמטריה פעילה (`telemetry_enabled=true`):

- `torii_query_snapshot_requests_total{mode}`: ספירה מצטברת של שאילתות נתיב snapshot לפי מצב.
- `torii_query_snapshot_first_batch_ms{mode}`: היסטוגרמת לטנציה (ב-ms) לאצווה הראשונה, לפי מצב.
- `torii_query_snapshot_gas_consumed_units_total{mode}`: סכימת יחידות גז שדווחו על ידי לקוחות במצב `stored` (כאשר מוגדרת דרישת מינימום).
- טלמטריה ליבתית מוסיפה מדדים משלימים כגון `query_snapshot_lane_first_batch_ms{mode}` ו-`query_snapshot_lane_remaining_items{mode}` למפעילים שעוקבים אחרי הצינור ישירות.

### דטרמיניזם וסמנטיקת סנאפשוט

- כל השאילתות פועלות על `StateView` שנלכד מראש; שינויים חיים לאחר הלכידה אינם משפיעים על המשך עם Cursor מאוחסן.
- במצב `ephemeral` האצווה הראשונה מחומרת ונשלחת ללקוח. כדי להתקדם, הלקוח חייב לשלוח בקשת Start חדשה עם פרמטרי עימוד מעודכנים.

### דוגמאות (קונספטואליות)

1) ברירת מחדל `ephemeral`, ללא עקיפה:

POST /v1/query  
Body: בקשת Start בקידוד Norito  
Response: אצווה ראשונה; ‎`cursor=null`

2) עקיפה ל-`stored` עם יחידות גז:

POST /v1/query?cursor_mode=stored&gas_units=100  
Body: בקשת Start בקידוד Norito  
Response: אצווה ראשונה ו-`cursor` להמשך

אם `pipeline.query_stored_min_gas_units=200`, הבקשה לעיל תידחה עם שגיאת NotPermitted.

---

לרשימת נקודות הקצה המלאה של Torii ראו את מדור ה-Reference. עמוד זה מתמקד בבחירת מצב ובתנהגות של נתיב snapshot בלבד.

</div>
