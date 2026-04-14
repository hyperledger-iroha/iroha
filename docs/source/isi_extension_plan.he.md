<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9648381ac7cc1716ffd3c48aca425ed17a6afe1ac73bdeff866ebbbd9147cf68
source_last_modified: "2026-03-30T18:22:55.972718+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# תוכנית הרחבה של ISI (v1)

הערה זו חותמת על סדר העדיפות עבור ההוראות המיוחדות והלכידות החדשות של Iroha
אינוריאנטים בלתי ניתנים למשא ומתן עבור כל הוראה לפני היישום. ההזמנה תואמת
סיכון אבטחה ותפעול ראשון, תפוקת UX שנית.

## מחסנית עדיפות

1. **RotateAccountSignatory** – נדרש לסיבוב מפתח היגייני ללא הגירות הרסניות.
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** - ספק חוזה דטרמיניסטי
   הרג מתגים ושיחזור אחסון עבור פריסות שנפגעו.
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** - הרחבת שוויון המטא נתונים לנכס קונקרטי
   מאזן כך שכלי צפייה יכולים לתייג אחזקות.
4. **BatchMintAsset** / **BatchTransferAsset** - מסייעים דטרמיניסטים להוצאת מטען לשמירה על גודל המטען
   ולחץ נפילה של VM ניתן לניהול.

## אינוריאנטי הוראה

### SetAssetKeyValue / RemoveAssetKeyValue
- השתמש מחדש במרחב השמות `AssetMetadataKey` (`state.rs`) כדי שמפתחות WSV קנוניים יישארו יציבים.
- אכוף מגבלות גודל וסכימה של JSON באופן זהה למסייעי מטא נתונים בחשבון.
- פליט `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` עם `AssetId` המושפע.
- דרוש את אותם אסימוני הרשאה כמו עריכות קיימות של מטא נתונים של נכס (בעל הגדרה OR
  מענקים בסגנון `CanModifyAssetMetadata`).
- בטל אם רשומת הנכס חסרה (ללא יצירה מרומזת).### RotateAccountSignatory
- החלפה אטומית של החותם ב-`AccountId` תוך שמירה על מטא נתונים ומקושרים של החשבון
  משאבים (נכסים, טריגרים, תפקידים, הרשאות, אירועים ממתינים).
- ודא שהחתם הנוכחי תואם למתקשר (או סמכות שהוקצתה באמצעות אסימון מפורש).
- דחה אם המפתח הציבורי החדש כבר מגבה חשבון קנוני אחר.
- עדכן את כל המפתחות הקנוניים שמטמיעים את מזהה החשבון ומבטלים מטמונים לפני הביצוע.
- פלט `AccountEvent::SignatoryRotated` ייעודי עם מפתחות ישנים/חדשים למסלולי ביקורת.
- פיגום הגירה: הסתמכו על `AccountAlias` + `AccountRekeyRecord` (ראה `account::rekey`) אז
  חשבונות קיימים יכולים לשמור על כריכות כינוי יציבות במהלך שדרוג מתגלגל ללא הפסקות hash.

### DeactivateContractInstance
- הסר או מצבה את כריכת `(namespace, contract_id)` תוך שמירה על נתוני מוצא
  (מי, מתי, קוד סיבה) לפתרון בעיות.
- דרוש את אותה ערכת הרשאות ממשל כמו הפעלה, עם ווים של מדיניות לא לאפשר
  השבתה של מרחבי שמות של מערכת הליבה ללא אישור מוגבר.
- דחה כאשר המופע כבר לא פעיל כדי לשמור על יומני אירועים דטרמיניסטים.
- פלט `ContractInstanceEvent::Deactivated` שצופים במורד הזרם יכולים לצרוך.### RemoveSmartContractBytes
- אפשר גיזום של קוד בתים מאוחסן על ידי `code_hash` רק כאשר אין מניפסטים או מופעים פעילים
  התייחסות לחפץ; אחרת נכשל עם שגיאת תיאור.
- רישום מראות שער הרשאה (`CanRegisterSmartContractCode`) בתוספת רמת מפעיל
  שומר (לדוגמה, `CanManageSmartContractStorage`).
- ודא שה-`code_hash` שסופק תואם לעיכול הגוף המאוחסן רגע לפני המחיקה כדי להימנע
  ידיות מעופשות.
- Emit `ContractCodeEvent::Removed` עם hash ומטא נתונים של מתקשרים.

### BatchMintAsset / BatchTransferAsset
- סמנטיקה של הכל או כלום: או שכל טופלה מצליחה או שההוראה תבוטל ללא צד
  אפקטים.
- וקטורי קלט חייבים להיות מסודרים בצורה דטרמיניסטית (ללא מיון מרומז) ומוגדרים על ידי תצורה
  (`max_batch_isi_items`).
- פליט אירועי נכסים לפי פריט כך שהחשבונאות במורד הזרם תישאר עקבית; הקשר אצווה הוא תוסף,
  לא תחליף.
- בדיקות הרשאות משתמשות מחדש בלוגיקה קיימת של פריט בודד לכל יעד (בעל נכס, בעל הגדרה,
  או מוענקת יכולת) לפני מוטציה של מצב.
- ערכות גישה מייעצות חייבות לאחד את כל מפתחות הקריאה/כתיבה כדי לשמור על התאמה אופטימית.

## פיגומי יישום- מודל הנתונים נושא כעת פיגומים `SetAssetKeyValue` / `RemoveAssetKeyValue` עבור מטא נתונים לאיזון
  עריכות (`transparent.rs`).
- מבקרים של מבצעים חושפים מצייני מיקום שישיגו הרשאות ברגע שהחיווט של המארח ינחת
  (`default/mod.rs`).
- סוגי אב טיפוס של Rekey (`account::rekey`) מספקים אזור נחיתה להגירות מתגלגלות.
- המדינה העולמית כוללת `account_rekey_records` עם מקשים `AccountAlias` כדי שנוכל לביים כינוי →
  הגירות חתומות מבלי לגעת בקידוד `AccountId` ההיסטורי.

## IVM שרטוט Syscall

- shims מארח עבור `DeactivateContractInstance` / `RemoveSmartContractBytes` נשלח כ
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) ו
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44), שניהם צורכים Norito TLVs המשקפים את
  מבני ISI קנוניים.
- הארך את `abi_syscall_list()` רק לאחר שמטפלי מארחים משקפים נתיבי ביצוע של `iroha_core` כדי לשמור
  ABI hashes יציב במהלך הפיתוח.
- עדכון Kotodama הורדה לאחר התייצבות של מספרי סיסמא; להוסיף כיסוי זהוב למורחב
  פני השטח בו זמנית.

## סטטוס

הסדרים והאינוריאנטים לעיל מוכנים ליישום. סניפי מעקב צריכים להתייחס
מסמך זה בעת חיווט נתיבי ביצוע וחשיפה ל-syscall.