---
lang: ar
direction: rtl
source: docs/portal/docs/reference/README.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: فهرس المراجع
سبيكة: / مرجع
---

جمع هذا القسم المواد "اقرأها كمواصفة" الخاصة بـ Iroha. تستمر هذه الصفحات حتى مع عقود المعادلة والدروس.

## إحجز اليوم

- **نظرة عامة على برنامج الترميز Norito** - ربط `reference/norito-codec.md` مباشرة بالمواصفة القياسية `norito.md` بينما يتم ربط جدول البوابة.
- **Torii OpenAPI** - المقدمة `/reference/torii-openapi` احدث مواصفات REST الخاصة بـ Torii باستخدام Redoc. إعادة توليد المواصفة عبر `npm run sync-openapi -- --version=current --latest` (اضف `--mirror=<label>` لنسخ اللقطة الى النسخة الاحتياطية).
- ** جداول الاعدادات ** - يوجد الكتالوج الكامل للمعلمات في `docs/source/references/configuration.md`. الى ان يوفر لك استيرادا خارجيا، ارجع الى ملف Markdown هذا لقيم محددة واستخدامات البيئة.
- **اصدار الوثائق** - تم عرض قائمة الاصدار في شريط لقطات مجمدة تم انشاؤها بواسطة `npm run docs:version -- <label>`، مما يمثل مقارنة التوجيه عبر الاصدارات.