# מבנה חשבון RFC

**סטטוס:** מקובל (ADDR-1)  
**קהל:** מודל נתונים, Torii, Nexus, Wallet, צוותי ממשל  
**בעיות קשורות:** TBD

## סיכום

מסמך זה מתאר את ערימת הכתובת לחשבון המשלוח המיושמת ב
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) וה
כלי עבודה נלווים. הוא מספק:

- **כתובת Iroha Base58 (IH58)** מסוכמת, הפונה לאדם, שהופקה על ידי
  `AccountAddress::to_ih58` שקושר אפליה שרשרת לחשבון
  בקר ומציע צורות טקסטואליות ידידותיות לאינטררופיות דטרמיניסטיות.
- בוררי דומיינים עבור דומיינים מרומזים של ברירת מחדל ותקצירים מקומיים, עם א
  תג בורר רישום גלובלי שמור עבור ניתוב עתידי בגיבוי Nexus (ה
  חיפוש הרישום **עדיין לא נשלח**).

## מוטיבציה

ארנקים וכלי עבודה מחוץ לשרשרת מסתמכים על כינויי ניתוב גולמיים של `alias@domain` כיום. זה
יש שני חסרונות עיקריים:

1. **אין כריכת רשת.** למחרוזת אין סכום בדיקה או קידומת שרשרת, אז למשתמשים
   יכול להדביק כתובת מהרשת הלא נכונה ללא משוב מיידי. ה
   העסקה תידחה בסופו של דבר (אי-התאמה של שרשרת) או, גרוע מכך, תצליח
   כנגד חשבון לא מכוון אם היעד קיים באופן מקומי.
2. **התנגשות דומיינים.** דומיינים הם מרחב שמות בלבד וניתן לעשות בהם שימוש חוזר בכל אחד מהם
   שרשרת. פדרציה של שירותים (אפוטרופוסים, גשרים, זרימות עבודה חוצות שרשרת)
   הופך שביר כי `finance` בשרשרת A אינו קשור ל-`finance` ב
   שרשרת ב.

אנחנו צריכים פורמט כתובת ידידותי לאדם ששומר מפני שגיאות העתקה/הדבקה
ומיפוי דטרמיניסטי מ שם דומיין לשרשרת הסמכותית.

## יעדים

- תאר את מעטפת IH58 Base58 המיושמת במודל הנתונים ואת
  כללי ניתוח/כינוי קנוניים ש-`AccountId` ו-`AccountAddress` עוקבים אחריהם.
- מקודד את מבחנה השרשרת המוגדר ישירות לכל כתובת ו
  להגדיר את תהליך הממשל/רישום שלו.
- תאר כיצד להציג רישום דומיינים גלובלי מבלי לשבור זרם
  פריסות וציין כללי נורמליזציה/אנטי זיוף.

## ללא מטרות

- יישום העברות נכסים צולבות שרשרת. שכבת הניתוב מחזירה רק את
  שרשרת היעד.
- השלמת ממשל להנפקת תחום גלובלי. RFC זה מתמקד בנתונים
  פרימיטיביות מודל ותחבורה.

## רקע

### כינוי ניתוב נוכחי

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical IH58 literal (no `@domain` suffix)
Parse accepts:
- IH58 (preferred), `sora` compressed, or canonical hex (`0x...`) inputs, with
  optional `@<domain>` suffixes for explicit routing hints.
- `<label>@<domain>` aliases resolved through the account-alias resolver
  (Torii installs one; plain data-model parsing requires a resolver to be set).
- `<public_key>@<domain>` where `public_key` is the canonical multihash string.
- `uaid:<hex>` / `opaque:<hex>` literals resolved via UAID/opaque resolvers.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` חיים מחוץ ל-`AccountId`. צמתים בודקים את `ChainId` של העסקה
נגד תצורה במהלך הקבלה (`AcceptTransactionFail::ChainIdMismatch`)
ודוחים עסקאות זרות, אבל מחרוזת החשבון עצמה נושאת לא
רמז לרשת.

### מזהי דומיין

`DomainId` עוטף `Name` (מחרוזת מנורמלת) והטווח הוא לשרשרת המקומית.
כל רשת יכולה לרשום `wonderland`, `finance` וכו' באופן עצמאי.

### הקשר Nexus

Nexus אחראית על תיאום בין רכיבים (נתיבים/מרחבי נתונים). זה
כרגע אין מושג של ניתוב דומיינים חוצי שרשרת.

## עיצוב מוצע

### 1. מבחין שרשרת דטרמיניסטית

`iroha_config::parameters::actual::Common` חושף כעת:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **אילוצים:**
  - ייחודי לכל רשת פעילה; מנוהל באמצעות רישום ציבורי חתום עם
    טווחים שמורים מפורשים (לדוגמה, `0x0000–0x0FFF` בדיקה/פיתוח, `0x1000–0x7FFF`
    הקצאות קהילה, `0x8000–0xFFEF` מאושרת ממשל, `0xFFF0–0xFFFF`
    שמור).
  - בלתי משתנה עבור שרשרת פועלת. שינוי זה דורש מזלג קשה וא
    עדכון הרישום.
- **ממשל ורישום (מתוכנן):** מערך ממשל מרובה חתימות יהיה
  לשמור על רישום JSON חתום הממפה מפלים לכינויים אנושיים ו
  מזהי CAIP-2. הרישום הזה עדיין לא חלק מזמן הריצה שנשלח.
- **שימוש:** משורשר דרך כניסת המדינה, Torii, SDKs וממשקי API של ארנק כך
  כל רכיב יכול להטמיע או לאמת אותו. חשיפת CAIP-2 נותרה עתיד
  משימה אינטררופית.

### 2. קודקים כתובים קנוניים

מודל הנתונים של Rust חושף ייצוג מטען קנוני יחיד
(`AccountAddress`) שיכולים להיפלט כמספר פורמטים הפונים לאדם. IH58 הוא
פורמט החשבון המועדף לשיתוף ופלט קנוני; הדחוס
טופס `sora` הוא האופציה השנייה הטובה ביותר, סורה בלבד עבור UX שבו האלפבית קאנה
מוסיף ערך. hex קנוני נשאר עזר לניפוי באגים.

- **IH58 (Iroha Base58)** - מעטפת Base58 שמטביעה את השרשרת
  מפלה. מפענחים מאמתים את הקידומת לפני קידום המטען ל
  הצורה הקנונית.
- **תצוגה דחוסה של סורה** - אלפבית סורה בלבד של **105 סמלים** שנבנה על ידי
  הוספה של השיר イロハ ברוחב חצי (כולל ヰ ו-ヱ) ל-58 התווים
  סט IH58. מחרוזות מתחילות עם הזקיף `sora`, הטמעת Bech32m הנגזרת
  checksum, והשמיט את קידומת הרשת (Sora Nexus משתמע מהזקיף).

  ```
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- ** hex קנוני** - קידוד `0x…` ידידותי לניפוי באגים של הבת הקנוני
  מעטפה.

`AccountAddress::parse_any` מזהה אוטומטית IH58 (מועדף), דחוס (`sora`, השני הכי טוב) או hex קנוני
(`0x...` בלבד; hex חשוף נדחה) מכניס ומחזיר גם את המטען שפוענח וגם את המזוהה
`AccountAddressFormat`. Torii קורא כעת ל-`parse_any` עבור המשלים של ISO 20022
מתייחס ומאחסן את צורת ההקסדה הקנונית כך שמטא נתונים נשארים דטרמיניסטיים
ללא קשר לייצוג המקורי.

#### פריסת בתים של כותרת 2.1 (ADDR-1a)

כל מטען קנוני מונח כ-`header · controller`. ה
`header` הוא בייט בודד שמתקשר אילו כללי מנתח חלים על בתים
עקוב אחר:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

