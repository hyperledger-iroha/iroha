---
lang: ar
direction: rtl
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-11-08T06:08:33.073497+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: فهرس المراجع
slug: /reference
---

يجمع هذا القسم مواد "اقرأها كمواصفة" الخاصة بـ Iroha. تبقى هذه الصفحات مستقرة حتى مع تطور الادلة والدروس.

## المتاح اليوم

- **نظرة عامة على Norito codec** - يربط `reference/norito-codec.md` مباشرة بالمواصفة القياسية `norito.md` بينما يتم ملء جدول البوابة.
- **Torii OpenAPI** - يعرض `/reference/torii-openapi` احدث مواصفات REST الخاصة بـ Torii باستخدام Redoc. اعادة توليد المواصفة عبر `npm run sync-openapi -- --version=current --latest` (اضف `--mirror=<label>` لنسخ اللقطة الى نسخ تاريخية اضافية).
- **جداول الاعدادات** - يوجد الكتالوج الكامل للمعلمات في `docs/source/references/configuration.md`. الى ان يوفر البوابة استيرادا تلقائيا، ارجع الى ملف Markdown هذا للقيم الافتراضية الدقيقة واستبدالات البيئة.
- **اصدارات الوثائق** - تعرض قائمة الاصدار في شريط التنقل لقطات مجمدة تم انشاؤها بواسطة `npm run docs:version -- <label>`، مما يسهل مقارنة التوجيه عبر الاصدارات.
