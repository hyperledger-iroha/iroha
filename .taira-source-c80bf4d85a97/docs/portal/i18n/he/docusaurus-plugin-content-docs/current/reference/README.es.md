---
lang: es
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1fbf38f14e6e5b4b90d31b072e189035b463e3a3dd2559b8b347fe57b524c55a
source_last_modified: "2026-01-03T18:07:58+00:00"
translation_last_reviewed: 2026-01-30
---

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
