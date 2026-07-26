---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-routed-trace-audit-2026q1
العنوان: تقرير التدقيق الموجه للتتبع 2026، الربع الأول (B1)
الوصف: مرآة `docs/source/nexus_routed_trace_audit_report_2026q1.md`، تغطي نتائج التكرار الثلاثي للقياس عن بعد.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/nexus_routed_trace_audit_report_2026q1.md`. قم بمحاذاة نسختين حتى تصل الترجمات.
:::

# تقرير التدقيق الموجه للربع الأول من عام 2026 (B1)

عنصر خريطة الطريق **B1 - عمليات تدقيق التتبع الموجهة وخط الأساس للقياس عن بعد** يتطلب مراجعة ثلاثية لبرنامج التتبع التوجيهي Nexus. يوثق هذا التقرير نافذة التدقيق للربع الأول من عام 2026 (يناير-مارس) بحيث يمكن لمجلس الحوكمة التحقق من وضع القياس عن بعد قبل تكرارات الرمي في الربع الثاني.

## بورتيه وتقويم

| معرف التتبع | فينيتري (UTC) | موضوعي |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | تحقق من الرسوم البيانية للدخول إلى الممرات وأخبار الملفات وتدفق التنبيهات قبل التنشيط متعدد المسارات. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | التحقق من إعادة تشغيل OTLP، وتكافؤ الروبوتات المختلفة، وإدخال أدوات القياس عن بعد SDK قبل الجالونات AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | قم بتأكيد دلتا `iroha_config` التي توافق على الإدارة والإعداد للتراجع قبل قطع RC1. |

كل تكرار للدورة على طوبولوجيا إنتاجية متقدمة باستخدام جهاز التتبع النشط (جهاز القياس عن بعد `nexus.audit.outcome` + أجهزة الكمبيوتر Prometheus)، وقواعد إدارة التنبيهات والإرشادات المصدرة في `docs/examples/`.

## المنهجية

1. **تجميع أجهزة القياس عن بعد.** جميع الأيام الصادرة عن هيكل الحدث `nexus.audit.outcome` والمقاييس المرتبطة به (`nexus_audit_outcome_total*`). يتابع المساعد `scripts/telemetry/check_nexus_audit_outcome.py` سجل JSON ويصحح حالة الحدث ويحفظ الحمولة `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **التحقق من صحة التنبيهات.** `dashboards/alerts/nexus_audit_rules.yml` ويتم استخدام الاختبار للتأكد من أن نبضات الصوت ونماذج الحمولات تبقى متماسكة. CI تنفيذ `dashboards/alerts/tests/nexus_audit_rules.test.yml` تعديل Chaque؛ يتم تنظيم الميمات يدويًا من خلال كل نافذة.
3. **التقاط لوحات المعلومات.** يقوم المشغلون بتصدير لوحات التتبع الموجهة من `dashboards/grafana/soranet_sn16_handshake.json` (مصافحة سليمة) ولوحات معلومات القياس عن بعد العالمية لربط صحة الملفات مع نتائج التدقيق.
4. **ملاحظات المراجعين.** سر الإدارة يرسل الأحرف الأولى والقرار وبطاقات التخفيف في [Nexus مذكرات الانتقال](./nexus-transition-notes) وفي متتبع دلتا التكوين (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## الحواشي

| معرف التتبع | النتيجة | بريفز | ملاحظات |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | تمرير | يلتقط تنبيه إطلاق النار/الاسترداد (lien interne) + إعادة التشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`؛ diffs de telemetrie enregistres dans [Nexus ملاحظات الانتقال](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | يستغرق إدخال الملف P95 612 مللي ثانية (الكابل <=750 مللي ثانية). يتطلب الأمر ذلك. |
| `TRACE-TELEMETRY-BRIDGE` | تمرير | أرشيف الحمولة النافعة `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` بالإضافة إلى تجزئة إعادة التشغيل OTLP المسجلة في `status.md`. | تتوافق عناصر التنقيح SDK مع قاعدة Rust؛ لو فرق بوت إشارة دلتا صفر. |
| `TRACE-CONFIG-DELTA` | تمرير (التخفيف مغلق) | مدخل متتبع الإدارة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + ملف تعريف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + بيانات القياس عن بعد لحزمة البيانات (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | قم بإعادة تشغيل Q2 وتجزئة ملف تعريف TLS الذي وافق عليه وتأكد من عدم وجود أي تأخيرات؛ سجل بيان القياس عن بعد فتحة الفتحة 912-936 ومستوى عبء العمل `NEXUS-REH-2026Q2`. |

كل الآثار تنتج على الأقل حدثًا `nexus.audit.outcome` في النوافذ، مما يرضي حواجز الحماية Alertmanager (`NexusAuditOutcomeFailure` هو الباقي على مدار الأشهر الثلاثة).

##الصوفيين

- الملحق الذي تم توجيهه - تتبعه لمدة يوم مع تجزئة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`؛ تم إغلاق عملية التخفيف `NEXUS-421` في ملاحظات الانتقال.
- تابع الانضمام إلى عمليات إعادة تشغيل OTLP bruts وعناصر الاختلاف Torii في الأرشيف لتعزيز مستوى التكافؤ لإصدارات Android AND4/AND7.
- تأكد من أن التدريبات اللاحقة `TRACE-MULTILANE-CANARY` ستستخدم الميم المساعد للقياس عن بعد حتى يستفيد التحقق من صحة Q2 من صلاحية سير العمل.

## فهرس القطع الأثرية

| الأصول | التمركز |
|-------|----------|
| التحقق من القياس عن بعد | `scripts/telemetry/check_nexus_audit_outcome.py` |
| قواعد واختبارات التنبيه | `dashboards/alerts/nexus_audit_rules.yml`، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال على نتائج الحمولة | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| تعقب دلتا التكوين | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| التخطيط والملاحظات توجيه التتبع | [ملاحظات الانتقال Nexus](./nexus-transition-notes) |

يجب إرفاق هذه العلاقة والمصنوعات اليدوية وصادرات التنبيهات/القياس عن بعد في مجلة قرار الحوكمة لإغلاق الفصل الدراسي B1.