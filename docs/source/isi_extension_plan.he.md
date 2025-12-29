<!-- Hebrew translation of docs/source/isi_extension_plan.md -->

---
lang: he
direction: rtl
source: docs/source/isi_extension_plan.md
status: complete
translator: manual
---

<div dir="rtl">

# תוכנית הרחבת ISI (v1)

מסמך זה מאשר את סדר העדיפויות עבור ההרחבות החדשות של Iroha Special Instructions ומרכז את האינבריאנטים המחייבים לכל הוראה לפני תחילת הפיתוח. הסדר נקבע: סיכון אבטחה והפעלה קודם, תפוקת UX אחר כך.

## מחסנית עדיפויות

1. **RotateAccountSignatory** – נדרש לרוטציית מפתחות נקייה ללא מיגרציות הרסניות.
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – מספקות מתגי עצירה דטרמיניסטיים ושחרור אחסון לפריסה שנפרצה.
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – מרחיבות פריטי מטא־דאטה גם ליתרות נכס בפועל כדי שתשתיות observability יוכלו לתייג החזקות.
4. **BatchMintAsset** / **BatchTransferAsset** – עוזרות לניהול Fan-out דטרמיניסטי כך שגודל המטען ועומס ה-VM fallback יישארו בשליטה.

## אינבריאנטים להוראות

### SetAssetKeyValue / RemoveAssetKeyValue
- לעשות שימוש חוזר במרחב השמות `AssetMetadataKey` (`state.rs`) כדי לשמר מפתחות WSV קנוניים.
- לאכוף מגבלות גודל JSON וסכמה זהות לאלו של מטא־דאטה לחשבון.
- לפלוט `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` יחד עם ה-`AssetId` הרלוונטי.
- לדרוש את אותם אסימוני הרשאה כמו בעריכת מטא־דאטה קיימת של נכס (בעל ההגדרה או הרשאות בסגנון `CanModifyAssetMetadata`).
- לעצור אם רשומת הנכס חסרה (ללא יצירה מרומזת).

### RotateAccountSignatory
- לבצע החלפה אטומית של החותם ב-`AccountId` תוך שמירה על מטא־דאטה ומשאבים משויכים (נכסים, טריגרים, תפקידים, הרשאות, אירועים תלויים).
- לוודא שהחותם הנוכחי תואם לקורא (או בעל סמכות דרך אסימון מפורש).
- לדחות אם המפתח הציבורי החדש כבר משויך לחשבון אחר באותו דומיין.
- לעדכן את כל המפתחות הקנוניים שבהם ה-ID משובץ ולבטל קאשינג לפני commit.
- לפלוט `AccountEvent::SignatoryRotated` ייעודי עם מפתחות ישן/חדש לתיעוד.
- תשתית מיגרציה: להוסיף `AccountLabel` ו-`AccountRekeyRecord` (`account::rekey`) כך שניתן יהיה למפות חשבונות קיימים לתוויות יציבות במהלך שדרוג מתגלגל ללא שינויי hash.

### DeactivateContractInstance
- להסיר או להחצין (tombstone) את הצימוד `(namespace, contract_id)` תוך שימור נתוני מקור (מי, מתי, קוד סיבה) לצורך диагностиיקה.
- לדרוש את אותה סט הרשאות ממשל כמו הפעלה ולהוסיף hooks למדיניות המונעים השבתת מרחבי שמות קריטיים ללא אישור מוגבר.
- לדחות כאשר המופע כבר מושבת כדי לשמור על דטרמיניזם ביומני אירועים.
- לפלוט `ContractInstanceEvent::Deactivated` לצרכני downstream.

### RemoveSmartContractBytes
- לאפשר ניקוי בייטקוד לפי `code_hash` רק כשאין מניפסטים או מופעים פעילים שמצביעים על הארכיון; אחרת להיכשל עם שגיאה תיאורית.
- שער ההרשאות צריך לשקף רישום (`CanRegisterSmartContractCode`) בתוספת שמירה ברמת מפעיל (למשל `CanManageSmartContractStorage`).
- לאמת שה-`code_hash` שסופק תואם לחותמת הגוף לפני המחיקה כדי למנוע ידיות מיושנות.
- לפלוט `ContractCodeEvent::Removed` עם החותם ומידע על הקורא.

### BatchMintAsset / BatchTransferAsset
- סמנטיקה של “הכול או כלום”: כל הצמדים מצליחים או שההוראה מבוטלת ללא תופעות לוואי.
- וקטורי קלט חייבים להיות מסודרים באופן דטרמיניסטי (ללא מיון מרומז) ולהיות מוגבלים לפי קונפיגורציה (`max_batch_isi_items`).
- לפלוט אירועי נכס לכל פריט כדי לשמר עקיבות חשבונאית; ההקשר הבאטצ'י הוא תוספת בלבד.
- בדיקות הרשאה משתמשות מחדש בלוגיקה הקיימת לכל פריט (בעל נכס, בעל הגדרה, או יכולות שהוענקו) טרם שינוי המצב.
- קבוצות הגישה המייעצות חייבות לאחד את כל מפתחות הקריאה/כתיבה כדי לשמר תקינות של concurrency אופטימיסטי.

## תשתיות יישום

- דגם הנתונים כבר כולל שלדים עבור `SetAssetKeyValue` / `RemoveAssetKeyValue` לעריכת מטא־דאטה של יתרות (`transparent.rs`).
- מבקרי האקזקיוטור חושפים Placeholder-ים שיאכפו הרשאות לאחר חיבור צד הוסט (`default/mod.rs`).
- טיפוסי אב-טיפוס לרייקי (`account::rekey`) מספקים נקודת נחיתה למיגרציות מתגלגלות.
- מצב העולם מכיל `account_rekey_records` לפי `AccountLabel`, מה שמאפשר לתאם הגירת תווית→חותם מבלי לגעת בקידוד ההיסטורי של `AccountId`.

## ניסוח Syscall ב-IVM

- Shim-ים בהוסט ל-`DeactivateContractInstance` / `RemoveSmartContractBytes` זמינים כעת דרך `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) ו-`SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44), שניהם מקבלים TLV מסוג Norito התואם למבני ה-ISI.
- להרחיב את `abi_syscall_list()` רק לאחר שהוסט משקף את מסלולי הביצוע של `iroha_core`, כדי לשמור על Hash של ABI יציב במהלך הפיתוח.
- לעדכן את ההורדה של Kotodama ברגע שמספרי syscall מתקבעים ולהוסיף כיסוי זהב למשטח המורחב.

## סטטוס

סדר העדיפויות והאינבריאנטים לעיל מוכנים ליישום. ענפים המשך צריכים להפנות למסמך זה בעת חיבור מסלולי הביצוע וחשיפת ה-syscall.

</div>
