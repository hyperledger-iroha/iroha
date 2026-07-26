---
lang: ar
direction: rtl
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-11-10T20:01:03.610024+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/docs_preview_request_template.md -->

# طلب الوصول الى معاينة بوابة docs (قالب)

استخدم هذا القالب عند جمع تفاصيل المراجع قبل منح الوصول الى بيئة المعاينة العامة. انسخ
الـ markdown الى تذكرة او نموذج طلب واستبدل القيم النائبة.

```markdown
## ملخص الطلب
- مقدم الطلب: <الاسم الكامل / الجهة>
- اسم مستخدم GitHub: <username>
- وسيلة التواصل المفضلة: <email/Matrix/Signal>
- المنطقة والمنطقة الزمنية: <UTC offset>
- تواريخ البداية / النهاية المقترحة: <YYYY-MM-DD -> YYYY-MM-DD>
- نوع المراجع: <Core maintainer | Partner | Community volunteer>

## قائمة الامتثال
- [ ] وقّع سياسة الاستخدام المقبول للمعاينة (الرابط).
- [ ] راجع `docs/portal/docs/devportal/security-hardening.md`.
- [ ] راجع `docs/portal/docs/devportal/incident-runbooks.md`.
- [ ] اقر بجمع التليمترية والتحليلات المجهولة (نعم/لا).
- [ ] تم طلب alias لـ SoraFS (نعم/لا). اسم alias: `<docs-preview-???>`

## احتياجات الوصول
- روابط المعاينة: <https://docs-preview.sora.link/...>
- نطاقات API المطلوبة: <Torii read-only | Try it sandbox | none>
- سياق اضافي (اختبارات SDK، تركيز مراجعة التوثيق، الخ):
  <التفاصيل هنا>

## الموافقة
- المراجع (maintainer): <الاسم + التاريخ>
- تذكرة الحوكمة / طلب تغيير: <رابط>
```

---

## اسئلة خاصة بالمجتمع (W2+)
- الدافع للوصول الى المعاينة (جملة واحدة):
- تركيز المراجعة الرئيسي (SDK، الحوكمة، Norito، SoraFS، اخرى):
- الالتزام الاسبوعي والمدة المتاحة (UTC):
- احتياجات التوطين او الوصول (نعم/لا + التفاصيل):
- تم اقرار مدونة السلوك للمجتمع + ملحق الاستخدام المقبول للمعاينة (نعم/لا):

</div>
