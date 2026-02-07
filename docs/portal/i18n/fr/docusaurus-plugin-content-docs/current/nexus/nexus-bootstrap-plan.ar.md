---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan nexus-bootstrap
titre : اقلاع Sora Nexus والمراقبة
description : Utilisez le module d'extension Nexus pour SoraFS et SoraNet.
---

:::note المصدر القانوني
Il s'agit de la référence `docs/source/soranexus_bootstrap_plan.md`. ابق النسختين متوافقتين حتى تصل النسخ الموطنة الى البوابة.
:::

# خطة اقلاع ومراقبة Sora Nexus

## الاهداف
- تشغيل شبكة المدققين/المراقبين الاساسية لـ Sora Nexus avec مفاتيح الحوكمة وواجهات Torii ومراقبة اجماع.
- تحقق من الخدمات الاساسية (Torii, الاجماع، الاستمرارية) pour تمكين عمليات نشر SoraFS/SoraNet المتراكبة.
- Workflows de travail pour les applications CI/CD et les applications de type CI/CD.

## المتطلبات المسبقة
- مادة مفاتيح الحوكمة (multisig للمجلس، مفاتيح اللجنة) متاحة في HSM et Vault.
- Une utilisation en mode bare-metal (avec Kubernetes ou bare-metal) pour une utilisation/une installation en réseau.
- La méthode bootstrap (`configs/nexus/bootstrap/*.toml`) est utilisée pour les applications.## بيئات الشبكة
- تشغيل بيئتين لـ Nexus مع بادئات شبكة مختلفة:
- **Sora Nexus (mainnet)** - Utiliser le module `nexus` pour le réseau principal et SoraFS/SoraNet Description (ID de chaîne `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - Mise en scène `testus` pour le réseau principal et la chaîne UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- الحفاظ على ملفات genesis ومفاتيح حوكمة وبصمات بنية تحتية منفصلة لكل بيئة. Utilisez Testus pour les connexions SoraFS/SoraNet avec Nexus.
- يجب ان تنشر خطوط CI/CD الى Testus اولا وتنفذ smoke tests تلقائية وتطلب ترقية يدوية الى Nexus بعد نجاح الفحوصات.
- Vous pouvez utiliser `configs/soranexus/nexus/` (mainnet) et `configs/soranexus/testus/` (testnet) et `config.toml`. و`genesis.json` et Torii نموذجية.

## الخطوة 1 - مراجعة التكوين
1. تدقيق التوثيق الموجود:
   - `docs/source/nexus/architecture.md` (اجماع، تخطيط Torii).
   - `docs/source/nexus/deployment_checklist.md` (متطلبات البنية التحتية).
   - `docs/source/nexus/governance_keys.md` (اجراءات حفظ المفاتيح).
2. التحقق من ان ملفات genesis (`configs/nexus/genesis/*.json`) تتوافق مع roster الحالي واوزان jalonnement.
3. تاكيد معلمات الشبكة:
   - حجم لجنة الاجماع et quorum.
   - فاصل الكتل / عتبات finalité.
   - منافذ خدمة Torii et TLS.## الخطوة 2 - نشر عنقود bootstrap
1. تجهيز عقد المدققين:
   - نشر مثيلات `irohad` (مدققين) مع وحدات تخزين دائمة.
   - ضمان ان قواعد الجدار الناري تسمح بحركة مرور الاجماع و Torii byين العقد.
2. Utilisez le Torii (REST/WebSocket) pour utiliser TLS.
3. نشر عقد مراقبة (قراءة فقط) لمرونة اضافية.
4. Utilisez le bootstrap (`scripts/nexus_bootstrap.sh`) pour la genèse et le processus de démarrage.
5. Effectuer des tests de fumée :
   - ارسال معاملات اختبار عبر Torii (`iroha_cli tx submit`).
   - التحقق من انتاج/نهائية الكتل عبر التليمتري.
   - فحص تكرار السجل بين المدققين/المراقبين.

## الخطوة 3 - الحوكمة وادارة المفاتيح
1. تحميل تكوين multisig للمجلس; التاكد من امكانية ارسال واقرار مقترحات الحوكمة.
2. تخزين مفاتيح الاجماع/اللجنة بشكل امن ; اعداد نسخ احتياطية تلقائية مع تسجيل وصول.
3. Utilisez le runbook pour ajouter le runbook (`docs/source/nexus/key_rotation.md`).

## الخطوة 4 - Télécharger CI/CD
1. تكوين خطوط الانابيب:
   - Il s'agit du validateur/Torii (Actions GitHub et GitLab CI).
   - التحقق التلقائي من التكوين (lint لـ Genesis, تحقق من التواقيع).
   - خطوط نشر (Helm/Kustomize) pour la mise en scène et la mise en scène.
2. Effectuer des tests de fumée pour CI (تشغيل عنقود مؤقت وتشغيل مجموعة المعاملات القانونية).
3. Effectuer la restauration des runbooks.## الخطوة 5 - المراقبة والتنبيهات
1. Utilisez le gestionnaire d'alerte (Prometheus + Grafana + Alertmanager) pour le faire.
2. جمع المقاييس الاساسية:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Les personnages de Loki/ELK sont Torii et.
3. لوحات التحكم:
   - صحة الاجماع (ارتفاع الكتلة، النهائية، حالة pairs).
   - Utilisez l'API Torii pour les applications.
   - معاملات الحوكمة وحالة المقترحات.
4. التنبيهات:
   - توقف انتاج الكتل (>2 فواصل كتل).
   - هبوط عدد pairs تحت quorum.
   - ارتفاع معدل اخطاء Torii.
   - تراكم طابور مقترحات الحوكمة.

## الخطوة 6 - التحقق والتسليم
1. تنفيذ تحقق de bout en bout :
   - ارسال مقترح حوكمة (مثل تغيير معلمة).
   - تمريره عبر موافقة المجلس لضمان عمل خط الحوكمة.
   - تنفيذ diff لحالة السجل لضمان الاتساق.
2. Utiliser le runbook pour la mise à l'échelle (basculement et mise à l'échelle).
3. ابلاغ فرق SoraFS/SoraNet بالجاهزية; Il s'agit d'un ferroutage sur Nexus.

## قائمة تنفيذ
- [ ] تدقيق genèse/configuration مكتمل.
- [ ] نشر عقد المدققين والمراقبين مع اجماع سليم.
- [ ] تحميل مفاتيح الحوكمة واختبار المقترح.
- [ ] خطوط CI/CD تعمل (build + déploiement + smoke tests).
- [ ] لوحات المراقبة تعمل مع التنبيهات.
- [ ] تسليم توثيق handoff للفرق en aval.