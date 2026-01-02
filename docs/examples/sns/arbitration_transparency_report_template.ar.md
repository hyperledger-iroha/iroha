---
lang: ar
direction: rtl
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-11-15T07:34:14.070551+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/sns/arbitration_transparency_report_template.md -->

# تقرير شفافية التحكيم SNS - <Month YYYY>

- **اللاحقة:** `<.sora / .nexus / .dao>`
- **نافذة التقرير:** `<ISO start>` -> `<ISO end>`
- **اعده:** `<Council liaison>`
- **مصادر artefacts:** `cases.ndjson` SHA256 `<hash>`, تصدير لوحة `<filename>.json`

## 1. الملخص التنفيذي

- اجمالي القضايا الجديدة: `<count>`
- القضايا المغلقة خلال الفترة: `<count>`
- الامتثال لـ SLA: `<ack %>` acknowledge / `<resolution %>` decision
- overrides الحارس الصادرة: `<count>`
- التحويلات/الاستردادات المنفذة: `<count>`

## 2. مزيج القضايا

| نوع النزاع | قضايا جديدة | قضايا مغلقة | وسيط زمن الحل (ايام) |
|------------|------------|------------|----------------------|
| الملكية | 0 | 0 | 0 |
| انتهاك السياسة | 0 | 0 | 0 |
| اساءة | 0 | 0 | 0 |
| الفوترة | 0 | 0 | 0 |
| اخرى | 0 | 0 | 0 |

## 3. اداء SLA

| الاولوية | SLA الاستلام | متحقق | SLA الحل | متحقق | الانتهاكات |
|---------|--------------|-------|----------|-------|------------|
| عاجل | <= 2 h | 0% | <= 72 h | 0% | 0 |
| عالي | <= 8 h | 0% | <= 10 d | 0% | 0 |
| قياسي | <= 24 h | 0% | <= 21 d | 0% | 0 |
| معلوماتي | <= 3 d | 0% | <= 30 d | 0% | 0 |

اوصف الاسباب الجذرية لاي انتهاك واربط تذاكر المعالجة.

## 4. سجل القضايا

| معرّف القضية | المحدد | الاولوية | الحالة | النتيجة | ملاحظات |
|-------------|--------|----------|--------|---------|---------|
| SNS-YYYY-NNNNN | `label.suffix` | Standard | Closed | Upheld | `<summary>` |

قدم ملاحظات سطر واحد تشير الى حقائق مجهولة الهوية او روابط تصويت عامة. اختم حيث يلزم
واذكر التنقيحات المطبقة.

## 5. الاجراءات والمعالجات

- **التجميد / الافراج:** `<counts + case ids>`
- **التحويلات:** `<counts + assets moved>`
- **تعديلات الفوترة:** `<credits/debits>`
- **متابعات السياسة:** `<tickets or RFCs opened>`

## 6. الاستئنافات و overrides الحارس

لخص اي استئناف تم تصعيده الى guardian board، بما في ذلك timestamps والقرارات
(approve/deny). اربط سجلات `sns governance appeal` او تصويت council.

## 7. بنود معلقة

- `<Action item>` - Owner `<name>`, ETA `<date>`
- `<Action item>` - Owner `<name>`, ETA `<date>`

ارفق NDJSON وتصديرات Grafana وسجلات CLI المشار اليها في هذا التقرير.

</div>
