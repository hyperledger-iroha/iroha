---
lang: ar
direction: rtl
source: docs/portal/docs/intro.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bienvenue sur le portail Developmentpeur SORA Nexus

تم تطوير البوابة SORA Nexus من خلال إعادة تجميع الوثائق التفاعلية والبرامج التعليمية SDK ومراجع API لمشغلي Nexus والمساهمين Hyperledger Iroha. يكمل الموقع الرئيسي للمستندات من خلال تقديم الأدلة العملية والمواصفات العامة مباشرة من هذا المستودع. تقترح الصفحة الرئيسية تغيير نقاط الدخول المواضيعية Norito/SoraFS، وعلامات اللقطات OpenAPI، ومرجع ينقل إلى Norito البث حتى يتمكن المساهمون من اكتشاف عقد خطة التحكم فيك البث بدون خطأ في مواصفات راسين.

## Ce que vous pouvez faire ici

- **التعرف على Norito** - ابدأ باستخدام الفتحة والبدء السريع لفهم نموذج التسلسل وأدوات الكود الثانوي.
- **إنشاء مجموعات SDK** - متابعة Quickstarts JavaScript والصدأ في اليوم التالي؛ أدلة Python وSwift وAndroid تنضم إلى الفراء وقياس ترحيل القراءات.
- **إظهار المراجع API** - الصفحة OpenAPI من Torii توفر آخر مواصفات REST، وتتطلب جداول التكوين مصادر Markdown canoniques.
- **تحضير عمليات النشر** - عمليات تشغيل دفاتر التشغيل (القياس عن بعد، التسوية، تراكبات Nexus) هي خلال عملية النقل من `docs/source/` وتصل إلى هذا الإجراء أثناء الترحيل المسبق.

## القانون الفعلي

- Landing Docusaurus v3 theme with typographie rafraichie، أدلة البطل/البطاقات حسب التدهور وأدوات الموارد بما في ذلك السيرة الذاتية Norito Streaming.
- البرنامج المساعد OpenAPI Torii كابل على `npm run sync-openapi`، مع التحقق من لقطات اللقطات وحماية CSP المطبقة على `buildSecurityHeaders`.
- يتم تنفيذ معاينة الغطاء والتحقيق في CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`)، وحظر تدفق المستندات، وبدء التشغيل السريع SoraFS، وقوائم المراجعة المرجعية قبل نشر العناصر.
- Quickstarts Norito وSoraFS وSDK بالإضافة إلى أن الأقسام المرجعية موجودة في الشريط الجانبي؛ وصلت الواردات الجديدة من `docs/source/` (البث المباشر والتنسيق وسجلات التشغيل) إلى تنقيحها.

## مشارك

- Voir `docs/portal/README.md` لأوامر التطوير المحلية (`npm install`، `npm run start`، `npm run build`).
- تنتقل مستندات ترحيل المحتوى إلى عناصر خريطة الطريق `DOCS-*`. المساهمات متاحة على الفور: قم بنقل الأقسام من `docs/source/` وقم بإضافة الصفحة إلى الشريط الجانبي.
- إذا قمت بإضافة منتج عام (المواصفات، لوحات التكوين)، قم بتوثيق أمر البناء حتى يتمكن المستقبلون من المساهمة في إعادة الإنشاء بسهولة.