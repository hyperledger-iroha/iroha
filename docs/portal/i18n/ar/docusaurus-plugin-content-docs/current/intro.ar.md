---
lang: ar
direction: rtl
source: docs/portal/docs/intro.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مرحبا بكم في بوابة مطوري SORA Nexus

جيجا بوابة مطوري SORA Nexus اتصالات تفاعلية ودروس SDK ومراجع API لمشغلي Nexus ومساهمي Hyperledger Iroha. تكمل وثائق الموقع الرئيسية عبر ابراز المعاملة والمواصفات المولدة مباشرة من هذا المستودع. صفحة تحتوى على الان على دخول نقاط ذات طابع Norito/SoraFS، ولقطات OpenAPI موقعة، ومرجع مخصص لـ Norito Streaming حتى يتمكن المساهمون من العثور على عقد التحكم للبث دون التنقيب في المواصفة المتعددة.

## ما الذي فعلته هنا

- **تعلم Norito** - البدء بنظرة عامة وبدء سريع لفهم نموذج التسلسل وادوات البايت كود.
- **طلاق SDKs** - اتبع البدايات السريعة لـ JavaScript وRust Day؛ ستنضم إلى Python وSwift وAndroid مع ترحيل الوصفات.
- **تصفح المراجع API** - تم عرض صفحة Torii OpenAPI احدث مواصفات REST، وترجمات الاعدادات بمصادر Markdown الممتازة.
- ** التحضير النشر** - يتم رحيل طيارات التشغيل (القياس عن بعد، التسوية، تراكبات Nexus) من `docs/source/` وستصل إلى هذا الموقع مع تقدم الرحيل.

## الحالة الحالية

- ✅ صفحة الهبوط Docusaurus v3 ذات طابع مع طباعة مجددة و Hero/cards مدفوع بتدرجات وبلاتات الموارد تتضمن ملخص Norito Streaming.
- ✅ تم توصيل اضافة Torii OpenAPI بامر `npm run sync-openapi` مع فحوصات لقطات موقعة وحمايات CSP يفرضها `buildSecurityHeaders`.
- ✅ تشغيل معاينة التغطية و المسبار في CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) والتي باتت تضع بوابة التلوثيقة الجري و Quickstarts SoraFS وقوائم المراجعة المرجعية قبل نشر الحزم.
- ✅ بدأت عمليات التشغيل السريع الخاصة بـ Norito و SoraFS و SDKs مع أقسام المرجعية في الشريط الجانبي؛ وتصل الاستيرادات الجديدة من `docs/source/` (البث المباشر، التوزيع، كتب التشغيل) هنا عند كتابها.

## المشاركة

- راجع `docs/portal/README.md` لاوامر التطوير المحلي (`npm install`, `npm run start`, `npm run build`).
- تم تطبيق مهام ترحيل المحتوى جنبًا إلى جنب مع عناصر خريطة الطريق `DOCS-*`. المساهمات مرحب بها - انقل اقساما من `docs/source/` واضف الصفحة الى الشريط الجانبي.
- اذا اضفت مخرجا مولدا (المواصفات، جداول التكوين)، ويصدق امر البناء ليتمكن المساهمون مستقبلا من تحديثه بسهولة.