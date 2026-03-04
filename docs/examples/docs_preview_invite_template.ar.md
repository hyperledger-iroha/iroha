---
lang: ar
direction: rtl
source: docs/examples/docs_preview_invite_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/docs_preview_invite_template.md -->

# دعوة معاينة بوابة المستندات (قالب)

استخدم هذا القالب عند ارسال تعليمات الوصول لمراجعي المعاينة. استبدل القوالب (`<...>`) بالقيم
المناسبة، وارفق artefacts الخاصة بـ descriptor + archive المشار اليها في الرسالة، واحفظ
النص النهائي داخل تذكرة الاستقبال المقابلة.

```text
الموضوع: [DOCS-SORA] دعوة معاينة بوابة docs <preview_tag> لـ <reviewer/org>

مرحبا <name>,

شكرا لتطوعك لمراجعة بوابة docs قبل GA. تم اعتمادك للموجة <wave_id>. يرجى اتباع
الخطوات ادناه قبل تصفح المعاينة:

1. نزّل artefacts الموثقة من CI او SoraFS:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archive: <archive_url> (`sha256:<archive_sha256>`)
2. شغّل بوابة التحقق من checksum:

   ./docs/portal/scripts/preview_verify.sh      --descriptor <path-to-descriptor>      --archive <path-to-archive>      --build-dir <path-to-extracted-build>

3. شغّل المعاينة مع تفعيل enforcement للـ checksum:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. اقرأ ملاحظات الاستخدام المقبول والامن والمراقبة:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. قدم الملاحظات عبر <request_ticket> وضع وسم `<preview_tag>` على كل ملاحظة.

الدعم متاح عبر <contact_channel>. يجب الابلاغ الفوري عن الحوادث او القضايا الامنية عبر
<incident_channel>. اذا كنت تحتاج الى رموز Torii API فاطلبها عبر التذكرة؛ لا تعاود
استخدام بيانات اعتماد الانتاج.

تنتهي صلاحية الوصول للمعاينة في <end_date> ما لم يتم تمديدها كتابيا. نسجل checksums
وبيانات الدعوة للحوكمة؛ ابلغنا عند الانتهاء حتى نقوم بانهاء الوصول بشكل نظيف.

شكرا مرة اخرى لمساعدتنا على تثبيت البوابة!

- فريق DOCS-SORA
```

</div>
