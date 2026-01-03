<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ar
direction: rtl
source: docs/source/nexus_routed_trace_audit_report_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77d8021c6e09ba132ba080f183b532b35f8f6293a13497646566d13e932306
source_last_modified: "2025-11-22T12:03:01.494516+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_routed_trace_audit_report_2026q1.md -->

# تقرير تدقيق Routed-Trace للربع الاول 2026 (B1)

يتطلب بند خارطة الطريق **B1 — Routed-Trace Audits & Telemetry Baseline** مراجعة
ربع سنوية لبرنامج routed-trace في Nexus. يوثق هذا التقرير نافذة تدقيق الربع الاول
2026 (يناير-مارس) حتى يتمكن مجلس الحوكمة من اعتماد وضع التلِمتري قبل تدريبات
اطلاق الربع الثاني.

## النطاق والجدول الزمني

| Trace ID | النافذة (UTC) | الهدف |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | التحقق من مدرجات قبول lane، وgossip الطوابير، وتدفق التنبيهات قبل تفعيل multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | التحقق من OTLP replay، وتطابق diff bot، والتقاط تلِمتري SDK قبل محطات AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | تاكيد فروقات `iroha_config` المعتمدة من الحوكمة وجاهزية التراجع قبل قطع RC1. |

تم تنفيذ كل تدريب على طوبولوجيا شبيهة بالانتاج مع تفعيل ادوات routed-trace
(تلِمتري `nexus.audit.outcome` + عدادات Prometheus)، وتحميل قواعد Alertmanager،
وتصدير الادلة الى `docs/examples/`.

## المنهجية

1. **جمع التلِمتري.** اصدرت كل العقد الحدث المنظم `nexus.audit.outcome` والمقاييس
   المصاحبة (`nexus_audit_outcome_total*`). قام المساعد
   `scripts/telemetry/check_nexus_audit_outcome.py` بتتبع سجل JSON، والتحقق من حالة الحدث،
   وارشفة الحمولة تحت `docs/examples/nexus_audit_outcomes/`
   (`scripts/telemetry/check_nexus_audit_outcome.py:1`).
2. **تحقق التنبيهات.** ضمن `dashboards/alerts/nexus_audit_rules.yml` وحزمة الاختبارات الخاصة به
   ثبات عتبات الضوضاء وقوالب الحمولة. تقوم CI بتشغيل
   `dashboards/alerts/tests/nexus_audit_rules.test.yml` مع كل تغيير؛ وتمت مراجعة
   القواعد يدويا في كل نافذة.
3. **لقطات اللوحات.** قام المشغلون بتصدير لوحات routed-trace من
   `dashboards/grafana/soranet_sn16_handshake.json` (صحة handshake) ولوحات نظرة عامة على
   التلِمتري لربط صحة الطوابير بنتائج التدقيق.
4. **ملاحظات المراجعين.** سجلت سكرتارية الحوكمة احرف المراجعين والقرار واي تذاكر
   تخفيف في `docs/source/nexus_transition_notes.md` ومتتبع فروقات الاعداد
   (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## النتائج

| Trace ID | النتيجة | الادلة | الملاحظات |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | لقطات اطلاق/تعافي التنبيه (رابط داخلي) + replay لـ `dashboards/alerts/tests/soranet_lane_rules.test.yml`; فروقات التلِمتري مسجلة في `docs/source/nexus_transition_notes.md#quarterly-routed-trace-audit-schedule`. | بقي P95 لادخال الطابور عند 612 ms (الهدف <=750 ms). لا يوجد متابعة. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | حمولة مؤرشفة `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` مع hash لـ OTLP replay مسجل في `status.md`. | تطابقت املاح تنقيح SDK مع baseline Rust؛ وابلغ diff bot عن عدم وجود فروقات. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | مدخل متتبع الحوكمة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest ملف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest لحزمة التلِمتري (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | اعاد تشغيل Q2 تجزئة ملف TLS المعتمد واكد عدم وجود متاخرين؛ يسجل manifest التلِمتري نطاق slots 912–936 وبذرة workload `NEXUS-REH-2026Q2`. |

انتجت كل الـ traces حدثا واحدا على الاقل من `nexus.audit.outcome` داخل نوافذها،
مما يلبي guardrails في Alertmanager (بقي `NexusAuditOutcomeFailure` اخضر طوال الربع).

## المتابعات

- تم تحديث ملحق routed-trace بقيمة hash لـ TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`
  (انظر `nexus_transition_notes.md`); تم اغلاق `NEXUS-421`.
- الاستمرار في ارفاق OTLP replays الخام وقطع diff الخاصة بـ Torii في الارشيف لتعزيز
  ادلة التطابق لمراجعات AND4/AND7.
- تاكيد ان تدريبات `TRACE-MULTILANE-CANARY` القادمة تعيد استخدام نفس مساعد التلِمتري حتى
  يستفيد sign-off للربع الثاني من سير العمل المثبت.

## فهرس الادلة

| الاصل | الموقع |
|-------|----------|
| مدقق التلِمتري | `scripts/telemetry/check_nexus_audit_outcome.py` |
| قواعد واختبارات التنبيه | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| حمولة نتيجة نموذجية | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| متتبع فروقات الاعداد | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| جدول routed-trace والملاحظات | `docs/source/nexus_transition_notes.md` |

يجب ارفاق هذا التقرير والادلة اعلاه وصادرات التنبيه/التلِمتري بسجل قرار الحوكمة
لاغلاق B1 لهذا الربع.

</div>