לכן, הבת הראשון אורז את המטא נתונים של הסכימה עבור מפענחים במורד הזרם:

| ביטים | שדה | ערכים מותרים | שגיאה על הפרה |
|------|-------|----------------|--------------------|
| 7-5 | `addr_version` | `0` (v1). הערכים `1-7` שמורים עבור גרסאות עתידיות. | ערכים מחוץ ל-`0-7` מפעילים `AccountAddressError::InvalidHeaderVersion`; יישומים חייבים להתייחס לגרסאות שאינן אפס כלא נתמכות כיום. |
| 4-3 | `addr_class` | `0` = מפתח בודד, `1` = multisig. | ערכים אחרים מעלים `AccountAddressError::UnknownAddressClass`. |
| 2-1 | `norm_version` | `1` (נורמה v1). הערכים `0`, `2`, `3` שמורים. | ערכים מחוץ ל-`0-3` מעלים `AccountAddressError::InvalidNormVersion`. |
| 0 | `ext_flag` | חייב להיות `0`. | Set bit מעלה `AccountAddressError::UnexpectedExtensionFlag`. |

מקודד ה-Rust כותב `0x02` עבור בקרי מפתח יחיד (גרסה 0, מחלקה 0,
norm v1, דגל ההרחבה נוקה) ו-`0x0A` עבור בקרי מולטי-סיג (גרסה 0,
class 1, norm v1, דגל ההרחבה נוקה).

#### קידודי בורר דומיין 2.2 (ADDR-1a)

בורר הדומיין עוקב מיד אחרי הכותרת והוא איחוד מתויג:

| תג | המשמעות | מטען | הערות |
|-----|--------|--------|-------|
| `0x00` | דומיין ברירת מחדל מרומז | אף אחד | תואם את `default_domain_name()` המוגדרים. |
| `0x01` | תקציר דומיין מקומי | 12 בתים | תקציר = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | ערך רישום גלובלי | 4 בתים | Big-endian `registry_id`; שמורות עד שהרישום העולמי יישלח. |

תוויות דומיין עוברות קנוניזציה (UTS-46 + STD3 + NFC) לפני hashing. תגים לא ידועים מעלים `AccountAddressError::UnknownDomainTag`. בעת אימות כתובת מול דומיין, בוררים לא תואמים מעלים `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

הבורר צמוד מיד למטען הבקר, כך שמפענח יכול ללכת
פורמט החוט לפי הסדר: קרא את בייט התג, קרא את המטען הספציפי לתג, ואז המשך הלאה
לבייטים של הבקר.

**דוגמאות בוררות**

- *ברירת מחדל משתמעת* (`tag = 0x00`). אין מטען. hex קנוני לדוגמה עבור ברירת המחדל
  תחום באמצעות מפתח הבדיקה הדטרמיניסטי:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *תקציר מקומי* (`tag = 0x01`). מטען הוא התקציר של 12 בתים. דוגמה (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *רישום גלובלי* (`tag = 0x02`). המטען הוא `registry_id:u32`. הבתים
  העוקבים אחר המטען זהים למקרה ברירת המחדל המשתמע; הבורר פשוט
  מחליף את מחרוזת הדומיין המנורמלת במצביע רישום. דוגמה באמצעות
  `registry_id = 0x0000_002A` (עשרוני 42) ובקר ברירת המחדל הדטרמיניסטי:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  פירוט: `0x02` כותרת, `0x02` תג בורר, `00 00 00 2A` מזהה רישום, `0x00`
  תג בקר, `0x01` מזהה עקומה, `0x20` אורך מפתח, עומס מפתח Ed25519 של 32 בתים.

#### קידודי עומס 2.3 של בקר (ADDR-1a)

מטען הבקר הוא איגוד מתויג נוסף שצורף אחרי בורר הדומיין:

| תג | בקר | פריסה | הערות |
|-----|------------|--------|-------|
| `0x00` | מפתח בודד | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` ממפה ל-Ed25519 היום. `key_len` מוגבל ל-`u8`; ערכים גדולים יותר מעלים `AccountAddressError::KeyPayloadTooLong` (לכן מפתחות ציבוריים ML-DSA בעלי מפתח יחיד, שהם מעל 255 בתים, אינם ניתנים לקידוד וחייבים להשתמש ב-multisig). |
| `0x01` | מולטיסיג | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | תומך בעד 255 חברים (`CONTROLLER_MULTISIG_MEMBER_MAX`). עקומות לא ידועות מעלות את `AccountAddressError::UnknownCurve`; מדיניות שגויה עולה כ-`AccountAddressError::InvalidMultisigPolicy`. |

מדיניות Multisig גם חושפת מפת CBOR בסגנון CTAP2 ועיכול קנוני כך
מארחים ו-SDKs יכולים לאמת את הבקר באופן דטרמיניסטי. ראה
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) עבור הסכימה,
כללי אימות, נוהל גיבוב ואביזרי זהב.

כל בתים של המפתח מקודדים בדיוק כפי שהוחזרו על ידי `PublicKey::to_bytes`; מפענחים משחזרים `PublicKey` מופעים ומעלים `AccountAddressError::InvalidPublicKey` אם הבייטים אינם תואמים לעקומה המוצהרת.

> **אכיפה קנונית של Ed25519 (ADDR-3a):** מפתחות עקומת `0x01` חייבים לפענח למחרוזת הבתים המדויקת שנפלטת על ידי החותם, ואסור להם להיות בתת-הקבוצה מסדר קטן. צמתים דוחים כעת קידודים לא קנוניים (למשל, ערכים מופחתים מודולו `2^255-19`) ונקודות תורפה כגון אלמנט הזהות, כך ש-SDKs צריכים להציג שגיאות אימות תואמות לפני שליחת כתובות.

##### רישום מזהה עקומה 2.3.1 (ADDR-1d)

