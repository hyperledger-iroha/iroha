---
lang: ar
direction: rtl
source: docs/portal/docs/reference/README.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: فهرس الكلمات
سبيكة: / مرجع
---

تم تجميع هذه المواد "اقرأ حسب المواصفات" لـ Iroha. تتمتع هذه الدول بالاستقرار حتى الآن من خلال تقديم إرشادات وتعليمات بسيطة.

## الآن بكل سهولة

- **رمز البحث Norito** - `reference/norito-codec.md` يتوافق مع مواصفات التفويض `norito.md`، على الطاولة يتم إغلاق البوابة.
- **Torii OpenAPI** - يتم تحديد `/reference/torii-openapi` بالمواصفات التالية REST Torii من خلال Redoc. قم بإعادة إنشاء أمر المواصفات `npm run sync-openapi -- --version=current --latest` (أضف `--mirror=<label>` لنسخ اللقطة في الإصدارات التاريخية الإضافية).
- **تكوينات اللوحة** - يتم وضع معلمة الكتالوج الكامل في `docs/source/references/configuration.md`. حتى لا تسمح البوابة بالاستيراد التلقائي، قم بالتغطية من خلال هذا الملف Markdown لأهم نصائح التحسين والاقتراحات تكاثر.
- **إصدار المستندات** - قم بإضافة نسخة القائمة في القائمة لإظهار اللقطات المحمية، والتي تم إنشاؤها باستخدام `npm run docs:version -- <label>`، من أجل تنفيذ التوصية الصحيحة بين الإعلانات.