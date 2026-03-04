---
lang: ar
direction: rtl
source: docs/portal/docs/reference/README.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: الفهرس المرجعي
سبيكة: / مرجع
---

هذا القسم يعيد توحيد مادة "leelo مثل المواصفات" لـ Iroha. يتم الحفاظ على هذه الصفحات ثابتة بما في ذلك عند تطوير الأدلة والبرامج التعليمية.

## هوي متاح

- **استئناف برنامج الترميز Norito** - يتم تشغيل `reference/norito-codec.md` مباشرة على المواصفات التلقائية `norito.md` حتى تكتمل لوحة البوابة.
- **Torii OpenAPI** - `/reference/torii-openapi` يعرض REST المواصفات الأحدث لـ Torii باستخدام Redoc. قم بإعادة إنشاء المواصفات مع `npm run sync-openapi -- --version=current --latest` (اجمع `--mirror=<label>` لنسخ اللقطة في الإصدارات التاريخية الإضافية).
- **ألواح التكوين** - يتم الحفاظ على كتالوج المعلمات الكامل في `docs/source/references/configuration.md`. حتى تنشر البوابة استيرادًا تلقائيًا، راجع هذا الملف Markdown للقيم المعيبة والحذف الداخلي.
- **إصدار المستندات** - يعرض الإصدار القابل للإلغاء في شريط التنقل اللقطات المجمعة التي تم إنشاؤها باستخدام `npm run docs:version -- <label>`، مما يسهل مقارنة الدليل بين الإصدارات.