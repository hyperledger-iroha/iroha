---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-bootstrap-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة nexus-bootstrap
العنوان: Bootstrap e observabilidade do Sora Nexus
الوصف: خطة تشغيلية لتجميع أو مجموعة أدوات التحقق المركزية Nexus عبر الإنترنت قبل إضافة الخدمات SoraFS و SoraNet.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تعكس `docs/source/soranexus_bootstrap_plan.md`. احتفظ بنسختين متصلتين من خلال النسخ المحلية من البوابة.
:::

# خطة التمهيد والملاحظة من Sora Nexus

##الأهداف
- Levantar a rede base de validadores/observadores Sora Nexus com chaves de الحاكم، APIs Torii ومراقبة التوافق.
- التحقق من صحة الخدمات المركزية (Torii، الإجماع، المثابرة) قبل التمكن من نشر SoraFS/SoraNet.
- ضبط سير عمل CI/CD ولوحات المعلومات/تنبيهات المراقبة لضمان السلامة.

## المتطلبات الأساسية
- تتوفر مواد الإدارة (متعددة النصائح، ورؤوس الالتزامات) في HSM أو Vault.
- قاعدة البنية التحتية (مجموعات Kubernetes أو المعادن العارية) في المناطق الأولية/الثانوية.
- تم تحديث إعدادات التمهيد (`configs/nexus/bootstrap/*.toml`) لتعكس المعلمات المتفق عليها مؤخرًا.

