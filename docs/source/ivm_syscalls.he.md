<!-- Hebrew translation of docs/source/ivm_syscalls.md -->

---
lang: he
direction: rtl
source: docs/source/ivm_syscalls.md
status: complete
translator: manual
---

<div dir="rtl">

# ABI של קריאות מערכת (IVM)

מסמך זה מגדיר את מספרי הקריאות במערכת (syscalls) של IVM, מוסכמות ה־pointer‑ABI, מרווחי המספור השמורים, והטבלה הקנונית של הקריאות שממשק ה־Kotodama מייצר אליהן. המסמך משלים את `ivm.md` (ארכיטקטורה) ואת `kotodama_grammar.md` (שפה).

## ניהול גרסאות
- אוסף הקריאות הנתמכות תלוי בשדה `abi_version` שבכותרת הבייטקוד. במהדורה הראשונה נתמך רק `abi_version = 1`; ערכים אחרים נדחים בקבלה. מספרים שאינם מוכרים עבור `abi_version` הפעיל יגרמו ל־trap דטרמיניסטי עם `E_SCALL_UNKNOWN`.
- שדרוגי runtime שומרים על `abi_version = 1` ואינם מרחיבים syscalls או טיפוסי pointer‑ABI.
- עלויות הגז של קריאות המערכת הן חלק מלוח גזים ממוסגר לפי גרסה וכפופות לגרסת הכותרת. ראו `ivm.md` (מדיניות גז).

## תחומי מספור
- ‎`0x00..=0x1F`: ליבת ה־VM/כלי עזר (כלים לפיתוח; אינם זמינים תחת `CoreHost`).
- ‎`0x20..=0x5F`: גשר ISI של Iroha Core (יציב ב‑ABI v1).
- ‎`0x60..=0x7F`: הרחבות ISI שמוגנות ע״י פיצ'רים פרוטוקוליים (עדיין חלק מ‑ABI v1 כשהן מופעלות).
- ‎`0x80..=0xFF`: עזרי מארח/קריפטו וחריצים שמורים; רק מספרים שמופיעים ברשימת ההיתרים של ABI v1 מתקבלים.

## עזרי Durable (ABI v1)
- קריאות העזר למצב durable (‎0x50–0x5A: STATE_{GET,SET,DEL}, ‏ENCODE/DECODE_INT, ‏BUILD_PATH_*, ‏קידוד/פענוח JSON/SCHEMA) הן חלק מ־ABI V1 ונכללות בחישוב `abi_hash`.
- CoreHost מחבר את STATE_{GET,SET,DEL} למצב חוזים עמיד מגובה־WSV; מארחי dev/בדיקה יכולים להשתמש ב־overlays או בהתמדה מקומית אך חייבים לשמור על אותה התנהגות נצפית.

## מוסכמת pointer‑ABI (קריאות מערכת של חוזים חכמים)
- ארגומנטים ממוקמים ברגיסטרים `r10+` כערכי `u64` גולמיים או מצביעים באזור INPUT אל מעטפות TLV של Norito בלתי־ניתנות לשינוי (למשל `AccountId`, ‏`AssetDefinitionId`, ‏`Name`, ‏`Json`, ‏`NftId`).
- ערכי חזרה סקלריים הם ערכי `u64` שהמארח מחזיר. מצביעים מוחזרים באמצעות כתיבה של המארח אל `r10`.

## טבלת הקריאות הקנונית (תת־קבוצה)

| הקס | שם | ארגומנטים (`r10+`) | ערך חזרה | גז (בסיס + משתנה) | הערות |
|-----|-----|---------------------|-----------|---------------------|--------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | כתיבת פרט לחשבון |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `amount:u64` | `u64=0` | `G_mint` | הנפקת כמות `amount` לנכס בחשבון |
| 0x23 | BURN_ASSET | `&AccountId`, `&AssetDefinitionId`, `amount:u64` | `u64=0` | `G_burn` | שריפת הכמות מהחשבון |
| 0x24 | TRANSFER_ASSET | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `amount:u64` | `u64=0` | `G_transfer` | העברת סכום בין חשבונות |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | התחלת אצוות FASTPQ כך שהעברות יאוגדו |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | סיום האצווה והמרתה להוראת Batch אחת |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | החלת אצווה מקודדת Norito בקריאה יחידה |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | רישום NFT חדש |
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | העברת בעלות על NFT |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | עדכון מטא־נתונים של NFT |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | שריפת NFT |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` | עזר; מוגן ע״י פיצ'ר |
| 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | מנהל; מוגן ע״י פיצ'ר |
| 0xA4 | GET_AUTHORITY | – (המארח כותב תוצאה) | `&AccountId` | `G_get_auth` | המארח כותב מצביע ל־AccountId הנוכחי ב־`r10` |
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, אופציונלי `root_out:u64` | `u64=len` | `G_mpath + len` | כתיבת נתיב (עלה→שורש) ואופציונלית root |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, אופציונלי `depth_cap:u64`, אופציונלי `root_out:u64` | `u64=depth` | `G_mpath + depth` | פורמט קומפקטי `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT | `reg_index:u64`, `out_ptr:u64`, אופציונלי `depth_cap:u64`, אופציונלי `root_out:u64` | `u64=depth` | `G_mpath + depth` | אותו פורמט קומפקטי עבור קומיטמנט של רגיסטר |

