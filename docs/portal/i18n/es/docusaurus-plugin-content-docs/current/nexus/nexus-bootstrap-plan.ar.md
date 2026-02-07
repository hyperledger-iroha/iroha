---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-bootstrap-nexus
título: اقلاع Sora Nexus والمراقبة
descripción: خطة تشغيلية لتشغيل عنقود المدققين الاساسي لـ Nexus قبل اضافة خدمات SoraFS و SoraNet.
---

:::nota المصدر القانوني
Utilice el código `docs/source/soranexus_bootstrap_plan.md`. ابق النسختين متوافقتين حتى تصل النسخ الموطنة الى البوابة.
:::

# خطة اقلاع ومراقبة Sora Nexus

## الاهداف
- تشغيل شبكة المدققين/المراقبين الاساسية لـ Sora Nexus مع مفاتيح الحوكمة y Torii y مراقبة اجماع.
- التحقق من الخدمات الاساسية (Torii، الاجماع، الاستمرارية) قبل تمكين عمليات نشر SoraFS/SoraNet المتراكبة.
- Flujos de trabajo de تاسيس لـ CI/CD ولوحات/تنبيهات المراقبة لضمان صحة الشبكة.

## المتطلبات المسبقة
- مادة مفاتيح الحوكمة (multisig للمجلس، مفاتيح اللجنة) متاحة في HSM y Vault.
- بنية تحتية اساسية (عناقيد Kubernetes او عقد bare-metal) في مناطق اساسية/ثانوية.
- تكوين bootstrap محدث (`configs/nexus/bootstrap/*.toml`) يعكس احدث معلمات الاجماع.## بيئات الشبكة
- Haga clic en el botón Nexus para obtener el resultado deseado:
- **Sora Nexus (mainnet)** - بادئة شبكة انتاج `nexus` تستضيف الحوكمة القانونية y خدمات SoraFS/SoraNet المتراكبة (ID de cadena `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - بادئة شبكة staging `testus` تعكس تكوين mainnet لاختبارات التكامل والتحقق قبل الاصدار (UUID cadena `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- الحفاظ على ملفات genesis ومفاتيح حوكمة وبصمات بنية تحتية منفصلة لكل بيئة. Utilice Testus para conectar archivos SoraFS/SoraNet a Nexus.
- Utilice CI/CD Testus y pruebas de humo para probar Nexus.
- حزم التكوين المرجعية موجودة تحت `configs/soranexus/nexus/` (mainnet) y `configs/soranexus/testus/` (testnet) y `config.toml`. و`genesis.json` y قبول Torii نموذجية.

## الخطوة 1 - مراجعة التكوين
1. تدقيق التوثيق الموجود:
   - `docs/source/nexus/architecture.md` (الاجماع، تخطيط Torii).
   - `docs/source/nexus/deployment_checklist.md` (متطلبات البنية التحتية).
   - `docs/source/nexus/governance_keys.md` (اجراءات حفظ المفاتيح).
2. التحقق من ان ملفات genesis (`configs/nexus/genesis/*.json`) تتوافق مع roster المدققين الحالي واوزان replanteo.
3. تاكيد معلمات الشبكة:
   - حجم لجنة الاجماع y quórum.
   - فاصل الكتل / عتبات finalidad.
   - Adaptador Torii y TLS.## الخطوة 2 - نشر عنقود bootstrap
1. تجهيز عقد المدققين:
   - Introduzca el código `irohad` (módulo) para que funcione correctamente.
   - ضمان ان قواعد الجدار الناري تسمح بحركة مرور الاجماع و Torii بين العقد.
2. Instale Torii (REST/WebSocket) en el servidor TLS.
3. نشر عقد مراقبة (قراءة فقط) لمرونة اضافية.
4. Utilice el bootstrap (`scripts/nexus_bootstrap.sh`) para iniciar la génesis y el inicio del proceso.
5. Pruebas de humo siguientes:
   - ارسال معاملات اختبار عبر Torii (`iroha_cli tx submit`).
   - التحقق من انتاج/نهائية الكتل عبر التليمتري.
   - فحص تكرار السجل بين المدققين/المراقبين.

## الخطوة 3 - الحوكمة وادارة المفاتيح
1. تحميل تكوين multisig للمجلس; التاكد من امكانية ارسال واقرار مقترحات الحوكمة.
2. تخزين مفاتيح الاجماع/اللجنة بشكل امن; اعداد نسخ احتياطية تلقائية مع تسجيل وصول.
3. اعداد اجراءات تدوير مفاتيح الطوارئ (`docs/source/nexus/key_rotation.md`) y من runbook.

## Paso 4 - Ajuste CI/CD
1. تكوين خطوط الانابيب:
   - Validador de datos/Torii (GitHub Actions y GitLab CI).
   - التحقق التلقائي من التكوين (pelusa de génesis, تحقق من التواقيع).
   - خطوط نشر (Helm/Kustomize) لعناقيد puesta en escena y الانتاج.
2. تنفيذ pruebas de humo في CI (تشغيل عنقود مؤقت وتشغيل مجموعة المعاملات القانونية).
3. Haga una reversión de los runbooks.## الخطوة 5 - المراقبة والتنبيهات
1. Haga clic en el botón (Prometheus + Grafana + Alertmanager) para activarlo.
2. جمع المقاييس الاساسية:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - السجلات عبر Loki/ELK لخدمات Torii والاجماع.
3. لوحات التحكم:
   - صحة الاجماع (ارتفاع الكتلة، النهائية، حالة pares).
   - Utilice la API Torii y sus aplicaciones.
   - معاملات الحوكمة وحالة المقترحات.
4. التنبيهات:
   - توقف انتاج الكتل (>2 فواصل كتل).
   - هبوط عدد pares تحت quórum.
   - ارتفاع معدل اخطاء Torii.
   - تراكم طابور مقترحات الحوكمة.

## الخطوة 6 - التحقق والتسليم
1. تنفيذ تحقق de un extremo a otro:
   - ارسال مقترح حوكمة (مثل تغيير معلمة).
   - تمريره عبر موافقة المجلس لضمان عمل خط الحوكمة.
   - تنفيذ diff لحالة السجل لضمان الاتساق.
2. توثيق runbook للمناوبين (conmutación por error, escalado).
3. ابلاغ فرق SoraFS/SoraNet بالجاهزية; تاكيد ان نشر a cuestas يمكنه الاشارة الى عقد Nexus.

## قائمة تنفيذ
- [ ] تدقيق génesis/configuración مكتمل.
- [ ] نشر عقد المدققين والمراقبين مع اجماع سليم.
- [ ] تحميل مفاتيح الحوكمة واختبار المقترح.
- [] Actualización de CI/CD (compilación + implementación + pruebas de humo).
- [ ] لوحات المراقبة تعمل مع التنبيهات.
- [] تسليم توثيق traspaso للفرق aguas abajo.