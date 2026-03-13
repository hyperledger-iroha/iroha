---
lang: he
direction: rtl
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-11-08T06:08:33.073497+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: אינדקס רפרנס
slug: /reference
---

הסעיף הזה מרכז את חומרי "קראו זאת כמפרט" עבור Iroha. הדפים הללו נשארים יציבים גם כשהמדריכים והטוטוריאלים מתפתחים.

## זמין היום

- **סקירת codec Norito** - `reference/norito-codec.md` מקשר ישירות למפרט הקנוני `norito.md` בזמן שטבלת הפורטל מתמלאת.
- **Torii OpenAPI** - `/reference/torii-openapi` מציג את מפרט ה-REST העדכני של Torii באמצעות Redoc. אפשר לייצר מחדש את ה-spec עם `npm run sync-openapi -- --version=current --latest` (הוסיפו `--mirror=<label>` כדי להעתיק את ה-snapshot לגרסאות היסטוריות נוספות).
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v2/mcp`.
- **טבלאות תצורה** - קטלוג הפרמטרים המלא נמצא ב-`docs/source/references/configuration.md`. עד שהפורטל יספק auto-import, עיינו בקובץ Markdown הזה לערכי ברירת מחדל מדויקים ולהחלפות סביבה.
- **גרסאות docs** - תפריט הגרסה בסרגל הניווט מציג snapshots קפואים שנוצרו עם `npm run docs:version -- <label>`, כך שקל להשוות הנחיות בין releases.
