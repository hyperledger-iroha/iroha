<!-- Hebrew translation of docs/account_structure_sdk_alignment.md -->

---
lang: he
direction: rtl
source: docs/account_structure_sdk_alignment.md
status: complete
translator: manual
---

<div dir="rtl">

# הערת פריסה ל-IH-B32 עבור צוותי SDK וקודקים

צוותים רלוונטיים: ‏Rust SDK, ‏TypeScript/JavaScript SDK, ‏Python SDK, ‏Kotlin SDK, כלי קודקים.

הקשר: ‏`docs/account_structure.md` עודכן עם עיצוב IH-B32 הסופי, מיפויי אינטרופרביליות ורשימת המשימות לביצוע. נא ליישר את התנהגות הספריות והבדיקות מול המפרט הקנוני.

הפניות עיקריות (עוגני שורה):
- כללי הקידוד של IH-B32 וחובת אותיות קטנות — ‏`docs/account_structure.md:120`
- נוהל יצירת כינוי IH58 וווקטורים נורמטיביים — ‏`docs/account_structure.md:151`
- רשימת בדיקות היישום — ‏`docs/account_structure.md:276`

פריטי פעולה:
1. להציג את IH-B32 כברירת המחדל בכל SDK ולהסיר ניתוח `alias@domain` כך שכל הקלטים יעברו דרך הקודק הקנוני.
2. לממש המרות CAIP-10 ו-IH58 בדיוק כפי שמתועד.
3. להוסיף כיסוי בדיקות באמצעות הווקטורים הנורמטיביים (Ed25519 ו-secp256k1) ומקרה הכשל של בדיקת שגיאת checksum/אי-התאמת רשת.
4. לשקף את חוזה השאילת ממאגר הרישום: להניח שמניפסטי Nexus מפרסמים את הטופל `{discriminant, ih58_prefix, chain_alias}` ולאכוף את סמנטיקת ה-TTL.
5. לתאם הערות גרסה כדי שמטמיעי צד ג' ידעו שתמיכת IH-B32 זמינה באותו מחזור בכל השפות.

נא לאשר לאחר שהקודקים/בדיקות עודכנו. שאלות פתוחות נרשמות בשרשור ה-RFC של כתובות החשבון.
</div>
