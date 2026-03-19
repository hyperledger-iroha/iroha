<!-- Hebrew translation of docs/source/samples/find_active_abi_versions.md -->

---
lang: he
direction: rtl
source: docs/source/samples/find_active_abi_versions.md
status: complete
translator: manual
---

<div dir="rtl">

# ‏FindAbiVersion — סכמת שאילתת Norito (דוגמה)

שם טיפוס: `iroha_data_model::query::runtime::AbiVersion`

שדות
- `abi_version: u16` — the fixed ABI version currently accepted on this node.

תגובה לדוגמה ב־Norito JSON (מהדורה ראשונה; ABI יחיד)
```json
{
  "abi_version": 1
}
```

שאילתה (חתומה) — `FindAbiVersion`
- מסלול: ‏`/query` (מסגרת `SignedQuery` ב־Norito)
- וריאנט תיבת השאילתה הסינגולרית: `FindAbiVersion`
- וריאנט הפלט: `AbiVersion`

הערות
- במהדורה הראשונה גרסת ABI ‎1‎ פעילה תמיד. גרסאות שמופעלות באמצעות מנגנון הממשל מתווספות לקבוצה לצמיתות. הדוגמה לעיל מייצגת מצב שבו קיימת גרסה יחידה.

</div>
