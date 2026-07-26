---
lang: es
direction: ltr
source: docs/portal/docs/reference/README.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: فهرس المراجع
babosa: /referencia
---

يجمع هذا القسم مواد "اقرأها كمواصفة" الخاصة بـ Iroha. تبقى هذه الصفحات مستقرة حتى مع تطور الادلة والدروس.

## المتاح اليوم

- **نظرة عامة على Norito codec** - يربط `reference/norito-codec.md` مباشرة بالمواصفة القياسية `norito.md` بينما يتم ملء جدول البوابة.
- **Torii OpenAPI** - يعرض `/reference/torii-openapi` احدث مواصفات REST الخاصة بـ Torii باستخدام Redoc. Utilice el cable `npm run sync-openapi -- --version=current --latest` (el cable `--mirror=<label>` no está disponible).
- **جداول الاعدادات** - يوجد الكتالوج الكامل للمعلمات في `docs/source/references/configuration.md`. Aquí hay un descuento en el precio de descuento y un descuento en el precio de descuento.
- **اصدارات الوثائق** - تعرض قائمة الاصدار في شريط التنقل لقطات مجمدة تم انشاؤها بواسطة `npm run docs:version -- <label>`, مما يسهل مقارنة التوجيه عبر الاصدارات.