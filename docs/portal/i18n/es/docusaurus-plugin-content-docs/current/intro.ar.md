---
lang: es
direction: ltr
source: docs/portal/docs/intro.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مرحبا بكم في بوابة مطوري SORA Nexus

Programador SORA Nexus, programador, SDK y API Nexus y programador Hyperledger Iroha. تكمل موقع الوثائق الرئيسي عبر ابراز الادلة العملية والمواصفات المولدة مباشرة من هذا المستودع. Para obtener más información, consulte el enlace Norito/SoraFS y OpenAPI. ومرجع مخصص لـ Norito Streaming حتى يتمكن المساهمون من العثور على عقد طبقة التحكم للبث دون التنقيب في المواصفة الجذرية.

## ما الذي يمكنك فعله هنا

- **تعلم Norito** - ابدأ بنظرة عامة e inicio rápido para obtener información sobre el código de bytes.
- **اطلاق SDK** - Inicio rápido de JavaScript y Rust Utilice Python, Swift y Android para ejecutar aplicaciones.
- **API de actualización** - Aplicación Torii OpenAPI Aplicación de REST y reducción de Markdown القياسية.
- **تحضير النشر** - يتم ترحيل كتيبات التشغيل (telemetría, liquidación, superposiciones Nexus) de `docs/source/` y وستصل الى هذا الموقع مع تقدم الترحيل.

## الحالة الحالية- ✅ صفحة هبوط Docusaurus v3 ذات طابع مع طباعة مجددة و hero/cards مدفوعة بتدرجات وبلاطات موارد تضمن ملخص Norito Transmisión.
- ✅ تم توصيل اضافة Torii OpenAPI بامر `npm run sync-openapi` مع فحوصات لقطات موقعة وحمايات CSP يفرضها `buildSecurityHeaders`.
- ✅ Vista previa y sonda de CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) y inicio rápido SoraFS y inicios rápidos المراجعة المرجعية قبل نشر الحزم.
- ✅ Inicio rápido de inicios rápidos de Norito, SoraFS y SDK para obtener más información وتصل الاستيرادات الجديدة من `docs/source/` (streaming, orquestación, runbooks) هنا عند كتابتها.

## المشاركة

- راجع `docs/portal/README.md` لاوامر التطوير المحلي (`npm install`, `npm run start`, `npm run build`).
- تتم متابعة مهام ترحيل المحتوى جنبًا الى جنب مع عناصر roadmap `DOCS-*`. المساهمات مرحب بها - انقل اقساما من `docs/source/` واضف الصفحة الى الشريط الجانبي.
- اذا اضفت مخرجا مولدا (especificaciones, جداول config) , y وثق امر البناء ليتمكن المساهمون مستقبلا من تحديثه بسهولة.