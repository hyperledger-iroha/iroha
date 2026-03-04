---
lang: fr
direction: ltr
source: docs/portal/docs/intro.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مرحبا بكم في بوابة مطوري SORA Nexus

Utilisez le SDK et l'API Nexus pour SORA Nexus. Il s'agit de Hyperledger Iroha. تكمل موقع الوثائق الرئيسي عبر ابراز الادلة العملية والمواصفات المولدة مباشرة من هذا المستودع. صفحة الهبوط تحتوي الان على نقاط دخول ذات طابع Norito/SoraFS, ولقطات OpenAPI موقعة، ومرجع مخصص لـ Norito Streaming حتى يتمكن المساهمون من العثور على عقد طبقة التحكم للبث دون التنقيب في المواصفة الجذرية.

## ما الذي يمكنك فعله هنا

- **تعلم Norito** - ابدأ بنظرة عامة et quickstart pour le bytecode.
- **Kits SDK** - Démarrages rapides pour JavaScript et Rust Il existe des applications Python, Swift et Android.
- **Application API** - Application Torii OpenAPI pour REST et pour Markdown القياسية.
- **تحضير النشر** - يتم ترحيل كتيبات التشغيل (télémétrie, règlement, superpositions Nexus) et `docs/source/` وستصل الى هذا الموقع مع تقدم الترحيل.

## الحالة الحالية- ✅ صفحة هبوط Docusaurus v3 ذات طابع مع طباعة مجددة et hero/cards مدفوعة بتدرجات وبلاطات موارد تتضمن ملخص Norito Streaming.
- ✅ تم توصيل اضافة Torii OpenAPI pour `npm run sync-openapi` pour les CSP يفرضها `buildSecurityHeaders`.
- ✅ Aperçu de l'aperçu et sonde pour CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) et démarrage rapide du streaming et démarrage rapide SoraFS وقوائم المراجعة المرجعية قبل نشر الحزم.
- ✅ Démarrages rapides pour Norito et SoraFS et SDK pour plus de détails. Les informations fournies par `docs/source/` (streaming, orchestration, runbooks) sont disponibles.

## المشاركة

- راجع `docs/portal/README.md` pour le téléchargement (`npm install`, `npm run start`, `npm run build`).
- تتم متابعة مهام ترحيل المحتوى جنبًا الى جنب مع عناصر roadmap `DOCS-*`. المساهمات مرحب بها - انقل اقساما من `docs/source/` واضف الصفحة الى الشريط الجانبي.
- اذا اضفت مخرجا مولدا (specs, جداول config) et وثق امر البناء ليتمكن المساهمون مستقبلا من تحديثه بسهولة.