## הערות
- כל ארגומנט מצביע מפנה למעטפת TLV של Norito באזור INPUT ועובר אימות בדפרנס הראשון (`E_NORITO_INVALID` במקרה של שגיאה).
- כל המוטציות מבוצעות דרך ההרצה הסטנדרטית של Iroha (`CoreHost`), ולא ישירות על ידי ה־VM.
- קבועי הגז המדויקים (`G_*`) מוגדרים בלוח הגזים הפעיל; ראו `ivm.md`.

## שגיאות
- ‎`E_SCALL_UNKNOWN`: מספר קריאה שאינו מוכר עבור `abi_version` הפעיל.
- שגיאות אימות קלט נלכדות כ־trap (למשל `E_NORITO_INVALID` עבור TLV פגום).

## מסמכים מקבילים
- ארכיטקטורה וסמנטיקה של ה־VM: ‏`ivm.md`
- שפה ומיפוי מובנים: ‏`docs/source/kotodama_grammar.md`

## הערת יצירה
- ניתן להפיק רשימה מלאה של קבועי הקריאות מהקוד באמצעות:
  - ‏`make docs-syscalls` → מייצר את `docs/source/ivm_syscalls_generated.md`
  - ‏`make check-docs` → מאמת שהטבלה הגנרית מעודכנת (שימושי ב־CI)
- תת־הקבוצה לעיל נשארת כטבלה קנונית שעומדת לרשות חוזים.

## דוגמאות TLV לממשק אדמין/תפקיד (מארח מדומה)

סעיף זה מתעד את מבני ה־TLV וה־JSON המינימליים שהמארח המדומה (Mock WSV) מקבל עבור קריאות מערכת בסגנון אדמין שנמצאות בשימוש בבדיקות. כל הארגומנטים עומדים ב־pointer‑ABI (מעטפות Norito באזור INPUT). מארחי ייצור עשויים להשתמש בסכמות עשירות יותר; הדוגמאות כאן נועדו להבהיר טיפוסים וצורות יסוד.

- REGISTER_PEER / UNREGISTER_PEER
  - ארגומנטים: `r10=&Json`
  - דוגמת JSON: ‎`{ "peer": "peer-id-or-info" }`

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - ארגומנטים: `r10=&Json`
    - JSON מינימלי: ‎`{ "name": "t1" }` (שדות נוספים מתעלמים מהם במארח המדומה)
  - REMOVE_TRIGGER:
    - ארגומנטים: `r10=&Name` (שם הטריגר)
  - SET_TRIGGER_ENABLED:
    - ארגומנטים: `r10=&Name`, ‏`r11=enabled:u64` (0 = מושבת, ערך אחר = פעיל)

- Roles: קריאות CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - ארגומנטים: `r10=&Name` (שם תפקיד), ‏`r11=&Json` (קבוצת הרשאות)
    - JSON מקבל מפתח `"perms"` או `"permissions"` עם מערך של שמות הרשאות.
    - דוגמאות:
      - ‎`{ "perms": [ "mint_asset:rose#wonder" ] }`
      - ‎`{ "permissions": [ "read_assets:ed0120...@wonder", "transfer_asset:rose#wonder" ] }`
    - קידומות שמות נתמכות במארח:
      - ‎`register_domain`, ‏`register_account`, ‏`register_asset_definition`
      - ‎`read_assets:<account_id>`
      - ‎`mint_asset:<asset_definition_id>`
      - ‎`burn_asset:<asset_definition_id>`
      - ‎`transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - ארגומנטים: `r10=&Name`
    - נכשל אם תפקיד משויך עדיין לחשבון כלשהו.
  - GRANT_ROLE / REVOKE_ROLE:
    - ארגומנטים: `r10=&AccountId` (נושא), ‏`r11=&Name` (שם תפקיד)

- פעולות ביטול רישום (דומיין/חשבון/נכס): תנאים (מארח מדומה)
  - ‏UNREGISTER_DOMAIN (`r10=&DomainId`) נכשל אם קיימים חשבונות או הגדרות נכס בדומיין.
  - ‏UNREGISTER_ACCOUNT (`r10=&AccountId`) נכשל אם לחשבון מאזן שאינו אפס או NFT בבעלות.
  - ‏UNREGISTER_ASSET (`r10=&AssetDefinitionId`) נכשל אם קיימים מאזנים לנכס.

## הערות נוספות
- הדוגמאות משקפות את המארח המדומה המשמש בבדיקות; מארחי נוד אמיתיים עשויים לספק סכמות אדמין עשירות יותר או לדרוש אימות נוסף. מוסכמות ה־pointer‑ABI עדיין מחייבות: TLVs באזור INPUT, גרסה 1, מזהי טיפוס תואמים ובדיקות hash תקינות.

</div>
