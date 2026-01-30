---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 49986499dd4c83fa25f2e30fe16f06d73553ac3cb3ef769e7bd3fcf36955a8d5
source_last_modified: "2025-11-14T04:43:20.258005+00:00"
translation_last_reviewed: 2026-01-30
---

# مرحبا بكم في بوابة مطوري SORA Nexus

تجمع بوابة مطوري SORA Nexus وثائق تفاعلية ودروس SDK ومراجع API لمشغلي Nexus ومساهمي Hyperledger Iroha. تكمل موقع الوثائق الرئيسي عبر ابراز الادلة العملية والمواصفات المولدة مباشرة من هذا المستودع. صفحة الهبوط تحتوي الان على نقاط دخول ذات طابع Norito/SoraFS، ولقطات OpenAPI موقعة، ومرجع مخصص لـ Norito Streaming حتى يتمكن المساهمون من العثور على عقد طبقة التحكم للبث دون التنقيب في المواصفة الجذرية.

## ما الذي يمكنك فعله هنا

- **تعلم Norito** - ابدأ بنظرة عامة و quickstart لفهم نموذج التسلسل وادوات bytecode.
- **اطلاق SDKs** - اتبع quickstarts ل JavaScript و Rust اليوم؛ ستنضم ادلة Python و Swift و Android مع ترحيل الوصفات.
- **تصفح مراجع API** - تعرض صفحة Torii OpenAPI احدث مواصفات REST، وترتبط جداول الاعدادات بمصادر Markdown القياسية.
- **تحضير النشر** - يتم ترحيل كتيبات التشغيل (telemetry, settlement, Nexus overlays) من `docs/source/` وستصل الى هذا الموقع مع تقدم الترحيل.

## الحالة الحالية

- ✅ صفحة هبوط Docusaurus v3 ذات طابع مع طباعة مجددة و hero/cards مدفوعة بتدرجات وبلاطات موارد تتضمن ملخص Norito Streaming.
- ✅ تم توصيل اضافة Torii OpenAPI بامر `npm run sync-openapi` مع فحوصات لقطات موقعة وحمايات CSP يفرضها `buildSecurityHeaders`.
- ✅ تشغيل تغطية preview و probe في CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) والتي باتت تضع بوابة لوثيقة streaming و quickstarts SoraFS وقوائم المراجعة المرجعية قبل نشر الحزم.
- ✅ اصبحت quickstarts الخاصة بـ Norito و SoraFS و SDKs مع اقسام المرجع متاحة في الشريط الجانبي؛ وتصل الاستيرادات الجديدة من `docs/source/` (streaming, orchestration, runbooks) هنا عند كتابتها.

## المشاركة

- راجع `docs/portal/README.md` لاوامر التطوير المحلي (`npm install`, `npm run start`, `npm run build`).
- تتم متابعة مهام ترحيل المحتوى جنبًا الى جنب مع عناصر roadmap `DOCS-*`. المساهمات مرحب بها - انقل اقساما من `docs/source/` واضف الصفحة الى الشريط الجانبي.
- اذا اضفت مخرجا مولدا (specs، جداول config)، وثق امر البناء ليتمكن المساهمون مستقبلا من تحديثه بسهولة.
