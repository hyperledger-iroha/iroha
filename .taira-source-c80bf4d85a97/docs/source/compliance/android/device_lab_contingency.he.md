---
lang: he
direction: rtl
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 461ea741be813740f0c99f4af35a2b23f5d98fe22292211e740f1448f522d617
source_last_modified: "2026-02-13T09:32:33.722418+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/compliance/android/device_lab_contingency.md -->

# יומן תוכנית גיבוי למעבדת מכשירים

תעדו כאן כל הפעלה של תוכנית הגיבוי למעבדת המכשירים של Android.
כללו די פירוט לבדיקות ציות ולביקורות מוכנות עתידיות.

| תאריך | טריגר | פעולות שבוצעו | המשך טיפול | בעלים |
|------|---------|---------------|------------|-------|
| 2026-02-11 | הקיבולת ירדה ל‑78% לאחר השבתת נתיב Pixel 8 Pro ועיכוב במסירת Pixel 8a (ראו `android_strongbox_device_matrix.md`). | קידום נתיב Pixel 7 ליעד CI ראשי, השאלת צי Pixel 6 משותף, תזמון smoke tests ב‑Firebase Test Lab לדוגמת retail-wallet, והפעלת מעבדת StrongBox חיצונית לפי תוכנית AND6. | להחליף מתאם USB‑C פגום עבור Pixel 8 Pro (עד 2026-02-15); לאשר הגעת Pixel 8a ולעדכן דו"ח קיבולת. | מוביל מעבדת חומרה |
| 2026-02-13 | החלפת ה‑hub של Pixel 8 Pro ואישור Galaxy S24, העלו את הקיבולת ל‑85%. | החזרת נתיב Pixel 7 לשני, הפעלה מחדש של עבודת Buildkite `android-strongbox-attestation` עם התגיות `pixel8pro-strongbox-a` ו‑`s24-strongbox-a`, ועדכון מטריצת המוכנות + יומן הראיות. | מעקב אחר ETA למסירת Pixel 8a (עדיין ממתין); לשמור מלאי hubs חלופיים מתועד. | מוביל מעבדת חומרה |

</div>
