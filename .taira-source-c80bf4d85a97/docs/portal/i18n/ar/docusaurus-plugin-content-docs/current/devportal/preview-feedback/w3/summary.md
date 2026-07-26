---
id: preview-feedback-w3-summary
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W3 - دفعات بيتا (المالية + ops + شريك SDK + داعم النظام البيئي) |
| نافذة الدعوة | 2026-02-18 -> 2026-02-28 |
| وسم الاثر | `preview-20260218` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W3` |
| المشاركون | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## ابرز النقاط

1. **خط انابيب الادلة end-to-end.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` يولد ملخص الموجة (`artifacts/docs_portal_preview/preview-20260218-summary.json`)، والملخص المختصر (`preview-20260218-digest.md`)، ويحدث `docs/portal/src/data/previewFeedbackSummary.json` حتى يتمكن مراجعو الحوكمة من الاعتماد على امر واحد.
2. **تغطية القياس والحوكمة.** اكد المراجعون الاربعة الوصول المحمي بالـ checksum، قدموا الملاحظات، وتم سحبهم في الوقت المحدد; يشير الملخص الى قضايا الملاحظات (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) مع تشغيلات Grafana التي جُمعت خلال الموجة.
3. **عرض البوابة.** جدول البوابة المحدث يعرض الان موجة W3 المغلقة مع مقاييس زمن الاستجابة ومعدل الاستجابة، والصفحة الجديدة للسجل ادناه تعكس الخط الزمني للمدققين الذين لا يسحبون سجل JSON الخام.

## بنود العمل

| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W3-A1 | التقاط ملخص preview وارفاقه بالمتتبع. | Docs/DevRel lead | ✅ مكتمل (2026-02-28). |
| W3-A2 | نسخ ادلة الدعوة/الملخص الى البوابة + roadmap/status. | Docs/DevRel lead | ✅ مكتمل (2026-02-28). |

## ملخص الخروج (2026-02-28)

- ارسلت الدعوات في 2026-02-18 مع تسجيل الاقرارات بعد دقائق; تم سحب صلاحيات المعاينة في 2026-02-28 بعد اجتياز فحص القياس الاخير.
- تم حفظ الملخص والموجز تحت `artifacts/docs_portal_preview/`, مع تثبيت السجل الخام عبر `artifacts/docs_portal_preview/feedback_log.json` لامكانية اعادة التشغيل.
- متابعات القضايا سجلت تحت `docs-preview/20260218` مع متتبع الحوكمة `DOCS-SORA-Preview-20260218`; ملاحظات CSP/Try it حولت الى مالكي المراقبة/المالية وربطت من الملخص.
- تم تحديث صف المتتبع الى 🈴 مكتمل وعكس جدول الملاحظات في البوابة اغلاق الموجة، منهيا مهمة بيتا المتبقية لـ DOCS-SORA.
