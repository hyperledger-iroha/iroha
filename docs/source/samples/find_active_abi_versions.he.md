<!-- Hebrew translation of docs/source/samples/find_active_abi_versions.md -->

---
lang: he
direction: rtl
source: docs/source/samples/find_active_abi_versions.md
status: complete
translator: manual
---

<div dir="rtl">

# ‏FindActiveAbiVersions — סכמת שאילתת Norito (דוגמה)

שם טיפוס: `iroha_data_model::query::runtime::ActiveAbiVersions`

שדות
- `active_versions: Vec<u16>` — רשימה ממוינת של גרסאות ABI הפעילות בצומת הנוכחי.
- `default_compile_target: u16` — הגרסה הגבוהה ביותר מבין הפעילות; קומפיילרים צריכים לייעד אותה כברירת המחדל.

תגובה לדוגמה ב־Norito JSON (מהדורה ראשונה; ABI יחיד)
```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```

שאילתה (חתומה) — `FindActiveAbiVersions`
- מסלול: ‏`/query` (מסגרת `SignedQuery` ב־Norito)
- וריאנט תיבת השאילתה הסינגולרית: `FindActiveAbiVersions`
- וריאנט הפלט: `ActiveAbiVersions`

הערות
- במהדורה הראשונה גרסת ABI ‎1‎ פעילה תמיד. גרסאות שמופעלות באמצעות מנגנון הממשל מתווספות לקבוצה לצמיתות. הדוגמה לעיל מייצגת מצב שבו קיימת גרסה יחידה.

</div>