## بيئة ريدي
- قم بتشغيل البيئات Nexus مع بادئات الشبكة المميزة:
- ** Sora Nexus (mainnet)** - prefixo de rede de producao `nexus`, hospedando Governorca canonica e servicos piggyback SoraFS/SoraNet (معرف السلسلة `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - بادئة إعادة التدريج `testus`، تم إعدادها لتكوين الشبكة الرئيسية لاختبارات التكامل والتحقق من الإصدار المسبق (سلسلة UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- إدارة أرشيفات التكوين المنفصلة، ​​وأعمدة الإدارة، وآثار البنية التحتية لكل محيط. اختبرنا كمجال تجريبي للطرح SoraFS/SoraNet قبل الترويج لـ Nexus.
- يتم نشر خطوط أنابيب CI/CD لأول مرة في الاختبار وتنفيذ اختبارات الدخان تلقائيًا وإصدار دليل ترويجي لـ Nexus عند مرور عمليات التحقق.
- حزم التكوين المرجعي المباشر على `configs/soranexus/nexus/` (mainnet) و`configs/soranexus/testus/` (testnet)، كل منافس `config.toml`، `genesis.json` وأدلة القبول Torii على سبيل المثال.

## الخطوة 1 - مراجعة التكوين
1. توثيق المستندات الموجودة:
   - `docs/source/nexus/architecture.md` (بالتوافق، تخطيط Torii).
   - `docs/source/nexus/deployment_checklist.md` (متطلبات البنية التحتية).
   - `docs/source/nexus/governance_keys.md` (إجراءات الحراسة).
2. التحقق من أن ملف التكوين (`configs/nexus/genesis/*.json`) يتم تضمينه في القائمة الحالية للمصدقين وأموال التوقيع المساحي.
3. تأكيد معلمات الشبكة:
   - ضمان التوافق والنصاب القانوني.
   - الفاصل الزمني للكتل / الحدود النهائية.
   - منافذ الخدمة Torii وشهادات TLS.

## Etapa 2 - نشر نظام التشغيل العنقودي
1. توفير المصادقات:
   - نشر المثيلات `irohad` (المصادقات) مع وحدات التخزين المستمرة.
   - ضمان أن تسمح أنظمة جدار الحماية بحركة الموافقة وTorii بيننا.
2. ابدأ الخدمات Torii (REST/WebSocket) على كل مدقق عبر TLS.
3. نشر مراقبينا (في بعض الأحيان) لتعزيز المرونة.
4. تنفيذ البرامج النصية للتمهيد (`scripts/nexus_bootstrap.sh`) لتوزيع التكوين وبدء الموافقة والمسجل.
5. اختبارات الدخان التنفيذية:
   - أرسل تحويلات الاختبار عبر Torii (`iroha_cli tx submit`).
   - التحقق من إنتاج/إنهاء الكتل عبر القياس عن بعد.
   - قم بفحص النسخة المتماثلة من دفتر الأستاذ بين المصدقين / المراقبين.

## Etapa 3 - Governanca e gestao de chaves
1. Carregar configuracao multisig do conselho؛ نؤكد أن مقترحات الحوكمة يمكن أن تخضع للضوابط والتصديق عليها.
2. التخزين مع ضمان توافق الآراء/اللجنة؛ تكوين النسخ الاحتياطية التلقائية مع تسجيل الوصول.
3. تكوين إجراءات تدوير رؤوس الطوارئ (`docs/source/nexus/key_rotation.md`) والتحقق من دليل التشغيل.

## ايتابا 4 - Integracao CI/CD
1. تكوين خطوط الأنابيب:
   - إنشاء مدقق الصور المنشور/Torii (GitHub Actions أو GitLab CI).
   - Validacao automatizada de configuracao (lint de genesis, verificacao de assinaturas).
   - خطوط أنابيب النشر (Helm/Customize) لمجموعات التدريج والإنتاج.
2. تنفيذ اختبارات الدخان بدون CI (المجموعة الفرعية efemero، مجموعة قضيب canonica de transacoes).
3. إضافة نصوص برمجية للتراجع للنشر باستخدام دفاتر التشغيل الصحيحة وتوثيقها.

## الخطوة 5 - إمكانية المراقبة والتنبيهات
1. قم بنشر مكدس المراقبة (Prometheus + Grafana + Alertmanager) من أجل الموقع.
2. قياسات كوليتار المركزية:
  - `nexus_consensus_height`، `nexus_finality_lag`، `torii_request_duration_seconds`، `validator_peer_count`.
   - السجلات عبر Loki/ELK للخدمة Torii وبالتوافق.
3. لوحات المعلومات:
   - "سعود بالموافقة" (ارتفاع الكتلة، النهاية، حالة الأقران).
   - وقت الاستجابة وتصنيف الأخطاء في API Torii.
   - المعاملات الحكومية وحالة العروض.
4. التنبيهات:
   - إنتاج الكتل (> فاصلين زمنيين للكتلة).
   - لا يوجد أي عدد من أقرانهم بعد النصاب القانوني.
   - الصور الموجودة في فئة الخطأ Torii.
   - تراكم ملف مقترحات الإدارة.

## إيتابا 6 - التحقق من الصحة والتسليم
1. رودار فاليداكاو من البداية إلى النهاية:
   - Submeter proposta de Governoranca (على سبيل المثال mudanca de parametro).
   - معالجة المشورة لضمان عمل خط أنابيب الإدارة.
   - اختلاف حالة دفتر الأستاذ لضمان الاتساق.
2. توثيق أو دليل التشغيل عند الطلب (الاستجابة للحوادث وتجاوز الفشل والقياس).
3. Comunicar prontidao para المعدات SoraFS/SoraNet؛ تأكيد نشر podem على الظهر لـ nos Nexus.

## قائمة التحقق من التنفيذ
- [ ] مراجعة التكوين/التكوين الختامي.
- [ ] تم نشر المصدقين والمراقبين بتوافق الآراء.
- [ ] Chaves de goveranca carregadas، proposta testada.
- [ ] خطوط الأنابيب CI/CD Rodando (البناء + النشر + اختبارات الدخان).
- [ ] لوحات التحكم في إمكانية المراقبة مع التنبيهات.
- [ ] توثيق عملية التسليم يستغرق عدة مرات في اتجاه مجرى النهر.