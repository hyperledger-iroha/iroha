<!-- Hebrew translation of docs/source/error_mapping.md -->

---
lang: he
direction: rtl
source: docs/source/error_mapping.md
status: complete
translator: manual
---

<div dir="rtl">

# מדריך מיפוי שגיאות

עודכן לאחרונה: ‎2025-08-21

המדריך ממפה מצבי כשל שכיחים ב-Iroha לקטגוריות שגיאה יציבות שנחשפות על ידי מודל הנתונים. השתמשו בו כדי לעצב בדיקות ולהפוך את טיפול השגיאות בצד הלקוח לצפוי.

עקרונות
- מסלולי הוראות ושאילתות מחזירים enums מובנים. הימנעו מפאניקות; דיווחו קטגוריה ספציפית ככל האפשר.
- הקטגוריות יציבות אך המסרים עשויים להשתנות. לקוחות צריכים להתאים לפי הקטגוריה, לא לפי מחרוזות חופשיות.

קטגוריות
- `InstructionExecutionError::Find`: ישות חסרה (נכס, חשבון, דומיין, NFT, תפקיד, טריגר, הרשאה, מפתח ציבורי, בלוק, טרנזקציה). דוגמה: הסרת מפתח מטא-דאטה שלא קיים תחזיר `Find(MetadataKey)`.
- `InstructionExecutionError::Repetition`: רישום כפול או מזהה מתנגש. כולל את סוג ההוראה ואת `IdBox` הכפול.
- `InstructionExecutionError::Mintability`: הפרת אינווריאנט של mintability (`Once` שנוצל פעמיים, `Limited(n)` מעבר לתקרה, ניסיון לאסור `Infinitely`). דוגמאות: ניסיון להטביע פעמיים נכס שהוגדר כ-`Once` יוביל ל-`Mintability(MintUnmintable)`; הגדרת `Limited(0)` תחזיר `Mintability(InvalidMintabilityTokens)`.
- `InstructionExecutionError::Math`: שגיאות בתחום נומרי (גלישה, חילוק באפס, ערך שלילי, כמות לא מספיקה). דוגמה: שריפת יותר מהמלאי תגרום ל-`Math(NotEnoughQuantity)`.
- `InstructionExecutionError::InvalidParameter`: פרמטר הוראה או קונפיגורציה לא חוקיים (למשל טריגר זמן בעבר). השתמשו גם עבור מטעני חוזה פגומים.
- `InstructionExecutionError::Evaluate`: אי-התאמה בין DSL/סכמה לבין מבנה או סוגי ההוראה. דוגמה: מפרט מספרי שגוי לערך נכס יחזיר `Evaluate(Type(AssetNumericSpec(..)))`.
- `InstructionExecutionError::InvariantViolation`: הפרת אינווריאנט מערכת שאינו נכנס לקטגוריות אחרות. דוגמה: ניסיון להסיר את החותם האחרון.
- `InstructionExecutionError::Query`: עטיפה של `QueryExecutionFail` כאשר שאילתה נכשלת במהלך הרצת הוראה.

`QueryExecutionFail`
- `Find`: ישות חסרה בהקשר של השאילתה.
- `Conversion`: סוג לא תואם לציפיות השאילתה.
- `NotFound`: Cursor חי שאינו קיים.
- `CursorMismatch` / `CursorDone`: שגיאות פרוטוקול Cursor.
- `FetchSizeTooBig`: חריגה ממגבלת שרת.
- `GasBudgetExceeded`: חריגה מתקציב הגז/המטיריאליזציה של ביצוע השאילתה.
- `InvalidSingularParameters`: פרמטרים בלתי נתמכים עבור שאילתות סינגולריות.
- `CapacityLimit`: חנות השאילתות החיות מלאה.

טיפים לבדיקות
- העדיפו בדיקות יחידה קרובות למקור השגיאה. למשל, אי התאמה במפרט מספרי של נכס ניתן לייצר בבדיקות מודל הנתונים.
- בדיקות אינטגרציה צריכות לכסות מיפוי end-to-end עבור מקרים מייצגים (רישום כפול, מפתח חסר בהסרה, העברה ללא בעלות).
- שמרו על אסרטים חסינים באמצעות התאמה ל-variant של enum ולא למסרים טקסטואליים.

</div>
