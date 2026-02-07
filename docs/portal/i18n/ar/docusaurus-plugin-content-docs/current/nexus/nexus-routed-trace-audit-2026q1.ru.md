---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-routed-trace-audit-2026q1
العنوان: إنهاء التدقيق التتبعي للربع الأول من عام 2026 (B1)
الوصف: ZERCALO `docs/source/nexus_routed_trace_audit_report_2026q1.md`, معزز من أجهزة قياس المسافة المتكررة.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Канонический источник
هذا الجزء يعرض `docs/source/nexus_routed_trace_audit_report_2026q1.md`. قم بالنسخ المتزامن دون الحاجة إلى نقل عالي الجودة.
:::

# أكمل مراجعة التتبع التوجيهي للربع الأول من عام 2026 (B1)

نقطة خريطة الطريق **B1 - خط الأساس لعمليات تدقيق التتبع والقياس عن بعد** تحتاج إلى برنامج التتبع التوجيهي Nexus. تم الانتهاء من ذلك بعد التدقيق في الربع الأول من عام 2026 (يناير-مارس)، والذي يمكن من خلاله تحقيق هدف التحكم في القياس عن بعد قبل التكرار، نطلق Q2.

## الموقع والجدول الزمني

| معرف التتبع | أوكنو (التوقيت العالمي المنسق) | Цель |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | التحقق من تسجيل الدخول في المسار، ومراقبة القيل والقال وتنبيهات الطريق قبل تضمين متعدد المسارات. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | التحقق من صحة إعادة تشغيل OTLP، وتكافؤ الروبوتات المختلفة، واستخدام أدوات قياس الاتصال عن بعد SDK إلى AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | التحقق من دلتا الإدارة المتوافقة مع `iroha_config` والرجوع إلى الحالة السابقة قبل سلسلة RC1. |

تم إنشاء العديد من التكرارات لطوبولوجيات أفضل باستخدام أدوات التتبع الموجهة (قياس البعد `nexus.audit.outcome` +) (Prometheus)، يتم تمكين Alertmanager وتصدير التصدير إلى `docs/examples/`.

## المنهجية

1. **قياس المسافة.** يتم استخدام جميع الأجهزة الهيكلية `nexus.audit.outcome` ومقاييس الطاقة الخاصة بها (`nexus_audit_outcome_total*`). يقوم المساعد `scripts/telemetry/check_nexus_audit_outcome.py` بمراقبة سجل JSON وحالة التحقق من صحة الاشتراك والحمولة المؤرشفة في `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **التحقق من التنبيهات.** `dashboards/alerts/nexus_audit_rules.yml` وهذا هو تسخير الاختبار الذي تم ضمان استقراره من خلال الحمولة الصافية والصلبة. أغلق CI `dashboards/alerts/tests/nexus_audit_rules.test.yml` عند التوسيع; إنهم مناسبون للتقدم في كل مرة.
3. **لوحة الاتصال.** لوحات تصدير المشغلين التي يتم توجيهها من `dashboards/grafana/soranet_sn16_handshake.json` (مصافحة الاتصال) وبطاقات المراقبة أجهزة القياس عن بعد لتسليم المعلومات إلى نتائج التدقيق.
4. **المراجعة.** قام سكرتير الإدارة بكتابة المراسلات والقرارات والتذاكر الأولية في انتقال [Nexus ملاحظات](./nexus-transition-notes) وأداة تكوين جهاز التوجيه (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## النتائج

| معرف التتبع | إيتوغ | التصريح | مساعدة |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | تمرير | لقطات الشاشة لتنبيهات إطلاق/استرداد (الإعدادات الداخلية) + إعادة التشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; Елеметраийные дификсированы в [Nexus ملاحظات الانتقال](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 праиема оченеди остался 612 مللي ثانية (цель <= 750 مللي ثانية). لا داعي للقلق. |
| `TRACE-TELEMETRY-BRIDGE` | تمرير | الحمولة الأرشيفية `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` بالإضافة إلى تجزئة إعادة تشغيل OTLP، المسجلة في `status.md`. | أملاح التنقيح SDK تكتمل مع خط الأساس الصدأ؛ diff bot сообщил ноль дельт. |
| `TRACE-CONFIG-DELTA` | تمرير (التخفيف مغلق) | انقر فوق Governance-Treкера (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + بيان ملف تعريف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + بيان حزمة القياس عن بعد (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | أعد تشغيل Q2 لملف تعريف TLS المعتمد وتم التحقق من النتيجة؛ بيان القياس عن بعد يصلح الفتحات 912-936 وبذور عبء العمل `NEXUS-REH-2026Q2`. |

تم تحديد جميع الآثار التي تم اختيارها من قبل شخص واحد `nexus.audit.outcome` في العمق، وهو ما يجعل حواجز الحماية المبهجة Alertmanager (`NexusAuditOutcomeFailure` خالية من المخاطر) весь кваrtал).

## المتابعات

- إضافة تتبع التوجيه إلى TLS من خلال `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; التخفيف `NEXUS-421` مغلق في ملاحظات النقل.
- مواصلة عرض عمليات إعادة التشغيل الخاصة بـ OTLP والعناصر الاصطناعية Torii diff في الأرشيف لتتمكن من تسهيل الإحالة المالية محقق Android AND4/AND7.
- تأكد من أن التكرارات السابقة `TRACE-MULTILANE-CANARY` تستخدم بمثابة مساعد القياس عن بعد لتشغيل تسجيل الخروج في Q2 سير العمل المثبت.

## فهرس المصنوعات اليدوية

| الأصول | الموقع |
|-------|----------|
| المدقق عن بعد | `scripts/telemetry/check_nexus_audit_outcome.py` |
| التنبيهات الصحيحة والاختبارات | `dashboards/alerts/nexus_audit_rules.yml`، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال على الحمولة النافعة | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| تريكر تكوين التكوين | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| الرسومات والبيانات تتبع التوجيه | [ملاحظات الانتقال Nexus](./nexus-transition-notes) |

هذا يعني أن جميع العناصر وتنبيهات الصادرات/أجهزة القياس عن بعد هي تطبيقات لإدارة اليومية لتأمين B1 غرفة.