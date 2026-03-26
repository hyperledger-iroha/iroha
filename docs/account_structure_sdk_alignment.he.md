# הערת פריסה של I105 לבעלי SDK וקודקים

צוותים: SDK ‏Rust, SDK ‏TypeScript/JavaScript, SDK ‏Python, SDK ‏Kotlin, כלי קודקים

הקשר: `docs/account_structure.md` משקף כעת את מימוש מזהה החשבון I105 שבייצור.
אנא יישרו את התנהגות ה‑SDK והבדיקות למפרט הקנוני.

הפניות מרכזיות:
- קודק כתובות + פריסת כותרת — `docs/account_structure.md` §2
- רישום עקומות — `docs/source/references/address_curve_registry.md`
- טיפול בדומיין Norm v1 — `docs/source/references/address_norm_v1.md`
- וקטורי fixtures — `fixtures/account/address_vectors.json`

משימות:
1. **פלט קנוני:** `AccountId::to_string()`/Display חייב להפיק I105 בלבד
   (ללא סיומת `@domain`). ה‑hex הקנוני מיועד לדיבוג (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical i105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **Checksum של I105:** השתמשו ב‑Blake2b‑512 על `I105PRE || prefix || payload`,
   וקחו את שני הבייטים הראשונים. בסיס האלפבית הדחוס הוא **105**.
5. **גייטינג עקומות:** ברירת המחדל ב‑SDK היא Ed25519 בלבד. ספקו opt‑in מפורש
   ל‑ML‑DSA/GOST/SM (דגלי build ב‑Swift; `configureCurveSupport` ב‑JS/Android).
   אל תניחו ש‑secp256k1 מופעל כברירת מחדל מחוץ ל‑Rust.
6. **ללא CAIP‑10:** עדיין אין מיפוי CAIP‑10 משוחרר; אל תחשפו או תסתמכו על
   המרות CAIP‑10.

אנא אשרו לאחר עדכון הקודקים/הבדיקות; שאלות פתוחות ניתן לעקוב אחריהן בשרשור RFC של כתובות חשבון.
