---
lang: ar
direction: rtl
source: docs/portal/docs/intro.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# شاهد بوابة المصممين SORA Nexus

تقوم بوابة المطورين SORA Nexus بإعادة توحيد المستندات التفاعلية ودروس SDK ومراجع API لمشغلي Nexus والمساهمين Hyperledger Iroha. إنه مكمل للموقع الرئيسي للمستندات من خلال عرض الأدلة العملية والمواصفات التي يتم توزيعها مباشرة من المستودع. الصفحة المقصودة الآن تحتوي على نقاط إدخال موضوعية لـ Norito/SoraFS، لقطات OpenAPI مقتولة ومرجع مخصص لـ Norito يتدفق ليتمكن المساهمون من الدخول أو التعاقد على مستوى التحكم في التدفق الأوعية الدموية وRaiz المواصفات.

## O que voce pode fazer aqui

- **اكتشف Norito** - احصل على نظرة عامة وبدء سريع لفهم نموذج التسلسل وأدوات الكود الثانوي.
- **Inicializar SDKs** - يدعم Quickstarts لـ JavaScript وRust اليوم؛ تتوافق أدوات Python وSwift وAndroid الإضافية مع الإيصالات المقدمة.
- **استكشاف مراجع واجهة برمجة التطبيقات** - تعرض الصفحة OpenAPI إلى Torii REST محددًا في الآونة الأخيرة، وجداول تكوين على شكل خطوط أساسية في Markdown.
- **إعداد عمليات النشر** - عمليات تشغيل دفاتر التشغيل (القياس عن بعد، والتسوية، وتراكبات Nexus) يتم إرسالها من `docs/source/` ويتم توصيلها إلى هذا الموقع وفقًا للترحيل المتقدم.

## الحالة حقيقية

- OK Landing Docusaurus v3 تم تخصيصه مع التصنيف المجدد والبطل/البطاقات الموجهة بالتدرج وشرائح الموارد التي تتضمن ملخص Norito Streaming.
- OK Plugin OpenAPI do Torii مرتبط بـ `npm run sync-openapi`، مع التحقق من اللقطات المقتولة وحماية CSP المطبقة بواسطة `buildSecurityHeaders`.
- OK Preview وتغطية المسبار على CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`)، قبل فتح مستند البث، وبدء التشغيل السريع لـ SoraFS وقوائم المراجعة المرجعية قبل نشر العناصر.
- OK Quickstarts de Norito, SoraFS وSDKs المرجعية الثانية موجودة على الشريط الجانبي؛ novas importacoes de `docs/source/` (البث المباشر، والتنسيق، وسجلات التشغيل) chegam هنا تتوافق مع كتابتك.

## كومو مشارك

- Veja `docs/portal/README.md` لأمر التطوير المحلي (`npm install`، `npm run start`، `npm run build`).
- كمهام الهجرة للمحتوى المصاحب لها بجانب عناصر خريطة الطريق `DOCS-*`. المساهمة في النوافذ - الباب الثاني من `docs/source/` وإضافة الصفحة إلى الشريط الجانبي.
- إذا قمت بإضافة قطعة فنية جيدة (المواصفات، جداول التكوين)، قم بتوثيق أو قيادة بناء حتى يتمكن المساهمون المستقبليون من تحديثها بسهولة.