---
lang: he
direction: rtl
source: docs/portal/docs/reference/README.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: אינדקס רפרנס
slug: /reference
---

הסעיף הזה מרכז את חומרי "קראו זאת כמפרט" עבור Iroha. הדפים הללו נשארים יציבים גם כשהמדריכים והטוטוריאלים מתפתחים.

## זמין היום

- **סקירת codec Norito** - `reference/norito-codec.md` מקשר ישירות למפרט הקנוני `norito.md` בזמן שטבלת הפורטל מתמלאת.
- **Torii OpenAPI** - `/reference/torii-openapi` מציג את מפרט ה-REST העדכני של Torii באמצעות Redoc. אפשר לייצר מחדש את ה-spec עם `npm run sync-openapi -- --version=current --latest` (הוסיפו `--mirror=<label>` כדי להעתיק את ה-snapshot לגרסאות היסטוריות נוספות).
- **טבלאות תצורה** - קטלוג הפרמטרים המלא נמצא ב-`docs/source/references/configuration.md`. עד שהפורטל יספק auto-import, עיינו בקובץ Markdown הזה לערכי ברירת מחדל מדויקים ולהחלפות סביבה.
- **גרסאות docs** - תפריט הגרסה בסרגל הניווט מציג snapshots קפואים שנוצרו עם `npm run docs:version -- <label>`, כך שקל להשוות הנחיות בין releases.
