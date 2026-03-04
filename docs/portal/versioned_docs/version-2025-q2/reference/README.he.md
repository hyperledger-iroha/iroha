---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e1f2fc637a1fc283e3079ffc22ddff70c6eb1e568a21b951b61491529052c234
source_last_modified: "2025-11-04T12:24:28.218431+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: אינדקס רפרנס
slug: /reference
---

הסעיף הזה מרכז את חומרי "קראו זאת כמפרט" עבור Iroha. הדפים הללו נשארים יציבים גם כשהמדריכים והטוטוריאלים מתפתחים.

## זמין היום

- **סקירת codec Norito** - `reference/norito-codec.md` מקשר ישירות למפרט הקנוני `norito.md` בזמן שטבלת הפורטל מתמלאת.
- **Torii OpenAPI** - `/reference/torii-openapi` מציג את מפרט ה-REST העדכני של Torii באמצעות Redoc. אפשר לייצר מחדש את ה-spec עם `npm run sync-openapi -- --version=current --latest` (הוסיפו `--mirror=<label>` כדי להעתיק את ה-snapshot לגרסאות היסטוריות נוספות).
- **טבלאות תצורה** - קטלוג הפרמטרים המלא נמצא ב-`docs/source/references/configuration.md`. עד שהפורטל יספק auto-import, עיינו בקובץ Markdown הזה לערכי ברירת מחדל מדויקים ולהחלפות סביבה.
- **גרסאות docs** - תפריט הגרסה בסרגל הניווט מציג snapshots קפואים שנוצרו עם `npm run docs:version -- <label>`, כך שקל להשוות הנחיות בין releases.
