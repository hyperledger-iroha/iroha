---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-routed-trace-audit-2026q1
العنوان: إعلام التتبع التوجيهي لعام 2026، الربع الأول (B1)
الوصف: شاهد `docs/source/nexus_routed_trace_audit_report_2026q1.md`، والذي يغطي نتائج المراجعة الثلاثية للقياس عن بعد.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/nexus_routed_trace_audit_report_2026q1.md`. احتفظ بنسخ من النسخ المجانية حتى تتمكن من قراءة التراخيص المتبقية.
:::

# إعلام مستمعي التتبع التوجيهي لعام 2026، الربع الأول (B1)

عنصر خريطة الطريق **B1 - عمليات تدقيق التتبع الموجهة وخط الأساس للقياس عن بعد** يتطلب مراجعة ثلاثية لبرنامج التتبع التوجيهي لـ Nexus. تم إبلاغنا بذلك بتوثيق نافذة قاعة الاجتماعات للربع الأول من عام 2026 (اللون الأسود) حتى يتمكن مجلس الإدارة من ضبط وضع القياس عن بعد قبل إجراء اختبارات التفريغ للربع الثاني.

## Alcance y Linea de Timepo

| معرف التتبع | فينتانا (التوقيت العالمي المنسق) | الهدف |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | التحقق من الرسوم البيانية للدخول إلى الممرات والأحاديث حول الكولا وتدفق التنبيهات قبل تأهيل الممرات المتعددة. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | التحقق من إعادة تشغيل OTLP، ومساواة الروبوتات المختلفة، واستيعاب القياس عن بعد لـ SDK قبل الوصول إلى AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | قم بتأكيد نطاقات `iroha_config` المعتمدة من أجل الإدارة والإعداد للاستعادة قبل قطع RC1. |

يتم تنفيذ كل نوع من الطوبولوجيا باستخدام أداة التتبع الموجهة (جهاز القياس عن بعد `nexus.audit.outcome` + وحدات التحكم Prometheus)، وضبط مدير التنبيهات والأدلة المصدرة في `docs/examples/`.

## الميتودولوجيا

1. **استرجاع القياس عن بعد.** جميع العقد تبعث الحدث المصمم `nexus.audit.outcome` والمقاييس المصاحبة (`nexus_audit_outcome_total*`). المساعد `scripts/telemetry/check_nexus_audit_outcome.py` يضغط على ذيل سجل JSON ويصحح حالة الحدث ويحفظ الحمولة في `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **التحقق من صحة التنبيهات.** `dashboards/alerts/nexus_audit_rules.yml` ويتم التأكد من ثبات مظلات إتلاف التنبيهات ونموذج الحمولة. يقوم CI بإخراج `dashboards/alerts/tests/nexus_audit_rules.test.yml` في كل تغيير؛ يتم تشغيل القواعد نفسها يدويًا خلال كل نافذة.
3. **التقاط لوحات المعلومات.** يقوم المشغلون بتصدير اللوحات التي تم توجيهها لـ `dashboards/grafana/soranet_sn16_handshake.json` (حالة المصافحة) ولوحات القياس العامة عن بعد لربط صحة الكولا بنتائج الاستماع.
4. **ملاحظات المراجعات.** سكرتارية إدارة السجل الأولي للمراجعات والقرارات وبطاقات التخفيف في [Nexus مذكرات الانتقال](./nexus-transition-notes) ومتتبع بيانات التكوين (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## هالازغوس

| معرف التتبع | النتيجة | ايفيدنسيا | نوتاس |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | تمرير | التقاطات تنبيه الحريق/الاسترداد (الداخلية) + إعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; تختلف القياسات عن بعد المسجلة في [Nexus ملاحظات الانتقال](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | يتم تشغيل P95 من إدخال الكولا في 612 مللي ثانية (الهدف <=750 مللي ثانية). لا يتطلب الأمر المتابعة. |
| `TRACE-TELEMETRY-BRIDGE` | تمرير | أرشيف الحمولة `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` مع تجزئة إعادة تشغيل OTLP المسجلة في `status.md`. | تتزامن أملاح تنقيح SDK مع قاعدة Rust؛ el diff bot reporto cero deltas. |
| `TRACE-CONFIG-DELTA` | تمرير (التخفيف مغلق) | أدخل إلى متتبع الحكومة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + بيان ملف تعريف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + بيان حزمة القياس عن بعد (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | تمت الموافقة على إعادة تشغيل Q2 لملف TLS وتأكيد التغييرات النهائية؛ سجل بيان القياس عن بعد سلسلة الفتحات 912-936 ومصدر عبء العمل `NEXUS-REH-2026Q2`. |

تنتج جميع الآثار حدثًا على الأقل `nexus.audit.outcome` داخل نوافذك، مما يرضي حواجز الحماية الخاصة بـ Alertmanager (`NexusAuditOutcomeFailure` يتم الحفاظ عليها خضراء خلال الأشهر الثلاثة الأخيرة).

## المتابعات

- قم بتحديث ملحق التتبع الموجه باستخدام التجزئة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`؛ يتم إجراء التخفيف `NEXUS-421` في ملاحظات الانتقال.
- استمر في إعادة تشغيل OTLP بدون معالجة وعناصر الاختلاف في Torii إلى الأرشيف لإصلاح أدلة الإثبات لمراجعات Android AND4/AND7.
- تأكد من أن التدريبات القريبة `TRACE-MULTILANE-CANARY` تستخدم نفس مساعد القياس عن بعد حتى يستفيد تسجيل الخروج من الربع الثاني من التدفق الصحيح.

## فهرس المصنوعات اليدوية

| اكتيف | أوبيكاسيون |
|-------|----------|
| التحقق من القياس عن بعد | `scripts/telemetry/check_nexus_audit_outcome.py` |
| إعدادات واختبارات التنبيهات | `dashboards/alerts/nexus_audit_rules.yml`، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| حمولة نتائج المثال | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| تعقب دلتا التكوين | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| جدولة وملاحظات التتبع التوجيهي | [ملاحظات الانتقال Nexus](./nexus-transition-notes) |

يُعلم هذا أن المصنوعات السابقة وصادرات التنبيهات/القياس عن بعد يجب أن تكون بالإضافة إلى سجل قرار الإدارة للوصول إلى B1 من الأشهر الثلاثة.