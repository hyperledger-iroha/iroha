---
lang: ar
direction: rtl
source: docs/portal/docs/reference/README.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: الفهرس المرجعي
سبيكة: / مرجع
---

يقوم هذا القسم بإعادة تجميع المواد إلى مواصفات خاصة بـ Iroha. هذه الصفحات موجودة في الاسطبلات عندما تتطور الأدلة والبرامج التعليمية.

## متاح في aujourd'hui

- **التعرف على برنامج الترميز Norito** - `reference/norito-codec.md` راجع التوجيه المباشر للمواصفات الرسمية `norito.md` حتى يتم إعادة جدولة الشاشة.
- **Torii OpenAPI** - `/reference/torii-openapi` rend la derniereمواصفات REST de Torii مع Redoc. قم بإعادة إنشاء المواصفات عبر `npm run sync-openapi -- --version=current --latest` (أضف `--mirror=<label>` لنسخ اللقطة في الإصدارات التاريخية الإضافية).
- **جداول التكوين** - تم العثور على الكتالوج الكامل للمعلمات في `docs/source/references/configuration.md`. إذا لم يقترح الباب الاستيراد التلقائي، فارجع إلى ملف Markdown هذا للقيم الافتراضية الدقيقة والرسوم الإضافية للبيئة.
- **إصدار المستندات** - تعرض قائمة الإصدار الموجودة في شريط التنقل لقطات تظهر مع `npm run docs:version -- <label>`، مما يسهل مقارنة التوصيات بين الإصدارات.