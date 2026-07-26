---
lang: ar
direction: rtl
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-11-21T12:24:49.353535+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/soranet_gateway_billing/reconciliation_report_template.md -->

# تسوية فواتير بوابة SoraGlobal

- **النافذة:** `<from>/<to>`
- **المستاجر:** `<tenant-id>`
- **اصدار الكتالوج:** `<catalog-version>`
- **لقطة الاستخدام:** `<path or hash>`
- **حدود الحماية:** حد مرن `<soft-cap-xor> XOR`, حد صارم `<hard-cap-xor> XOR`, عتبة تنبيه `<alert-threshold>%`
- **الدافع -> الخزانة:** `<payer>` -> `<treasury>` في `<asset-definition>`
- **الاجمالي المستحق:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## فحوصات بنود السطور
- [ ] قيود الاستخدام تغطي فقط معرفات عدادات الكتالوج ومناطق الفوترة الصالحة
- [ ] وحدات الكمية تطابق تعريفات الكتالوج (requests, GiB, ms, etc.)
- [ ] تطبيق مضاعفات المناطق وشرائح الخصم حسب الكتالوج
- [ ] صادرات CSV/Parquet تطابق بنود الفاتورة JSON

## تقييم حدود الحماية
- [ ] هل تم الوصول الى عتبة تنبيه الحد المرن؟ `<yes/no>` (ارفق دليل التنبيه اذا كان yes)
- [ ] هل تم تجاوز الحد الصارم؟ `<yes/no>` (اذا كان yes، ارفق موافقة override)
- [ ] الحد الادنى للفاتورة مستوفى

## اسقاط الدفتر
- [ ] اجمالي دفعة التحويل يساوي `total_micros` في الفاتورة
- [ ] تعريف الاصل يطابق عملة الفوترة
- [ ] حسابات الدافع والخزانة تطابق المستاجر والمشغل المسجل
- [ ] ارفاق artefacts Norito/JSON لاعادة تشغيل التدقيق

## ملاحظات النزاع/التعديل
- التباين الملحوظ: `<variance detail>`
- التعديل المقترح: `<delta and rationale>`
- الادلة الداعمة: `<logs/dashboards/alerts>`

## الموافقات
- محلل الفوترة: `<name + signature>`
- مراجع الخزانة: `<name + signature>`
- hash حزمة الحوكمة: `<hash/reference>`

</div>
