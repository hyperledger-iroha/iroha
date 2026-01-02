---
lang: he
direction: rtl
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-11-21T12:24:49.353535+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_gateway_billing/reconciliation_report_template.md -->

# התאמת חיוב לשער SoraGlobal

- **חלון:** `<from>/<to>`
- **Tenant:** `<tenant-id>`
- **גרסת קטלוג:** `<catalog-version>`
- **Snapshot שימוש:** `<path or hash>`
- **ספי הגנה:** מגבלה רכה `<soft-cap-xor> XOR`, מגבלה קשיחה `<hard-cap-xor> XOR`, סף התראה `<alert-threshold>%`
- **משלם -> אוצר:** `<payer>` -> `<treasury>` ב-`<asset-definition>`
- **סך לתשלום:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## בדיקות שורות חיוב
- [ ] רשומות שימוש מכסות רק מזהי מונה מהקטלוג ואזורי חיוב תקפים
- [ ] יחידות הכמות תואמות להגדרות הקטלוג (requests, GiB, ms, etc.)
- [ ] מכפילי אזור ומדרגות הנחה הוחלו לפי הקטלוג
- [ ] יצואי CSV/Parquet תואמים לשורות החשבונית ב-JSON

## הערכת ספי הגנה
- [ ] האם סף ההתראה של המגבלה הרכה הושג? `<yes/no>` (צרפו ראיית התראה אם yes)
- [ ] האם המגבלה הקשיחה הוחרגה? `<yes/no>` (אם yes, צרפו אישור override)
- [ ] רצפת חשבונית מינימלית מולאה

## הקרנת ספר חשבונות
- [ ] סך אצווה ההעברה שווה ל-`total_micros` בחשבונית
- [ ] הגדרת הנכס תואמת למטבע החיוב
- [ ] חשבונות המשלם והאוצר תואמים ל-tenant ולמפעיל הרשום
- [ ] צורפו artefacts Norito/JSON לשחזור ביקורת

## הערות מחלוקת/התאמה
- שונות נצפית: `<variance detail>`
- התאמה מוצעת: `<delta and rationale>`
- ראיות תומכות: `<logs/dashboards/alerts>`

## אישורים
- אנליסט חיוב: `<name + signature>`
- סוקר אוצר: `<name + signature>`
- hash של חבילת ממשל: `<hash/reference>`

</div>
