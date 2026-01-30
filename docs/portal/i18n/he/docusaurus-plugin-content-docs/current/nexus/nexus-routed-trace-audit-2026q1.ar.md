---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-routed-trace-audit-2026q1
title: تقرير تدقيق routed-trace للربع Q1 2026 (B1)
description: نسخة مطابقة لـ `docs/source/nexus_routed_trace_audit_report_2026q1.md` تغطي نتائج تدريبات التليمتري الفصلية.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note المصدر القانوني
تعكس هذه الصفحة `docs/source/nexus_routed_trace_audit_report_2026q1.md`. ابق النسختين متوافقتين حتى تصل الترجمات المتبقية.
:::

# تقرير تدقيق Routed-Trace للربع Q1 2026 (B1)

عنصر خارطة الطريق **B1 - Routed-Trace Audits & Telemetry Baseline** يتطلب مراجعة فصلية لبرنامج routed-trace في Nexus. يوثق هذا التقرير نافذة تدقيق Q1 2026 (يناير-مارس) لكي يستطيع مجلس الحوكمة اعتماد وضع التليمتري قبل تدريبات اطلاق Q2.

## النطاق والجدول الزمني

| Trace ID | النافذة (UTC) | الهدف |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | التحقق من مخططات قبول lane، وgossip الطوابير، وتدفق التنبيهات قبل تفعيل multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | التحقق من اعادة تشغيل OTLP، وتكافؤ diff bot، واستيعاب تليمتري SDK قبل مراحل AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | تاكيد فروقات `iroha_config` المعتمدة من الحوكمة والاستعداد للرجوع قبل قطع RC1. |

جرت كل بروفة على بنية شبيهة بالانتاج مع تفعيل ادوات routed-trace (تليمتري `nexus.audit.outcome` + عدادات Prometheus) وقواعد Alertmanager محملة وتصدير الادلة الى `docs/examples/`.

## المنهجية

1. **جمع التليمتري.** اصدرت كل العقد الحدث المهيكل `nexus.audit.outcome` والمقاييس المصاحبة (`nexus_audit_outcome_total*`). قام المساعد `scripts/telemetry/check_nexus_audit_outcome.py` بمتابعة سجل JSON والتحقق من حالة الحدث وارشفة الحمولة تحت `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **التحقق من التنبيهات.** ضمنت `dashboards/alerts/nexus_audit_rules.yml` واداة الاختبار الخاصة بها بقاء عتبات ضوضاء التنبيه وقوالب الحمولة متسقة. يشغل CI الملف `dashboards/alerts/tests/nexus_audit_rules.test.yml` عند كل تغيير؛ وتم اختبار القواعد نفسها يدويا خلال كل نافذة.
3. **التقاط لوحات المراقبة.** قام المشغلون بتصدير لوحات routed-trace من `dashboards/grafana/soranet_sn16_handshake.json` (صحة المصافحة) ولوحات نظرة عامة على التليمتري لربط صحة الطوابير بنتائج التدقيق.
4. **ملاحظات المراجعين.** سجلت سكرتارية الحوكمة الاحرف الاولى للمراجعين والقرار وتذاكر التخفيف في [Nexus transition notes](./nexus-transition-notes) وفي متتبع فروقات الاعدادات (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## النتائج

| Trace ID | النتيجة | الدليل | الملاحظات |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | لقطات تنبيه fire/recover (رابط داخلي) + اعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; فروقات التليمتري مسجلة في [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | بقي P95 لقبول الطوابير عند 612 ms (الهدف <=750 ms). لا يوجد متابعة مطلوبة. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | حمولة محفوظة `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` مع بصمة replay OTLP مسجلة في `status.md`. | تطابقت salts الخاصة بتنقيح SDK مع خط Rust الاساس؛ ابلغ diff bot عن صفر فروقات. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | سجل متتبع الحوكمة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest ملف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest حزمة التليمتري (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | اعاد تشغيل Q2 تجزئة ملف TLS المعتمد واكد عدم وجود متاخرين؛ سجل manifest التليمتري نطاق الفتحات 912-936 وبذرة الحمل `NEXUS-REH-2026Q2`. |

انتجت جميع الـ traces على الاقل حدثا واحدا `nexus.audit.outcome` ضمن نوافذها، بما يلبي حواجز Alertmanager (`NexusAuditOutcomeFailure` بقي اخضر طوال الربع).

## المتابعات

- تم تحديث ملحق routed-trace ببصمة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; تم اغلاق التخفيف `NEXUS-421` في transition notes.
- الاستمرار في ارفاق اعادات تشغيل OTLP الخام وقطع diff الخاصة بـ Torii بالارشيف لتعزيز ادلة التكافؤ لمراجعات Android AND4/AND7.
- تاكيد ان بروفة `TRACE-MULTILANE-CANARY` القادمة تعيد استخدام مساعد التليمتري نفسه حتى يستفيد توقيع Q2 من سير العمل المعتمد.

## فهرس artefacts

| الاصل | الموقع |
|-------|----------|
| مدقق التليمتري | `scripts/telemetry/check_nexus_audit_outcome.py` |
| قواعد وتنبيهات الاختبار | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال حمولة outcome | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| متتبع فروقات الاعدادات | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| جدول routed-trace والملاحظات | [Nexus transition notes](./nexus-transition-notes) |

يجب ارفاق هذا التقرير والقطع اعلاه وعمليات تصدير التنبيهات/التليمتري بسجل قرار الحوكمة لاغلاق B1 لهذا الربع.
