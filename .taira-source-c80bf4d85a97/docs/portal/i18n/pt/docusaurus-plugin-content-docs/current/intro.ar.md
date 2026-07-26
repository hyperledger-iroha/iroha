---
lang: pt
direction: ltr
source: docs/portal/docs/intro.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مرحبا بكم في بوابة مطوري SORA Nexus

Obtenha o SORA Nexus e instale o SDK e o SDK e a API Nexus E Hyperledger Iroha. تكمل موقع الوثائق الرئيسي عبر ابراز الادلة العملية والمواصفات المولدة مباشرة من هذا المستودع. A solução de problemas de hardware é Norito/SoraFS, e OpenAPI موقعة, ومرجع مخصص لـ Norito Streaming حتى يتمكن المساهمون من العثور على عقد طبقة التحكم للبث Eu não estou no lugar certo.

## ما الذي يمكنك فعله هنا

- **تعلم Norito** - ابدأ بنظرة عامة e quickstart لفهم نموذج التسلسل وادوات bytecode.
- **Como SDKs** - Como obter guias de início rápido para JavaScript e Rust É melhor usar Python, Swift e Android.
- **تصفح مراجع API** - تعرض صفحة Torii OpenAPI احدث مواصفات REST, وترتبط جداول O Markdown está disponível para download.
- **تحضير النشر** - يتم ترحيل كتيبات التشغيل (telemetria, liquidação, sobreposições Nexus) com `docs/source/` وستصل الى هذا الموقع مع تقدم الترحيل.

## الحالة الحالية

- ✅ صفحة هبوط Docusaurus v3 ذات طابع مع طباعة مجددة و hero/cards مدفوعة بتدرجات وبلاطات موارد تتضمن ملخص Norito Streaming.
- ✅ تم توصيل اضافة Torii OpenAPI بامر `npm run sync-openapi` مع فحوصات لقطات موقعة وحمايات CSP `buildSecurityHeaders`.
- ✅ Baixe a visualização e teste do CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) e baixe o streaming e os guias de início rápido SoraFS é um problema que está acontecendo.
- ✅ اصبحت quickstarts الخاصة بـ Norito و SoraFS و SDKs مع اقسام المرجع متاحة في الشريط الجانبي؛ وتصل الاستيرادات الجديدة `docs/source/` (streaming, orquestração, runbooks) está aqui.

## المشاركة

- راجع `docs/portal/README.md` لاوامر التطوير المحلي (`npm install`, `npm run start`, `npm run build`).
- تتم متابعة مهام ترحيل المحتوى جنبًا الى جنب مع عناصر roteiro `DOCS-*`. O código de barras está localizado em `docs/source/`.
- اذا اضفت مخرجا مولدا (especificações, configuração de configuração), وثق امر البناء ليتمكن المساهمون مستقبلا من تحديثه Bem.