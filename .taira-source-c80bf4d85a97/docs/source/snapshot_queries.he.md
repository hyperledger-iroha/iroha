<!-- Hebrew translation of docs/source/snapshot_queries.md -->

---
lang: he
direction: rtl
source: docs/source/snapshot_queries.md
status: complete
translator: manual
---

<div dir="rtl">

# פעולות שאילתת Snapshot

שאילתות Snapshot רצות מול `StateView` עקבי, כך שמפעילים יכולים לשרת עומסי קריאה מבלי להשפיע על מצב הקונצנזוס. מסמך זה מרכז את מעקות הביטחון התפעוליים שנלווים לנתיב ה-Snapshot.

## תקציבי משאבים ומכסות

- `pipeline.query_stored_min_gas_units` שולט באחסון cursors בצד השרת. כאשר הערך גדול מאפס:
  - בקשות `Start` שבוחרות במצב `stored` חייבות לספק לפחות ערך זה של `gas_units` דרך פרמטר ה-query של Torii.
  - בקשות `Continue` חייבות לשקף את התקציב ב-Payload של Norito באמצעות `ForwardCursor.gas_budget`. תקציב הנמוך מהמינימום יידחה עם שגיאת ולידציה `NotPermitted`.
- `live_query_store.capacity` מגביל את מספר ה-cursors המאוחסנים הכולל.
- `live_query_store.capacity_per_user` מטיל מכסות לכל בעל סמכות. חריגה תוביל ל-`AuthorityQuotaExceeded`.

## TTL של Cursor ואיסוף זבל

cursors מאוחסנים יורשים את `live_query_store.idle_time`. משימת ניקוי ברקע מסירה cursors שה-`last_access_time` שלהם חרג מה-TTL. בקשות `Continue` לאחר מחיקה יחזירו `QueryExecutionFail::Expired` כדי שהלקוחות יתאוששו בצורה חיננית.

## טלמטריה

כאשר הטלמטריה פעילה, חשופות שתי משפחות מדדים:

- חזית Torii:
  - `torii_query_snapshot_requests_total{mode}` – מונה בקשות.
  - `torii_query_snapshot_first_batch_ms{mode}` – היסטוגרמת לטנציה לאצווה הראשונה.
  - `torii_query_snapshot_gas_consumed_units_total{mode}` – סכימה של תקציבי הגז שהלקוחות סיפקו (פרמטרי query או payload של cursor).
- נתיב Snapshot ליבה:
  - `query_snapshot_lane_first_batch_ms{mode}` – זמן ביצוע בתוך הנתיב.
  - `query_snapshot_lane_first_batch_items{mode}` – מספר פריטים לכל אצווה.
  - `query_snapshot_lane_remaining_items{mode}` – מד גודל שארית.
  - `query_snapshot_lane_cursors_total{mode}` – מונה הנפקת cursors מאוחסנים.

כל המונים מתויגים לפי מצב cursor (`ephemeral` או `stored`) כדי שמפעילים יוכלו לעקוב אחר שיעורי האימוץ ולעמוד על עמידה בתקציבים.

## רשימת תיוג למפעיל

1. הגדירו את `pipeline.query_default_cursor_mode` כדי לבחור בין `ephemeral` ל-`stored` כברירת מחדל.
2. בעת הפעלת cursors מאוחסנים, הגדירו את `pipeline.query_stored_min_gas_units` ונטרו את `torii_query_snapshot_gas_consumed_units_total` כדי לוודא שהלקוחות מכבדים את התקציב.
3. כיילו את `live_query_store.capacity` ו-`capacity_per_user` בהתאם לעומסים הצפויים, ועקבו אחרי `query_snapshot_lane_cursors_total` כדי להבין כמה cursors מופקים.
4. בחנו את `query_snapshot_lane_remaining_items` כדי לזהות cursors ארוכי-חיים והתאימו את `idle_time` אם הניקוי אגרסיבי או רופף מדי.

מעקות אלו שומרים על התנהגות צפויה של שאילתות Snapshot ומספקים נראות לצריכת המשאבים מצד המפעיל.

</div>
