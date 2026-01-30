---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/nexus/nexus-bootstrap-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b913e65830d348d0b5480d7958427f9b1d23de2fc33de0e4786c17d330e054b5
source_last_modified: "2025-11-14T04:43:20.366126+00:00"
translation_last_reviewed: 2026-01-30
---

:::note المصدر القانوني
تعكس هذه الصفحة `docs/source/soranexus_bootstrap_plan.md`. ابق النسختين متوافقتين حتى تصل النسخ الموطنة الى البوابة.
:::

# خطة اقلاع ومراقبة Sora Nexus

## الاهداف
- تشغيل شبكة المدققين/المراقبين الاساسية لـ Sora Nexus مع مفاتيح الحوكمة وواجهات Torii ومراقبة اجماع.
- التحقق من الخدمات الاساسية (Torii، الاجماع، الاستمرارية) قبل تمكين عمليات نشر SoraFS/SoraNet المتراكبة.
- تاسيس workflows لـ CI/CD ولوحات/تنبيهات المراقبة لضمان صحة الشبكة.

## المتطلبات المسبقة
- مادة مفاتيح الحوكمة (multisig للمجلس، مفاتيح اللجنة) متاحة في HSM او Vault.
- بنية تحتية اساسية (عناقيد Kubernetes او عقد bare-metal) في مناطق اساسية/ثانوية.
- تكوين bootstrap محدث (`configs/nexus/bootstrap/*.toml`) يعكس احدث معلمات الاجماع.

## بيئات الشبكة
- تشغيل بيئتين لـ Nexus مع بادئات شبكة مختلفة:
- **Sora Nexus (mainnet)** - بادئة شبكة انتاج `nexus` تستضيف الحوكمة القانونية وخدمات SoraFS/SoraNet المتراكبة (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - بادئة شبكة staging `testus` تعكس تكوين mainnet لاختبارات التكامل والتحقق قبل الاصدار (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- الحفاظ على ملفات genesis ومفاتيح حوكمة وبصمات بنية تحتية منفصلة لكل بيئة. تعمل Testus كساحة اثبات لكل عمليات نشر SoraFS/SoraNet قبل الترقية الى Nexus.
- يجب ان تنشر خطوط CI/CD الى Testus اولا وتنفذ smoke tests تلقائية وتطلب ترقية يدوية الى Nexus بعد نجاح الفحوصات.
- حزم التكوين المرجعية موجودة تحت `configs/soranexus/nexus/` (mainnet) و `configs/soranexus/testus/` (testnet)، وكل منها يحتوي `config.toml` و`genesis.json` وادلة قبول Torii نموذجية.

## الخطوة 1 - مراجعة التكوين
1. تدقيق التوثيق الموجود:
   - `docs/source/nexus/architecture.md` (الاجماع، تخطيط Torii).
   - `docs/source/nexus/deployment_checklist.md` (متطلبات البنية التحتية).
   - `docs/source/nexus/governance_keys.md` (اجراءات حفظ المفاتيح).
2. التحقق من ان ملفات genesis (`configs/nexus/genesis/*.json`) تتوافق مع roster المدققين الحالي واوزان staking.
3. تاكيد معلمات الشبكة:
   - حجم لجنة الاجماع و quorum.
   - فاصل الكتل / عتبات finality.
   - منافذ خدمة Torii وشهادات TLS.

## الخطوة 2 - نشر عنقود bootstrap
1. تجهيز عقد المدققين:
   - نشر مثيلات `irohad` (مدققين) مع وحدات تخزين دائمة.
   - ضمان ان قواعد الجدار الناري تسمح بحركة مرور الاجماع و Torii بين العقد.
2. تشغيل خدمات Torii (REST/WebSocket) على كل مدقق باستخدام TLS.
3. نشر عقد مراقبة (قراءة فقط) لمرونة اضافية.
4. تشغيل سكربتات bootstrap (`scripts/nexus_bootstrap.sh`) لتوزيع genesis وبدء الاجماع وتسجيل العقد.
5. تنفيذ smoke tests:
   - ارسال معاملات اختبار عبر Torii (`iroha_cli tx submit`).
   - التحقق من انتاج/نهائية الكتل عبر التليمتري.
   - فحص تكرار السجل بين المدققين/المراقبين.

## الخطوة 3 - الحوكمة وادارة المفاتيح
1. تحميل تكوين multisig للمجلس; التاكد من امكانية ارسال واقرار مقترحات الحوكمة.
2. تخزين مفاتيح الاجماع/اللجنة بشكل امن; اعداد نسخ احتياطية تلقائية مع تسجيل وصول.
3. اعداد اجراءات تدوير مفاتيح الطوارئ (`docs/source/nexus/key_rotation.md`) والتحقق من runbook.

## الخطوة 4 - تكامل CI/CD
1. تكوين خطوط الانابيب:
   - بناء ونشر صور validator/Torii (GitHub Actions او GitLab CI).
   - التحقق التلقائي من التكوين (lint لـ genesis، تحقق من التواقيع).
   - خطوط نشر (Helm/Kustomize) لعناقيد staging والانتاج.
2. تنفيذ smoke tests في CI (تشغيل عنقود مؤقت وتشغيل مجموعة المعاملات القانونية).
3. اضافة سكربتات rollback لنشر فاشل وتوثيق runbooks.

## الخطوة 5 - المراقبة والتنبيهات
1. نشر حزمة المراقبة (Prometheus + Grafana + Alertmanager) لكل منطقة.
2. جمع المقاييس الاساسية:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - السجلات عبر Loki/ELK لخدمات Torii والاجماع.
3. لوحات التحكم:
   - صحة الاجماع (ارتفاع الكتلة، النهائية، حالة peers).
   - زمن تاخر Torii API ومعدلات الاخطاء.
   - معاملات الحوكمة وحالة المقترحات.
4. التنبيهات:
   - توقف انتاج الكتل (>2 فواصل كتل).
   - هبوط عدد peers تحت quorum.
   - ارتفاع معدل اخطاء Torii.
   - تراكم طابور مقترحات الحوكمة.

## الخطوة 6 - التحقق والتسليم
1. تنفيذ تحقق end-to-end:
   - ارسال مقترح حوكمة (مثل تغيير معلمة).
   - تمريره عبر موافقة المجلس لضمان عمل خط الحوكمة.
   - تنفيذ diff لحالة السجل لضمان الاتساق.
2. توثيق runbook للمناوبين (استجابة الحوادث، failover، scaling).
3. ابلاغ فرق SoraFS/SoraNet بالجاهزية; تاكيد ان نشر piggyback يمكنه الاشارة الى عقد Nexus.

## قائمة تنفيذ
- [ ] تدقيق genesis/configuration مكتمل.
- [ ] نشر عقد المدققين والمراقبين مع اجماع سليم.
- [ ] تحميل مفاتيح الحوكمة واختبار المقترح.
- [ ] خطوط CI/CD تعمل (build + deploy + smoke tests).
- [ ] لوحات المراقبة تعمل مع التنبيهات.
- [ ] تسليم توثيق handoff للفرق downstream.
