---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f25e7e97a8375c52e0f822cb6d6be44b95e104060a28a42c289794dc8eb525c
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ar
direction: rtl
source: docs/portal/docs/reference/README.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: فهرس المراجع
slug: /reference
---

يجمع هذا القسم مواد "اقرأها كمواصفة" الخاصة بـ Iroha. تبقى هذه الصفحات مستقرة حتى مع تطور الادلة والدروس.

## المتاح اليوم

- **نظرة عامة على Norito codec** - يربط `reference/norito-codec.md` مباشرة بالمواصفة القياسية `norito.md` بينما يتم ملء جدول البوابة.
- **Torii OpenAPI** - يعرض `/reference/torii-openapi` احدث مواصفات REST الخاصة بـ Torii باستخدام Redoc. اعادة توليد المواصفة عبر `npm run sync-openapi -- --version=current --latest` (اضف `--mirror=<label>` لنسخ اللقطة الى نسخ تاريخية اضافية).
- **جداول الاعدادات** - يوجد الكتالوج الكامل للمعلمات في `docs/source/references/configuration.md`. الى ان يوفر البوابة استيرادا تلقائيا، ارجع الى ملف Markdown هذا للقيم الافتراضية الدقيقة واستبدالات البيئة.
- **اصدارات الوثائق** - تعرض قائمة الاصدار في شريط التنقل لقطات مجمدة تم انشاؤها بواسطة `npm run docs:version -- <label>`، مما يسهل مقارنة التوجيه عبر الاصدارات.
