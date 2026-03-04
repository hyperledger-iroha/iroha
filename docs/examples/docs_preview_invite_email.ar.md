---
lang: ar
direction: rtl
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-11-15T04:49:30.881970+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/docs_preview_invite_email.md -->

# دعوة معاينة بوابة المستندات (بريد نموذجي)

استخدم هذا المثال عند صياغة الرسالة الصادرة. يلتقط النص الفعلي المرسل لمراجعي مجتمع
W2 (`preview-2025-06-15`) حتى تتمكن الموجات القادمة من محاكاة النبرة وتعليمات
التحقق ومسار الادلة دون الرجوع للتذاكر القديمة. حدّث روابط artefacts والهاشات
ومعرفات الطلب والتواريخ قبل ارسال دعوة جديدة.

```text
الموضوع: [DOCS-SORA] دعوة معاينة بوابة docs preview-2025-06-15 لـ Horizon Wallet

مرحبا Sam,

شكرا مرة اخرى لتطوع Horizon Wallet لمعاينة مجتمع W2. تم اعتماد موجة W2، لذا يمكنك
بدء المراجعة بمجرد اكمالك الخطوات ادناه. يرجى الحفاظ على سرية artefacts ورموز
الوصول: كل دعوة يتم تتبعها في DOCS-SORA-Preview-W2 وسيقوم المدققون بمراجعة
الاقرارات.

1. نزّل artefacts الموثقة (نفس الملفات التي ارسلناها الى SoraFS و CI):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. تحقق من bundle قبل فك الضغط:

   ./docs/portal/scripts/preview_verify.sh      --descriptor ~/Downloads/descriptor.json      --archive ~/Downloads/docs-portal-preview.tar.zst      --build-dir ~/sora-docs/preview-2025-06-15

3. شغّل المعاينة مع تفعيل enforcement للـ checksum:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. راجع runbooks المقواة قبل الاختبار:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. قدّم الملاحظات عبر DOCS-SORA-Preview-REQ-C04 وضع وسم `docs-preview/w2` على كل
   ملاحظة. استخدم نموذج الملاحظات اذا كنت تفضل intake منظم:
   docs/examples/docs_preview_feedback_form.md.

الدعم متاح عبر Matrix (`#docs-preview:matrix.org`) ولدينا office hours في
2025-06-18 15:00 UTC. لتصعيدات الامن او الحوادث، ارسل نداء فوري الى
docs on-call عبر ops@sora.org او +1-555-0109؛ لا تنتظر office hours.

مدة الوصول لمعاينة Horizon Wallet هي 2025-06-15 -> 2025-06-29. اخبرنا فور الانتهاء
حتى نلغي مفاتيح الوصول المؤقتة ونسجل الاغلاق في tracker.

نقدر مساعدتك للوصول بالبوابة الى GA!

- فريق DOCS-SORA
```

</div>
