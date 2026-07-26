---
lang: ar
direction: rtl
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-11-19T07:56:11.635822+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/docs_preview_feedback_digest.md -->

# ملخص ملاحظات معاينة بوابة المستندات (قالب)

استخدم هذا القالب عند تلخيص موجة معاينة للحوكمة او مراجعات الاصدار او `status.md`.
انسخ Markdown الى تذكرة المتابعة، واستبدل القوالب ببيانات فعلية، وارفق ملخص JSON
المصدّر عبر `npm run --prefix docs/portal preview:log -- --summary --summary-json`.
يساعدك `preview:digest` (`npm run --prefix docs/portal preview:digest -- --wave <label>`) على
توليد قسم المقاييس ادناه حتى تملأ فقط صفوف highlights/actions/artefacts.

```markdown
## ملخص ملاحظات موجة preview-<tag> (YYYY-MM-DD)
- نافذة الدعوة: <start -> end>
- عدد المراجعين المدعوين: <count> (open: <count>)
- عدد ارسال الملاحظات: <count>
- عدد القضايا المفتوحة: <count>
- اخر طابع زمني للحدث: <ISO8601 from summary.json>

| الفئة | التفاصيل | المالك / المتابعة |
| --- | --- | --- |
| Highlights | <مثال: "ISO builder walkthrough landed well"> | <owner + موعد> |
| ملاحظات حرجة | <قائمة issue IDs او روابط tracker> | <owner> |
| تحسينات طفيفة | <تجميع تعديلات شكلية او copy> | <owner> |
| شذوذات التيليمترية | <رابط snapshot للوحة / log probe> | <owner> |

## الاجراءات
1. <اجراء + رابط + ETA>
2. <اجراء ثان اختياري>

## artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Wave summary: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Dashboard snapshot: `<link or path>`

```

احتفظ بكل ملخص مع تذكرة تتبع الدعوات حتى يتمكن المراجعون والحوكمة من اعادة مسار
الادلة دون البحث في سجلات CI.

</div>
