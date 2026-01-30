# הערת פריסה של IH58 לבעלי SDK וקודקים

צוותים: SDK ‏Rust, SDK ‏TypeScript/JavaScript, SDK ‏Python, SDK ‏Kotlin, כלי קודקים

הקשר: `docs/account_structure.md` משקף כעת את מימוש מזהה החשבון IH58 שבייצור.
אנא יישרו את התנהגות ה‑SDK והבדיקות למפרט הקנוני.

הפניות מרכזיות:
- קודק כתובות + פריסת כותרת — `docs/account_structure.md` §2
- רישום עקומות — `docs/source/references/address_curve_registry.md`
- טיפול בדומיין Norm v1 — `docs/source/references/address_norm_v1.md`
- וקטורי fixtures — `fixtures/account/address_vectors.json`

משימות:
1. **פלט קנוני:** `AccountId::to_string()`/Display חייב להפיק IH58 בלבד
   (ללא סיומת `@domain`). ה‑hex הקנוני מיועד לדיבוג (`0x...`).
2. **קלטים נתמכים:** מפרשים חייבים לקבל IH58 (מועדף), `sora` דחוס, ו‑hex קנוני
   (רק `0x...`; hex ללא תחילית נדחה). הקלטים יכולים לכלול סיומת `@<domain>`
   לרמזי ניתוב; כינויים `<label>@<domain>` דורשים resolver. `public_key@domain`
   (multihash hex) נשאר נתמך.
3. **Resolvers:** פיענוח IH58/sora ללא דומיין דורש resolver לבוחר דומיין אלא אם
   הסלקטור הוא ברירת מחדל מובלעת (השתמשו בתווית הדומיין ברירת המחדל המוגדרת).
   ליטרלים UAID (`uaid:...`) ו‑opaque (`opaque:...`) דורשים resolvers.
4. **Checksum של IH58:** השתמשו ב‑Blake2b‑512 על `IH58PRE || prefix || payload`,
   וקחו את שני הבייטים הראשונים. בסיס האלפבית הדחוס הוא **105**.
5. **גייטינג עקומות:** ברירת המחדל ב‑SDK היא Ed25519 בלבד. ספקו opt‑in מפורש
   ל‑ML‑DSA/GOST/SM (דגלי build ב‑Swift; `configureCurveSupport` ב‑JS/Android).
   אל תניחו ש‑secp256k1 מופעל כברירת מחדל מחוץ ל‑Rust.
6. **ללא CAIP‑10:** עדיין אין מיפוי CAIP‑10 משוחרר; אל תחשפו או תסתמכו על
   המרות CAIP‑10.

אנא אשרו לאחר עדכון הקודקים/הבדיקות; שאלות פתוחות ניתן לעקוב אחריהן בשרשור RFC של כתובות חשבון.