| מזהה (`curve_id`) | אלגוריתם | שער תכונה | הערות |
|----------------|--------|-------------|-------|
| `0x00` | שמור | — | אסור לפלוט; משטח המפענחים `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | אלגוריתם Canonical v1 (`Algorithm::Ed25519`); מופעל בתצורת ברירת המחדל. |
| `0x02` | ML-DSA (Dilithium3) | — | משתמש בבתים של המפתח הציבורי Dilithium3 (1952 בתים). כתובות עם מפתח יחיד אינן יכולות לקודד ML-DSA מכיוון ש-`key_len` הוא `u8`; multisig משתמש ב-`u16` אורכים. |
| `0x03` | BLS12-381 (רגיל) | `bls` | מפתחות ציבוריים ב-G1 (48 בתים), חתימות ב-G2 (96 בתים). |
| `0x04` | secp256k1 | — | ECDSA דטרמיניסטי על פני SHA-256; מפתחות ציבוריים משתמשים בצורה הדחוסה של 33 בתים SEC1 וחתימות משתמשות בפריסה הקנונית של `r∥s` של 64 בתים. |
| `0x05` | BLS12-381 (קטן) | `bls` | מפתחות ציבוריים ב-G2 (96 בתים), חתימות ב-G1 (48 בתים). |
| `0x0A` | GOST R 34.10-2012 (256, סט A) | `gost` | זמין רק כאשר התכונה `gost` מופעלת. |
| `0x0B` | GOST R 34.10-2012 (256, סט ב') | `gost` | זמין רק כאשר התכונה `gost` מופעלת. |
| `0x0C` | GOST R 34.10-2012 (256, סט C) | `gost` | זמין רק כאשר התכונה `gost` מופעלת. |
| `0x0D` | GOST R 34.10-2012 (512, סט A) | `gost` | זמין רק כאשר התכונה `gost` מופעלת. |
| `0x0E` | GOST R 34.10-2012 (512, סט ב') | `gost` | זמין רק כאשר התכונה `gost` מופעלת. |
| `0x0F` | SM2 | `sm` | אורך DistID (u16 BE) + DistID בתים + מפתח SM2 לא דחוס של 65 בתים SEC1; זמין רק כאשר `sm` מופעל. |

משבצות `0x06–0x09` לא מוקצות עבור עקומות נוספות; מציגים חדש
האלגוריתם דורש עדכון מפת דרכים וכיסוי SDK/מארח תואם. מקודדים
חייב לדחות כל אלגוריתם שאינו נתמך עם `ERR_UNSUPPORTED_ALGORITHM`, וכן
מפענחים חייבים להיכשל במהירות במזהים לא ידועים עם `ERR_UNKNOWN_CURVE` לשימור
התנהגות סגורה כישלון.

הרישום הקנוני (כולל ייצוא JSON קריא במכונה) חי תחת
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
כלי עבודה צריכים לצרוך את מערך הנתונים ישירות כך שמזהי עקומה יישארו
עקבי בין ערכות SDK וזרימות עבודה של מפעילים.

- **שער SDK:** ערכות SDK כברירת מחדל היא אימות/קידוד Ed25519 בלבד. חושף מהיר
  דגלים בזמן הידור (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK דורש
  `AccountAddress.configureCurveSupport(...)`; ה-SDK של JavaScript משתמש
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  תמיכה ב-secp256k1 זמינה אך אינה מופעלת כברירת מחדל ב-JS/Android
  ערכות SDK; המתקשרים חייבים להצטרף באופן מפורש כאשר הם פולטים בקרים שאינם Ed25519.
- **שער מארח:** `Register<Account>` דוחה בקרים שהחתומים שלהם משתמשים באלגוריתמים
  חסרים ברשימת `crypto.allowed_signing` של הצומת **או** מזהי עקומה נעדרים מ
  `crypto.curves.allowed_curve_ids`, לכן אשכולות חייבים לפרסם תמיכה (תצורה +
  genesis) לפני שניתן לרשום בקרי ML-DSA/GOST/SM. בקר BLS
  אלגוריתמים מותרים תמיד בעת הידור (מפתחות קונצנזוס מסתמכים עליהם),
  ותצורת ברירת המחדל מאפשרת Ed25519 + secp256k1.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 הנחיית בקר Multisig

`AccountController::Multisig` מסדרת מדיניות באמצעות
`crates/iroha_data_model/src/account/controller.rs` ואוכף את הסכימה
מתועד ב-[`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md).
פרטי יישום מרכזיים:

- המדיניות מנורמלת ומאומתת על ידי `MultisigPolicy::validate()` לפני כן
  להיות מוטבע. הספים חייבים להיות ≥1 ומשקל ≤Σ; חברים כפולים הם
  הוסר באופן דטרמיניסטי לאחר מיון לפי `(algorithm || 0x00 || key_bytes)`.
- עומס הבקר הבינארי (`ControllerPayload::Multisig`) מקודד
  `version:u8`, `threshold:u16`, `member_count:u8`, ואז כל חבר
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. זה בדיוק מה
  `AccountAddress::canonical_bytes()` כותב למטענים של IH58 (מועדף)/סורה (השני בטובו).
- Hashing (`MultisigPolicy::digest_blake2b256()`) משתמש ב-Blake2b-256 עם ה-
  `iroha-ms-policy` מחרוזת התאמה אישית כך שמניפסטים של ממשל יכולים להיקשר ל-a
  מזהה מדיניות דטרמיניסטית התואם לביטים של הבקר המוטמעים ב-IH58.
- כיסוי רכיבים מתקיים ב-`fixtures/account/address_vectors.json` (מקרים
  `addr-multisig-*`). ארנקים ו-SDKs צריכים להגדיר את המחרוזות הקנוניות של IH58
  למטה כדי לאשר שהמקודדים שלהם מתאימים ליישום Rust.

