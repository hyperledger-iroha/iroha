---
lang: ar
direction: rtl
source: docs/portal/docs/reference/README.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: الفهرس المرجعي
سبيكة: / مرجع
---

هذا هو المكان الذي ستنضم إليه المادة "كما هي محددة" لـ Iroha. تبقى هذه الصفحات ثابتة حتى مع تطور الأدلة والبرامج التعليمية.

## متاح اليوم

- **العرض العام لبرنامج الترميز Norito** - `reference/norito-codec.md` يوجه مباشرة إلى محدد تلقائي `norito.md` أثناء إرسال لوحة البوابة.
- **Torii OpenAPI** - `/reference/torii-openapi` يعرض REST محددًا أحدث Torii باستخدام Redoc. قم بإعادة إنشاء المواصفات مع `npm run sync-openapi -- --version=current --latest` (أضف `--mirror=<label>` لنسخ أو لقطة للأجزاء التاريخية المضافة).
- **ألواح التكوين** - كتالوج كامل للمعلمات في `docs/source/references/configuration.md`. إذا كانت البوابة توفر الاستيراد التلقائي، فاطلع على ملف Markdown للإعدادات الافتراضية وتجاوزات البيئة المحيطة.
- **إصدار المستندات** - تعرض القائمة المنسدلة للعكس في شريط التنقل لقطات مجمعة مكتوبة على `npm run docs:version -- <label>`، مما يسهل مقارنة الاتجاهات بين الإصدارات.