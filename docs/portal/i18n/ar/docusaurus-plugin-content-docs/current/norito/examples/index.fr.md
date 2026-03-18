---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: الأمثلة Norito
الوصف: الإضافات Kotodama مختارة مع ساحة التسجيل.
سبيكة: /نوريتو/أمثلة
---

تعكس هذه الأمثلة عمليات التشغيل السريعة لـ SDK وساحات التسجيل. يمكنك بعد ذلك إعادة تجميع قائمة التحقق من التسجيل وإعادة توجيهها إلى أدلة Rust وPython وJavaScript حتى تتمكن من تجديد نفس سيناريو كل جولة.

- **[Squelette du point d'entrée Hajimari](./hajimari-entrypoint)** — الحد الأدنى من هيكل العقد Kotodama مع نقطة دخول عامة وإدارة حالة.
- **[تسجيل نطاق وإنشاء الأنشطة](./register-and-mint)** — إنشاء النطاقات باستخدام التفويضات وتسجيل الأنشطة وتحديد الإطار.
- **[Invoquer le Transfert hôte depuis Kotodama](./call-transfer-asset)** — يمكنك التعليق على نقطة إدخال Kotodama من خلال استدعاء التعليمات العالية `transfer_asset` مع التحقق من صحة البيانات عبر الإنترنت.
- **[نقل نشاط بين الحسابات](./transfer-asset)** — تدفق نقل الأنشطة البسيط الذي يعكس البدء السريع لـ SDK وساحات التسجيل.
- **[Frapper, transférer, and brûler un NFT](./nft-flow)** — قم بإيقاف دورة حياة NFT في نوبة: التقطيع إلى الملكية، والنقل، وإضافة المخلفات والتدمير.