| מזהה מקרה | סף / חברים | IH58 ליטרלית (קידומת `0x02F1`) | סורה דחוס (`sora`) מילולי | הערות |
|--------|----------------------|--------------------------------|------------------------|------|
| `addr-multisig-council-threshold3` | `≥3` משקל, חברים `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | מניין ממשל בתחום המועצה. |
| `addr-multisig-wonderland-threshold2` | `≥2`, חברים `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | דוגמה לארץ הפלאות עם חתימה כפולה (משקל 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, חברים `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | מניין תחום ברירת מחדל משתמע המשמש לממשל בסיסי.

#### 2.4 כללי כשל (ADDR-1a)

- מטענים קצרים מהכותרת + הבורר הנדרשת או עם בתים שנותרו פולטים `AccountAddressError::InvalidLength` או `AccountAddressError::UnexpectedTrailingBytes`.
- כותרות שמגדירות את `ext_flag` השמורים או מפרסמות גרסאות/מחלקות לא נתמכות חייבות להידחות באמצעות `UnexpectedExtensionFlag`, `InvalidHeaderVersion` או `UnknownAddressClass`.
- תגי בורר/בקר לא ידועים מעלים `UnknownDomainTag` או `UnknownControllerTag`.
- חומר מפתח גדול או פגום מעלה `KeyPayloadTooLong` או `InvalidPublicKey`.
- בקרי Multisig העולה על 255 חברים מעלים `MultisigMemberOverflow`.
- המרות IME/NFKC: ניתן לנרמל את Sora kana ברוחב חצי לצורות ברוחב המלא מבלי לשבור את הפענוח, אך ספרות/אותיות ASCII `sora` ו-IH58 חייבות להישאר ASCII. משטחים של זקיפים ברוחב מלא או מקופל מארז `ERR_MISSING_COMPRESSED_SENTINEL`, עומסי ASCII ברוחב מלא מעלים `ERR_INVALID_COMPRESSED_CHAR`, ואי-ההתאמה של סכום הבדיקה עולה כ-`ERR_CHECKSUM_MISMATCH`. בדיקות נכסים ב-`crates/iroha_data_model/src/account/address.rs` מכסות את הנתיבים האלה כך ש-SDK וארנקים יכולים להסתמך על כשלים דטרמיניסטיים.
- ניתוח Torii ו-SDK של כינויים `address@domain` פולטים כעת את אותם קודים של `ERR_*` כאשר קלטי IH58 (מועדף)/סורה (השני בטוב ביותר) נכשלים לפני החזרה של כינוי (למשל, חוסר התאמה של סכום בדיקה, חוסר התאמה של תקציר תחום), כך שלקוחות יכולים לנסח מחדש הסיבות מובנות.
- עומסי בורר מקומיים קצרים מ-12 בתים משטחים `ERR_LOCAL8_DEPRECATED`, משמרים ניתוק קשה מתקציר Local-8 מדור קודם.
- Domainless IH58 (preferred)/sora (second-best) literals bind directly to the configured default domain label for canonical selector-free payloads. Legacy selector-bearing literals without an explicit `@<domain>` suffix may still fail with `ERR_DOMAIN_SELECTOR_UNRESOLVED` when domain reconstruction is impossible.

#### 2.5 וקטורים בינאריים נורמטיביים

- **דומיין ברירת מחדל מרומז (`default`, byte seed `0x00`)**  
  hex קנוני: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  פירוט: `0x02` כותרת, `0x00` בורר (ברירת מחדל מרומזת), `0x00` תג בקר, `0x01` מזהה עקומה (Ed25519), `0x20` אורך מפתח, ואחריו עומס המפתח של 32 בתים.
- **תקציר דומיין מקומי (`treasury`, byte seed `0x01`)**  
  hex קנוני: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  פירוט: `0x02` כותרת, תג בורר `0x01` בתוספת תקציר `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, ואחריו המטען בעל מפתח יחיד (`0x00` תג, `0x01` מזהה עקומה, `0x20` אורך מפתח, עד 35-byte, עד 35-byte).

בדיקות יחידה (`account::address::tests::parse_any_accepts_all_formats`) קובעות את וקטורי V1 למטה באמצעות `AccountAddress::parse_any`, ומבטיחות שהכלים יכולים להסתמך על המטען הקנוני על פני hex, IH58 (מועדף) ודחוס (`sora`, השני בטוב ביותר). צור מחדש את סט המתקן המורחב עם `cargo run -p iroha_data_model --example address_vectors`.

| דומיין | Seed byte | קנונית hex | דחוס (`sora`) |
|-------------|--------|--------------------------------------------------------------------------------|------------|
| ברירת מחדל | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| אוצר | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| ארץ הפלאות | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| אירוחה | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| אלפא | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| אומגה | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| ממשל | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| מאמתים | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| חוקר | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| סורנט | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| דה | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

נבדק על ידי: Data Model WG, Cryptography WG - היקף מאושר עבור ADDR-1a.

##### כינויים של Sora Nexus

ברירת המחדל של רשתות Sora Nexus היא `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). ה
לכן, `AccountAddress::to_ih58` ו-`to_compressed_sora` עוזרים פולטים
טפסים טקסטואליים עקביים לכל מטען קנוני. מתקנים נבחרים מ
`fixtures/account/address_vectors.json` (נוצר באמצעות
`cargo xtask address-vectors`) מוצגים להלן לעיון מהיר:

| חשבון / בורר | IH58 ליטרל (קידומת `0x02F1`) | סורה דחוס (`sora`) מילולי |
|--------------------|--------------------------------|------------------------|
| `default` דומיין (בורר מרומז, מקור `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (סיומת `@default` אופציונלית בעת מתן רמזי ניתוב מפורשים) |
| `treasury` (בורר עיכול מקומי, זריעה `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| מצביע רישום גלובלי (`registry_id = 0x0000_002A`, שווה ערך ל-`treasury`) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

מחרוזות אלו תואמות את אלו הנפלטות על ידי ה-CLI (`iroha tools address convert`), Torii
תגובות (`address_format=ih58|compressed`), ועוזרי SDK, אז UX העתק/הדבק
זרמים יכולים להסתמך עליהם מילה במילה. הוסף `<address>@<domain>` רק כאשר אתה צריך רמז מפורש לניתוב; הסיומת אינה חלק מהפלט הקנוני.

#### 2.6 כינויים טקסטואליים עבור יכולת פעולה הדדית (מתוכנן)

- **סגנון כינוי שרשרת:** `ih:<chain-alias>:<alias@domain>` עבור יומנים ואדם
  כניסה. על ארנקים לנתח את הקידומת, לאמת את השרשרת המוטבעת ולחסום
  אי התאמה.
- **טופס CAIP-10:** `iroha:<caip-2-id>:<ih58-addr>` לאגנוסטיקה של שרשרת
  אינטגרציות. מיפוי זה **עדיין לא מיושם** במוצר שנשלח
  שרשרת כלים.
- **מסייעי מכונה:** פרסם קודקים עבור Rust, TypeScript/JavaScript, Python,
  ו-Kotlin מכסים IH58 ופורמטים דחוסים (`AccountAddress::to_ih58`,
  `AccountAddress::parse_any`, ומקבילות ה-SDK שלהם). עוזרי CAIP-10 הם
  עבודה עתידית.

#### 2.7 כינוי דטרמיניסטי IH58

- **מיפוי קידומת:** השתמש מחדש ב-`chain_discriminant` בתור קידומת הרשת IH58.
  `encode_ih58_prefix()` (ראה `crates/iroha_data_model/src/account/address.rs`)
  פולט קידומת של 6 סיביות (בייט בודד) עבור ערכים `<64` ו-14 סיביות, שני בתים
  טופס עבור רשתות גדולות יותר. המטלות הסמכותיות חיות
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  ערכות SDK חייבות לשמור על רישום JSON התואם מסונכרן כדי למנוע התנגשויות.
- **חומר החשבון:** IH58 מקודד את המטען הקנוני שנבנה על ידי
  `AccountAddress::canonical_bytes()`—בת כותרת, בורר דומיין ו
  מטען בקר. אין שלב גיבוב נוסף; IH58 מטמיע את
  מטען בקר בינארי (מפתח יחיד או multisig) כפי שמיוצר על ידי ה-Rust
  מקודד, לא מפת CTAP2 המשמשת עבור תקצירי מדיניות multisig.
- **קידוד:** `encode_ih58()` משרשרת את בתים של הקידומת עם הקנוני
  עומס מטען ומוסיף סכום בדיקה של 16 סיביות שנגזר מ-Blake2b-512 עם הערך הקבוע
  קידומת `IH58PRE` (`b"IH58PRE" || prefix || payload`). התוצאה מקודדת Base58 באמצעות `bs58`.
  עוזרי CLI/SDK חושפים את אותו הליך, ו-`AccountAddress::parse_any`
  הופך אותו דרך `decode_ih58`.

#### 2.8 וקטורי בדיקה טקסטואליים נורמטיביים

`fixtures/account/address_vectors.json` מכיל IH58 מלא (מועדף) ודחוס (`sora`, השני הכי טוב)
מילוליות עבור כל מטען קנוני. פַּסִים:

- **`addr-single-default-ed25519` (Sora Nexus, קידומת `0x02F1`).**  
  IH58 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, דחוס (`sora`)
  `sora2QG…U4N5E5`. Torii פולט את המחרוזות המדויקות הללו מ-`AccountId`
  `Display` יישום (IH58 קנוני) ו-`AccountAddress::to_compressed_sora`.
- **`addr-global-registry-002a` (בורר רישום → משרד האוצר).**  
  IH58 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, דחוס (`sora`)
  `sorakX…CM6AEP`. מדגים שבוררי הרישום עדיין מפענחים ל
  אותו מטען קנוני כמו התקציר המקומי המתאים.
- **מקרה כשל (`ih58-prefix-mismatch`).**  
  ניתוח IH58 ליטרלי מקודד עם הקידומת `NETWORK_PREFIX + 1` בצומת
  מצפה לקידומת ברירת המחדל תשואות
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  לפני ניסיון ניתוב דומיין. מתקן `ih58-checksum-mismatch`
  מפעיל זיהוי חבלה על סכום הבדיקה של Blake2b.

#### 2.9 מערכות תאימות

ADDR‑2 מספק חבילת מתקנים שניתן להפעיל מחדש המכסה חיובי ושלילי
תרחישים על פני hex קנוני, IH58 (מועדף), דחוס (`sora`, חצי/רוחב מלא), מרומז
בוררי ברירת מחדל, כינויי רישום גלובליים ובקרי ריבוי חתימות. ה
JSON הקנוני גר ב-`fixtures/account/address_vectors.json` ויכול להיות
התחדש עם:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

עבור ניסויים אד-הוק (נתיבים/פורמטים שונים) הדוגמה הבינארית עדיין
זמין:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

בדיקות יחידת חלודה ב-`crates/iroha_data_model/tests/account_address_vectors.rs`
ו-`crates/iroha_torii/tests/account_address_vectors.rs`, יחד עם ה-JS,
רתמות Swift ו-Android (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
לצרוך את אותו מתקן כדי להבטיח שוויון Codec בין SDKs וכניסה ל-Torii.

### 3. דומיינים ייחודיים ונורמליזציה

ראה גם: [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
עבור הצינור הקנוני Norm v1 המשמש ב-Torii, מודל הנתונים ו-SDKs.

הגדר מחדש את `DomainId` כפול מתויג:

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` עוטף את השם הקיים עבור דומיינים המנוהלים על ידי הרשת הנוכחית.
כאשר דומיין נרשם דרך הרישום הגלובלי, אנו ממשיכים להחזיק
המפלה של הרשת. תצוגה/ניתוח נשארים ללא שינוי לעת עתה, אבל ה
מבנה מורחב מאפשר החלטות ניתוב.

#### 3.1 נורמליזציה והגנות זיוף

Norm v1 מגדיר את הצינור הקנוני שכל רכיב חייב להשתמש בו לפני דומיין
השם נמשך או מוטבע ב-`AccountAddress`. ההדרכה המלאה
גר ב-[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md);
הסיכום שלהלן מתאר את השלבים הקשורים לארנקים, טורי, SDK וממשל
הכלים חייבים ליישם.

1. **אימות קלט.** דחה מחרוזות ריקות, רווח לבן והשמורה
   מפרידים `@`, `#`, `$`. זה תואם את האינווריאנטים שנאכפים על ידי
   `Name::validate_str`.
2. **הרכב Unicode NFC.** החל נורמליזציה של NFC מגובה ICU בצורה קנונית
   רצפים מקבילים קורסים באופן דטרמיניסטי (למשל, `e\u{0301}` → `é`).
3. **נורמליזציה של UTS-46.** הפעל את פלט ה-NFC דרך UTS-46 עם
   `use_std3_ascii_rules = true`, `transitional_processing = false`, ו
   אכיפה באורך DNS מופעלת. התוצאה היא רצף A-label באותיות קטנות;
   תשומות שמפרות את כללי STD3 נכשלות כאן.
4. **מגבלות אורך.** אכפו את הגבולות בסגנון DNS: כל תווית חייבת להיות 1-63
   בתים והדומיין המלא לא יעלה על 255 בתים לאחר שלב 3.
5. **מדיניות מתבלבלת אופציונלית.** מתבצע מעקב אחר בדיקות סקריפטים של UTS-39 עבור
   נורמה v2; מפעילים יכולים להפעיל אותם מוקדם, אך כשלון בבדיקה חייב לבטל
   עיבוד.

אם כל שלב מצליח, מחרוזת A-label באותיות קטנות נשמרת במטמון ומשמשת אותה
קידוד כתובות, תצורה, מניפסטים וחיפושי רישום. עיכול מקומי
הבוררים גוזרים את ערך ה-12 בתים שלהם בתור `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` באמצעות הפלט של שלב 3. כל שאר הניסיונות (מעורבים
קלט רישיות, אותיות גדולות, גולמי Unicode) נדחים עם מובנה
`ParseError`s בגבול שבו צוין השם.

מתקנים קנוניים המדגימים את הכללים האלה - כולל נסיעות הלוך ושוב של Punycode
ורצפי STD3 לא חוקיים - רשומים ב
`docs/source/references/address_norm_v1.md` ומשתקפים ב-SDK CI
חבילות וקטור מעקב תחת ADDR‑2.

### 4. רישום דומיינים של Nexus וניתוב

- **סכימת הרישום:** Nexus שומר על מפה חתומה `DomainName -> ChainRecord`
  כאשר `ChainRecord` כולל מטא נתונים אופציונליים (RPC
  נקודות קצה), והוכחת סמכות (למשל, חתימת ניהול רב).
- **מנגנון סנכרון:**
  - רשתות מגישות תביעות דומיין חתומות ל-Nexus (בין אם במהלך ההתחלה או באמצעות
    הוראת ממשל).
  - Nexus מפרסם מניפסטים תקופתיים (חתום JSON בתוספת שורש Merkle אופציונלי)
    באמצעות HTTPS ואחסון בכתובת תוכן (למשל, IPFS). לקוחות מצמידים את
    המניפסט העדכני ביותר ואימות חתימות.
- **זרימת חיפוש:**
  - Torii מקבל עסקה המתייחסת `DomainId`.
  - אם הדומיין אינו ידוע מקומית, Torii מבצעת שאילתות במניפסט ה-Nexus השמור.
  - אם המניפסט מצביע על שרשרת זרה, העסקה נדחית עם
    שגיאה דטרמיניסטית `ForeignDomain` ופרטי השרשרת המרוחקת.
  - אם הדומיין חסר ב-Nexus, Torii מחזירה `UnknownDomain`.
- **סמוך על עוגנים וסיבוב:** מניפסטים של סימני מפתחות ממשל; סיבוב או
  הביטול מתפרסם כערך מניפסט חדש. לקוחות אוכפים מניפסט
  TTLs (למשל, 24 שעות) ומסרבים לעיין בנתונים מיושנים מעבר לחלון זה.
- **מצבי תקלה:** אם אחזור המניפסט נכשל, Torii חוזר למטמון
  נתונים בתוך TTL; לאחר TTL הוא פולט `RegistryUnavailable` ומסרב
  ניתוב בין דומיינים כדי למנוע מצב לא עקבי.

### 4.1 אי-שינוי ברישום, כינויים ואבני מצבות (ADDR-7c)

Nexus מפרסם **מניפסט להוספה בלבד** כך שכל הקצאת דומיין או כינוי
ניתן לביקורת ולהפעיל מחדש. על המפעילים לטפל בחבילה המתוארת ב
[address manifest runbook](source/runbooks/address_manifest_ops.md) בתור
מקור האמת היחיד: אם מניפסט חסר או נכשל באימות, Torii חייב
לסרב לפתור את הדומיין המושפע.

תמיכה באוטומציה: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
משמיע מחדש את הבדיקות של סכום הבדיקה, הסכימה והתקציר הקודם שצוינו ב-
ספר ריצה. כלול את פלט הפקודה בכרטיסי שינוי כדי להציג את `sequence`
והקישור `previous_digest` אומת לפני פרסום החבילה.

#### כותרת מניפסט וחוזה חתימה

| שדה | דרישה |
|-------|-------------|
| `version` | כרגע `1`. Bump רק עם עדכון מפרט תואם. |
| `sequence` | הגדל ב-**בדיוק** אחד לכל פרסום. מטמוני Torii מסרבים לתיקונים עם פערים או רגרסיות. |
| `generated_ms` + `ttl_hours` | קבע את טריות המטמון (ברירת מחדל 24 שעות). אם ה-TTL יפוג לפני הפרסום הבא, Torii יתהפך ל-`RegistryUnavailable`. |
| `previous_digest` | BLAKE3 עיכול (hex) של הגוף המניפסט הקודם. המאמתים מחשבים אותו מחדש עם `b3sum` כדי להוכיח חוסר שינוי. |
| `signatures` | המניפסטים נחתמים דרך Sigstore (`cosign sign-blob`). Ops חייב להריץ `cosign verify-blob --bundle manifest.sigstore manifest.json` ולאכוף את אילוצי זהות הממשל/המנפיק לפני ההשקה. |

האוטומציה של השחרור פולטת `manifest.sigstore` ו-`checksums.sha256`
לצד גוף JSON. שמור את הקבצים יחד בעת שיקוף ל- SoraFS או
נקודות קצה של HTTP כדי שמבקרים יוכלו להפעיל מחדש את שלבי האימות מילה במילה.

#### סוגי ערכים

| הקלד | מטרה | שדות חובה |
|------|--------|----------------|
| `global_domain` | מצהיר שדומיין רשום ברחבי העולם ועליו למפות לאבחון שרשרת וקידומת IH58. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` | מוציא לפועל כינוי/בורר לצמיתות. נדרש בעת מחיקת תקציר Local-8 או הסרת דומיין. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` ערכים עשויים לכלול אופציונלי `manifest_url` או `sorafs_cid`
להפנות ארנקים למטא נתונים של שרשרת חתומה, אבל ה-tuple הקנוני נשאר
`{domain, chain, discriminant/ih58_prefix}`. `tombstone` רשומות **חייבים** לצטט
הבורר שיוצא לפנסיה וחפץ הכרטיס/הממשל שאישר
השינוי כך שמסלול הביקורת ניתן לשחזור במצב לא מקוון.

#### כינוי/זרימת עבודה וטלמטריה של מצבות

1. **זהה סחיפה.** השתמש ב-`torii_address_local8_total{endpoint}`,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, ו
   `torii_address_invalid_total{endpoint,reason}` (עיבוד ב
   `dashboards/grafana/address_ingest.json`) כדי לאשר הגשות מקומיות ו
   התנגשויות מקומיות-12 נשארות באפס לפני שמציעות מצבה. ה
   מונים לדומיין מאפשרים לבעלים להוכיח שרק דומיינים של מפתחים/בדיקות פולטים Local-8
   תעבורה (וההתנגשויות Local-12 ממפות לדומיינים ידועים ב-Staging) תוך
   כולל את הפאנל **Domain Kind Mix (5m)** כך ש-SREs יכולים לתאר את הכמות
   נותרה `domain_kind="local12"` תנועה, וה-`AddressLocal12Traffic`
   התראה על שריפות בכל פעם שההפקה עדיין רואה בוררים מקומיים-12 למרות ה
   שער פרישה.
2. **הפקת תקצירים קנוניים.** הפעל
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (או לצרוך `fixtures/account/address_vectors.json` באמצעות
   `scripts/account_fixture_helper.py`) כדי ללכוד את `digest_hex` המדויק.
   ה-CLI מקבל את IH58, `sora…` ו-`0x…` מילוליות קנוניות; לצרף
   `@<domain>` רק כאשר אתה צריך לשמור תווית למניפסטים.
   סיכום ה-JSON מציג את הדומיין הזה דרך השדה `input_domain`, וכן
   `legacy  suffix` מפעיל מחדש את הקידוד שהומר כ-`<address>@<domain>` עבור
   Manifest diffs (סיומת זו היא מטא נתונים, לא מזהה חשבון קנוני).
   עבור ייצוא מכוון קו חדש השתמש
   `iroha tools address normalize --input <file> legacy-selector input mode` להמרה המוני מקומית
   בוררים לצורות IH58 קנוניות (מועדף), דחוסות (`sora`, השני הכי טוב), hex, או JSON בזמן דילוג
   שורות לא מקומיות. כאשר מבקרים צריכים ראיות ידידותיות לגיליון אלקטרוני, רץ
   `iroha tools address audit --input <file> --format csv` לשליחת סיכום CSV
   (`input,status,format,domain_kind,…`) שמדגיש בוררים מקומיים,
   קידודים קנוניים, וכישלונות ניתוח באותו קובץ.
3. **צרף רשומות מניפסט.** טיוטה של הרשומה `tombstone` (ואת המעקב
   `global_domain` תיעוד בעת ההגירה לרישום הגלובלי) ואמת
   המניפסט עם `cargo xtask address-vectors` לפני בקשת חתימות.
4. **אמת ופרסם.** עקוב אחר רשימת ה-runbook (hashes, Sigstore,
   מונוטוניות ברצף) לפני שיקוף החבילה ל- SoraFS. טורי עכשיו
   מקנוניזציה ל-IH58 (מועדף)/סורה (השני בטובו) מילולית מיד לאחר נחיתת החבילה.
5. **ניטור והחזרה לאחור.** השאר את לוחות ההתנגשות Local-8 ו-Local-12 במיקום
   אפס למשך 30 ימים; אם מופיעות רגרסיות, פרסם מחדש את המניפסט הקודם
   רק בסביבה הלא-ייצור המושפעת עד שהטלמטריה מתייצבת.

כל השלבים שלמעלה הם ראיות חובה עבור ADDR-7c: מניפסטים ללא
חבילת החתימה `cosign` או ללא ערכי `previous_digest` תואמים חייבים
להידחות אוטומטית, והמפעילים חייבים לצרף את יומני האימות
כרטיסי החלפה שלהם.

### 5. ארגונומיה של ארנק ו-API

- **ברירות מחדל לתצוגה:** ארנקים מציגים את כתובת IH58 (קצר, סיכום בדיקה)
  בתוספת הדומיין שנפתר כתווית שנלקחה מהרישום. דומיינים הם
  מסומן בבירור כמטא נתונים תיאוריים שעשויים להשתנות, בעוד IH58 הוא
  כתובת יציבה.
- **קנוניזציה של קלט:** Torii ו-SDKs מקבלים IH58 (מועדף)/sora (שני הכי טוב)/0x
  כתובות בתוספת `alias@domain`, `public_key@domain`, `uaid:…`, ו
  `opaque:…` טפסים, ואז קנוניזציה ל-IH58 לפלט. אין
  החלפת מצב קפדנית; יש לשמור על מזהי טלפון/אימייל גולמיים מחוץ לפנקס החשבונות
  באמצעות UAID/מיפויים אטומים.
- **מניעת שגיאות:** ארנקים מנתחים קידומות IH58 ואוכפים אבחון שרשרת
  ציפיות. אי התאמה של שרשרת מעוררת כשלים קשים עם אבחון בר-פעולה.
- **ספריות Codec:** רסט רשמית, TypeScript/JavaScript, Python ו-Kotlin
  ספריות מספקות קידוד/פענוח IH58 בתוספת תמיכה דחוסה (`sora`) ל
  להימנע מיישומים מקוטעים. המרות CAIP-10 לא נשלחות עדיין.

#### הנחיית נגישות ושיתוף בטוח

- מעקב אחר הנחיות יישום עבור משטחי מוצר נמצא בזמן אמת
  `docs/portal/docs/reference/address-safety.md`; עיין ברשימת הבדיקה הזו מתי
  התאמת דרישות אלה לארנק או ל-Explorer UX.
- **זרימות שיתוף בטוחות:** משטחים שמעתיקים או מציגים כתובות כברירת מחדל לטופס IH58 וחושפים פעולת "שיתוף" סמוכה המציגה גם את המחרוזת המלאה וגם קוד QR שנגזר מאותו מטען, כך שמשתמשים יכולים לאמת את סכום הבדיקה באופן ויזואלי או על ידי סריקה. כאשר חיתוך בלתי נמנע (למשל, מסכים קטנים), שמור על ההתחלה והסוף של המחרוזת, הוסף אליפסות ברורות ושמור את הכתובת המלאה נגישה באמצעות העתקה ללוח כדי למנוע גזירה בשוגג.
- **הגנת IME:** כניסות כתובות חייבות לדחות פריטי קומפוזיציה ממקלדות בסגנון IME/IME. אכוף כניסת ASCII בלבד, הציגו אזהרה מוטבעת כאשר מזוהים תווים ברוחב מלא או קאנה, והצעו אזור הדבקה של טקסט רגיל שמסיר סימני שילוב לפני אימות כך שמשתמשים יפנים וסיניים יוכלו להשבית את ה-IME שלהם מבלי לאבד התקדמות.
- **תמיכה בקורא מסך:** ספק תוויות נסתרות ויזואלית (`aria-label`/`aria-describedby`) המתארות את ספרות הקידומת המובילות של Base58 ומחלקות את מטען ה-IH58 לקבוצות של 4 או 8 תווים, כך שטכנולוגיה מסייעת קוראת תווים מקובצים במקום רצף של תווים. הכריזו על הצלחה בהעתקה/שיתוף באמצעות אזורים חיים מנומסים והבטיחו שתצוגות QR מקדימות כוללות טקסט חלופי תיאורי ("כתובת IH58 עבור <כינוי> בשרשרת 0x02F1").
- **שימוש דחוס בסורה בלבד:** תמיד תייג את התצוגה הדחוסה `sora…` כ"סורה בלבד" והצמד אותה מאחורי אישור מפורש לפני ההעתקה. ערכות SDK וארנקים חייבים לסרב להציג פלט דחוס כאשר מאבחנת השרשרת אינה הערך של Sora Nexus ועליהם להפנות את המשתמשים חזרה ל-IH58 לצורך העברות בין רשתות כדי למנוע ניתוב שגוי של כספים.

## רשימת רשימת יישום

- **מעטפת IH58:** הקידומת מקודדת את `chain_discriminant` באמצעות הקומפקטית
  סכימת 6-/14 סיביות מ-`encode_ih58_prefix()`, הגוף הוא הבתים הקנוניים
  (`AccountAddress::canonical_bytes()`), וסכום הבדיקה הוא שני הבייטים הראשונים
  של Blake2b-512(`b"IH58PRE"` || קידומת || גוף). המטען המלא הוא Base58-
  מקודד באמצעות `bs58`.
- **חוזה רישום:** פרסום JSON חתום (ואופציונלי Merkle root).
  `{discriminant, ih58_prefix, chain_alias, endpoints}` עם 24 שעות TTL ו
  מקשי סיבוב.
- **מדיניות דומיינים:** ASCII `Name` היום; אם מפעילים את i18n, החל UTS-46 עבור
  נורמליזציה ו-UTS-39 עבור בדיקות מתבלבלות. אכיפת תווית מקסימום (63) ו
  סה"כ (255) אורכים.
- **עוזרים טקסטואליים:** משלוח IH58 ↔ דחוס (`sora…`) קודקים ב-Rust,
  TypeScript/JavaScript, Python ו-Kotlin עם וקטורי בדיקה משותפים (CAIP-10
  המיפויים נשארים עבודה עתידית).
- **כלי CLI:** ספק זרימת עבודה דטרמיניסטית של מפעיל באמצעות `iroha tools address convert`
  (ראה `crates/iroha_cli/src/address.rs`), המקבל IH58/`sora…`/`0x…` מילוליות ו
  תוויות `<address>@<domain>` אופציונליות, ברירת המחדל היא פלט IH58 באמצעות הקידומת Sora Nexus (`753`),
  ופולטת את האלפבית הדחוס של Sora רק כאשר המפעילים מבקשים זאת במפורש
  `--format compressed` או מצב סיכום JSON. הפקודה אוכפת את ציפיות הקידומת על
  ניתוח, מתעד את הדומיין שסופק (`input_domain` ב-JSON), ואת הדגל של `legacy  suffix`
  מפעיל מחדש את הקידוד שהומר כ-`<address>@<domain>` כך שההבדלים המניפסטים יישארו ארגונומיים.
- **UX של ארנק/אקספלורר:** עקוב אחר [הנחיות הצגת הכתובות](source/sns/address_display_guidelines.md)
  נשלח עם ADDR-6 - מציע כפתורי העתקה כפולים, שמור את IH58 כמטען QR והזהיר
  משתמשים שהטופס הדחוס `sora…` הוא Sora בלבד ורגיש לשכתובים מחדש של IME.
- **שילוב Torii:** Cache Nexus מתבטא בכבוד TTL, emit
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` באופן דטרמיניסטי, וכן
  keep account-literal parsing encoded-only (`IH58` preferred, `sora…`
  compressed accepted) with canonical IH58 output.

### פורמטי תגובה של Torii

- `GET /v1/accounts` מקבל פרמטר שאילתה אופציונלי `address_format` ו
  `POST /v1/accounts/query` מקבל את אותו שדה בתוך מעטפת ה-JSON.
  הערכים הנתמכים הם:
  - `ih58` (ברירת מחדל) - תגובות פולטות עומסי IH58 Base58 קנוניים (למשל,
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `compressed` — תגובות פולטות את התצוגה הדחוסה של סורה בלבד בזמן
    שמירה על פרמטרי מסננים/נתיבים קנוניים.
- ערכים לא חוקיים מחזירים `400` (`QueryExecutionFail::Conversion`). זה מאפשר
  ארנקים וחוקרים לבקש מחרוזות דחוסות עבור סורה בלבד UX תוך
  שמירה על IH58 כברירת המחדל הדדית.
- רישומי בעלי נכסים (`GET /v1/assets/{definition_id}/holders`) וה-JSON שלהם
  מקבילה למעטפה (`POST …/holders/query`) מכבדת גם את `address_format`.
  השדה `items[*].account_id` פולט ליטרלים דחוסים בכל פעם ש
  שדה פרמטר/מעטפה מוגדר ל-`compressed`, שיקוף את החשבונות
  נקודות קצה כדי שחוקרים יוכלו להציג פלט עקבי בין ספריות.
- **בדיקה:** הוסף בדיקות יחידה עבור מקודדים/מפענחים הלוך ושוב, שרשרת לא נכונה
  כשלים, וחיפושים גלויים; הוסף כיסוי אינטגרציה ב- Torii ו-SDKs
  עבור IH58 זורם מקצה לקצה.

## רישום קוד שגיאה

מקודדי כתובות ומפענחים חושפים כשלים דרך
`AccountAddressError::code_str()`. הטבלאות הבאות מספקות את הקודים היציבים
ש-SDKs, ארנקים ומשטחי Torii צריכים לצוץ לצד הניתנים לקריאה על ידי אדם
הודעות, בתוספת הדרכה מומלצת לתיקון.

### בנייה קנונית

| קוד | כישלון | תיקון מומלץ |
|------|--------|------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | המקודד קיבל אלגוריתם חתימה שאינו נתמך על ידי תכונות הרישום או ה-build. | הגבל את בניית החשבון לעקומות המופעלות ברישום ובתצורה. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | אורך עומס המפתח לחתימה חורג מהמגבלה הנתמכת. | בקרי מפתח בודד מוגבלים לאורכים של `u8`; השתמש ב-multisig עבור מפתחות ציבוריים גדולים (למשל, ML-DSA). |
| `ERR_INVALID_HEADER_VERSION` | גרסת כותרת הכתובת נמצאת מחוץ לטווח הנתמך. | פלט גרסת כותרת `0` עבור כתובות V1; שדרג את המקודדים לפני אימוץ גרסאות חדשות. |
| `ERR_INVALID_NORM_VERSION` | דגל גרסת נורמליזציה אינו מזוהה. | השתמש בגרסת נורמליזציה `1` והימנע מהחלפת ביטים שמורים. |
| `ERR_INVALID_IH58_PREFIX` | לא ניתן לקודד את קידומת הרשת המבוקשת IH58. | בחר קידומת בטווח הכולל `0..=16383` שפורסם ברישום השרשרת. |
| `ERR_CANONICAL_HASH_FAILURE` | גיבוב מטען קנוני נכשל. | נסה שוב את הפעולה; אם השגיאה נמשכת, התייחס אליה כאל באג פנימי בערימת הגיבוב. |

### פענוח פורמט וזיהוי אוטומטי

| קוד | כישלון | תיקון מומלץ |
|------|--------|------------------------|
| `ERR_INVALID_IH58_ENCODING` | מחרוזת IH58 מכילה תווים מחוץ לאלפבית. | ודא שהכתובת משתמשת באלפבית IH58 שפורסם ושלא נקטעה במהלך העתקה/הדבקה. |
| `ERR_INVALID_LENGTH` | אורך המטען אינו תואם לגודל הקנוני הצפוי עבור הבורר/בקר. | ספק את המטען הקנוני המלא עבור בורר הדומיין ופריסת הבקר שנבחרו. |
| `ERR_CHECKSUM_MISMATCH` | אימות סכום הבדיקה של IH58 (מועדף) או דחוס (`sora`, השני הטוב ביותר) נכשל. | צור מחדש את הכתובת ממקור מהימן; זה בדרך כלל מצביע על שגיאת העתקה/הדבקה. |
| `ERR_INVALID_IH58_PREFIX_ENCODING` | בתים של קידומת IH58 שגויים. | מקודד מחדש את הכתובת באמצעות מקודד תואם; אל תשנה את הבתים המובילים של Base58 באופן ידני. |
| `ERR_INVALID_HEX_ADDRESS` | צורה הקסדצימלית קנונית לא הצליחה לפענח. | ספק מחרוזת hex עם קידומת `0x` באורך שווה שהופק על ידי המקודד הרשמי. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | טופס דחוס לא מתחיל ב-`sora`. | הקידומת של כתובות סורה דחוסות עם הזקיף הנדרש לפני מסירתן למפענחים. |
| `ERR_COMPRESSED_TOO_SHORT` | למחרוזת דחוסה אין מספיק ספרות עבור מטען וסכום ביקורת. | השתמש במחרוזת הדחוסה המלאה שנפלטת על ידי המקודד במקום בקטעים קטועים. |
| `ERR_INVALID_COMPRESSED_CHAR` | דמות מחוץ לאלפבית הדחוס נתקל. | החלף את התו בגליף Base-105 חוקי מהטבלאות שפורסמו ברוחב חצי/רוחב מלא. |
| `ERR_INVALID_COMPRESSED_BASE` | המקודד ניסה להשתמש ברדיוס לא נתמך. | הגיש באג נגד המקודד; האלפבית הדחוס מקובע ל-radix 105 ב-V1. |
| `ERR_INVALID_COMPRESSED_DIGIT` | ערך הספרה חורג מגודל האלפבית הדחוס. | ודא שכל ספרה נמצאת בתוך `0..105)`, צור מחדש את הכתובת במידת הצורך. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | הזיהוי האוטומטי לא הצליח לזהות את פורמט הקלט. | ספק מחרוזות hex IH58 (מועדף), דחוסות (`sora`) או `0x` קנוניות בעת הפעלת מנתחים. |

### אימות דומיין ורשת

| קוד | כישלון | תיקון מומלץ |
|------|--------|------------------------|
| `ERR_DOMAIN_MISMATCH` | בורר הדומיין אינו תואם לדומיין הצפוי. | השתמש בכתובת שהונפקה עבור הדומיין המיועד או עדכן את הציפייה. |
| `ERR_INVALID_DOMAIN_LABEL` | תווית הדומיין נכשלה בבדיקות הנורמליזציה. | קנוניזציה של הדומיין באמצעות UTS-46 עיבוד לא מעברי לפני הקידוד. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | קידומת הרשת המפוענחת של IH58 שונה מהערך המוגדר. | עבור לכתובת משרשרת היעד או התאם את המבחין/הקידומת הצפוי. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | סיביות מחלקות כתובות אינן מזוהות. | שדרג את המפענח לגרסה שמבינה את המחלקה החדשה, או הימנע משיבוש בסיביות הכותרת. |
| `ERR_UNKNOWN_DOMAIN_TAG` | תג בורר הדומיין אינו ידוע. | עדכן למהדורה התומכת בסוג הבורר החדש, או הימנע משימוש במטענים ניסיוניים בצמתי V1. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | ביט הרחבה שמור הוגדר. | נקה ביטים שמורים; הם נשארים סגורים עד ש-ABI עתידי יציג אותם. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | תג מטען בקר לא מזוהה. | שדרג את המפענח כדי לזהות סוגי בקרים חדשים לפני ניתוחם. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | מטען קנוני הכיל בתים נגררים לאחר פענוח. | צור מחדש את המטען הקנוני; רק האורך המתועד צריך להיות נוכח. |

### אימות מטען בקר

| קוד | כישלון | תיקון מומלץ |
|------|--------|------------------------|
| `ERR_INVALID_PUBLIC_KEY` | בתים של מפתח אינם תואמים את העקומה המוצהרת. | ודא שבתי המפתח מקודדים בדיוק כנדרש עבור העקומה שנבחרה (למשל, 32 בתים Ed25519). |
| `ERR_UNKNOWN_CURVE` | מזהה עקומה אינו רשום. | השתמש במזהה עקומה `1` (Ed25519) עד לאישור ופרסום של עקומות נוספות ברישום. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | בקר Multisig מצהיר על יותר חברים מאשר נתמכים. | צמצם את החברות ב-multisig למגבלה המתועדת לפני הקידוד. |
| `ERR_INVALID_MULTISIG_POLICY` | אימות מדיניות Multisig נכשל (סף/משקלים/סכימה). | בנה מחדש את המדיניות כך שתעמוד בסכימת CTAP2, גבולות המשקל ומגבלות הסף. |

## נשקלו חלופות

- **Pure Base58Check (בסגנון ביטקוין).** סכום בדיקה פשוט יותר אך זיהוי שגיאות חלש יותר
  מאשר סכום הבדיקה של IH58 שמקורו ב-Blake2b (`encode_ih58` מקצץ hash של 512 סיביות)
  וחסרה סמנטיקה של קידומת מפורשת עבור מבחנים של 16 סיביות.
- **הטמעת שם שרשרת במחרוזת הדומיין (לדוגמה, `finance@chain`).** הפסקות
- **סמוך אך ורק על ניתוב Nexus מבלי לשנות כתובות.** המשתמשים עדיין יעשו זאת
  העתק/הדבק מחרוזות דו-משמעיות; אנחנו רוצים שהכתובת עצמה תישא הקשר.
- **מעטפת Bech32m.** ידידותית ל-QR ומציעה קידומת קריאת אדם, אך
  יהיה שונה מהטמעת המשלוח IH58 (`AccountAddress::to_ih58`)
  ודורשים ליצור מחדש את כל המתקנים/ערכות ה-SDK. מפת הדרכים הנוכחית שומרת על IH58 +
  תמיכה דחוסה (`sora`) תוך המשך מחקר לעתיד
  שכבות Bech32m/QR (מיפוי CAIP-10 נדחה).

## שאלות פתוחות

- אשר ש-`u16` מפלים בתוספת טווחים שמורים מכסים ביקוש לטווח ארוך;
  אחרת, הערך `u32` עם קידוד וריאנט.
- סיים את תהליך הניהול של ריבוי חתימות עבור עדכוני רישום וכיצד
  ביטולים/הקצאות שפג תוקפן מטופלות.
- הגדר את סכימת חתימת המניפסט המדויקת (למשל, Ed25519 multi-sig) ו
  אבטחת תחבורה (הצמדה של HTTPS, פורמט גיבוב IPFS) עבור הפצת Nexus.
- קבע אם לתמוך בכינויי דומיין/הפניות מחדש עבור העברות וכיצד
  להציף אותם מבלי לשבור את הדטרמיניזם.
- ציין כיצד חוזי Kotodama/IVM לגשת לעוזרים IH58 (`to_address()`,
  `parse_address()`) והאם אחסון בשרשרת אמור אי פעם לחשוף את CAIP-10
  מיפויים (היום IH58 הוא קנוני).
- חקור את רישום רשתות Iroha ברישום חיצוני (למשל, רישום IH58,
  ספריית מרחב השמות של CAIP) ליישור מערכת אקולוגית רחבה יותר.

## השלבים הבאים

1. קידוד IH58 נחת ב-`iroha_data_model` (`AccountAddress::to_ih58`,
   `parse_any`); המשך להעביר מתקנים/בדיקות לכל SDK ולנקות כל
   מצייני מיקום של Bech32m.
2. הרחב את סכימת התצורה עם `chain_discriminant` והפקה הגיונית
  ברירת מחדל עבור הגדרות בדיקה/פיתוח קיימות. **(בוצע: `common.chain_discriminant`
  כעת נשלח ב-`iroha_config`, ברירת המחדל היא `0x02F1` עם פר רשת
  עוקף.)**
3. נסח את סכימת הרישום של Nexus ואת מפרסם המניפסט של הוכחת מושג.
4. אסוף משוב מספקי ארנקים ואפוטרופוסים על היבטים של גורם אנושי
   (שמות HRP, עיצוב תצוגה).
5. עדכן את התיעוד (`docs/source/data_model.md`, מסמכי Torii API) לאחר
   נתיב היישום מחויב.
6. משלוח ספריות קודקים רשמיות (Rust/TS/Python/Kotlin) עם מבחן נורמטיבי
   וקטורים המכסים מקרי הצלחה וכישלון.
