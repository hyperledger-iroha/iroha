---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-routed-trace-audit-2026q1
العنوان: Relatorio de Auditoria توجيه التتبع 2026 Q1 (B1)
الوصف: Espelho de `docs/source/nexus_routed_trace_audit_report_2026q1.md`، cobrindo os resultados Trimestrais das telemetria revisoes.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::ملاحظة فونتي كانونيكا
هذه الصفحة تعكس `docs/source/nexus_routed_trace_audit_report_2026q1.md`. احتفظ بنسختين من النسخ المكملة كما هو الحال مع الترجمة الباقية.
:::

# نسبة تتبع التوجيه للربع الأول من عام 2026 (B1)

العنصر الخاص بخريطة الطريق **B1 - عمليات تدقيق التتبع الموجهة وخط الأساس للقياس عن بعد** يتطلب مراجعة ثلاثية لبرنامج التتبع الموجه لـ Nexus. هذه الوثيقة المتعلقة بسجل مستمعي الربع الأول من عام 2026 (يناير-ماركو) لكي يتمكن مستشارو الإدارة من اتخاذ وضعية القياس عن بعد قبل إجراء التجارب في الربع الثاني.

## إسكوبو إي كرونوجراما

| معرف التتبع | جانيلا (التوقيت العالمي المنسق) | الهدف |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | التحقق من الرسوم البيانية لدخول الممرات وأحاديث الأسرة وتدفق التنبيهات قبل تأهيل الممرات المتعددة. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | التحقق من صحة إعادة تشغيل OTLP، ومقارنة الروبوتات المختلفة، واستيعاب القياس عن بعد لـ SDK قبل علامتي AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | قم بتأكيد دلتا `iroha_config` من خلال الإدارة وإيقاف التراجع قبل قطع RC1. |

يتم إنتاج كل قطعة من الطبولوجيا باستخدام أداة التتبع التوجيهي (جهاز القياس عن بعد `nexus.audit.outcome` + أجهزة القياس Prometheus)، كما يتم إعادة توجيه مدير التنبيهات والأدلة المصدرة لـ `docs/examples/`.

## الميتودولوجيا

1. **مجموعة القياس عن بعد.** جميعنا نصدر الحدث الذي تم تصميمه `nexus.audit.outcome` والمقاييس المرتبطة به (`nexus_audit_outcome_total*`). يا مساعد `scripts/telemetry/check_nexus_audit_outcome.py` قم بتسجيل JSON، وتحقق من حالة الحدث واحفظ الحمولة في `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **التحقق من صحة التنبيهات.** `dashboards/alerts/nexus_audit_rules.yml` وتضمن أدوات الاختبار الخاصة بك أن تكون حدود الحمولة وقوالب الحمولة متسقة دائمًا. قم بتنفيذ CI `dashboards/alerts/tests/nexus_audit_rules.test.yml` على كل حال؛ كإجراء روتيني للتمارين اليدوية خلال كل مرة.
3. **التقاط لوحات المعلومات.** يقوم المشغلون بتصدير التتبع الموجه لـ `dashboards/grafana/soranet_sn16_handshake.json` (صوت المصافحة) ولوحات المعلومات العامة للقياس عن بعد لربط جميع الملفات بنتائج الاستماع.
4. **ملاحظات المراجعة.** سكرتارية الإدارة تسجل بدء المراجعة والقرار وتذاكر التخفيف في [Nexus ملاحظات الانتقال](./nexus-transition-notes) ولا تتعقب دلتا التكوين (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

##اخادوس

| معرف التتبع | النتيجة | ايفيدنسيا | نوتاس |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | تمرير | لقطات التنبيه/الاسترداد (الرابط الداخلي) + إعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; تختلف القياسات عن بعد المسجلة في [Nexus ملاحظات الانتقال](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 تم قبول الملف بشكل دائم لمدة 612 مللي ثانية (الطول <=750 مللي ثانية). متابعة سم. |
| `TRACE-TELEMETRY-BRIDGE` | تمرير | تم أرشفة الحمولة `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` بالإضافة إلى تجزئة إعادة تشغيل OTLP المسجلة في `status.md`. | تعمل أملاح التنقيح على SDK على أساس Rust؛ o تقرير الروبوت المختلف يشير إلى صفر دلتا. |
| `TRACE-CONFIG-DELTA` | تمرير (التخفيف مغلق) | أدخل متتبع الإدارة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + بيان ملف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + بيان حزمة القياس عن بعد (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | إعادة تشغيل تجزئة الربع الثاني أو تحسين TLS وتأكيد عدم وجود أي متطرفين؛ o بيان تسجيل القياس عن بعد أو الفاصل الزمني للفتحات 912-936 e o ​​بذرة عبء العمل `NEXUS-REH-2026Q2`. |

يتم إنتاج جميع الآثار في حدث أقل من `nexus.audit.outcome` في خزاناتك، مما يرضي حواجز الحماية الخاصة بـ Alertmanager (`NexusAuditOutcomeFailure` بشكل دائم في الأشهر الثلاثة الأخيرة).

## المتابعات

- تم تحديث ملحق التتبع الموجه مع تجزئة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`؛ تم حذف `NEXUS-421` من الملاحظات الانتقالية.
- استمر في إضافة عمليات إعادة تشغيل OTLP بروتوس وفرق الاختلاف Torii إلى الملف لإصلاح أدلة الإثبات لمراجعة Android AND4/AND7.
- تأكد من إعادة استخدام البروفات التالية `TRACE-MULTILANE-CANARY` أو نفس مساعد القياس عن بعد حتى يستفيد تسجيل الخروج من الربع الثاني من صحة سير العمل.

## مؤشر التحف

| أتيفو | محلي |
|-------|----------|
| التحقق من القياس عن بعد | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regras e testes de notifications | `dashboards/alerts/nexus_audit_rules.yml`، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| الحمولة الناتجة عن المثال | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de delta de configuracao | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronograma e notas توجيه التتبع | [ملاحظات الانتقال Nexus](./nexus-transition-notes) |

هذا الصدد، يجب أن يتم إرفاق العناصر العليا وصادرات التنبيهات/القياس عن بعد بسجل قرار الإدارة لجلب أو B1 إلى الثلث.