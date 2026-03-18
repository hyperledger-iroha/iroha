---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
título: تقرير تدقيق seguimiento enrutado للربع Q1 2026 (B1)
descripción: نسخة مطابقة لـ `docs/source/nexus_routed_trace_audit_report_2026q1.md` تغطي نتائج تدريبات التليمتري الفصلية.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota المصدر القانوني
Utilice el código `docs/source/nexus_routed_trace_audit_report_2026q1.md`. ابق النسختين متوافقتين حتى تصل الترجمات المتبقية.
:::

# تقرير تدقيق Routed-Trace للربع Q1 2026 (B1)

Haga clic en **B1 - Línea base de telemetría y auditorías de seguimiento de enrutamiento** Haga clic en el enlace de seguimiento de enrutamiento Nexus. يوثق هذا التقرير نافذة تدقيق Q1 2026 (يناير-مارس) لكي يستطيع مجلس الحوكمة اعتماد وضع التليمتري قبل تدريبات اطلاق Q2.

## النطاق والجدول الزمني

| ID de seguimiento | النافذة (UTC) | الهدف |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Hay muchos carriles, chismes y muchos carriles múltiples. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Utilice OTLP, diff bot y SDK para AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Utilice el controlador `iroha_config` para conectar el conector RC1. |

Haga clic en el enlace de seguimiento enrutado (`nexus.audit.outcome` + Prometheus) y Alertmanager محملة وتصدير الادلة الى `docs/examples/`.

## المنهجية1. **جمع التليمتري.** اصدرت كل العقد الحدث المهيكل `nexus.audit.outcome` andالمقاييس المصاحبة (`nexus_audit_outcome_total*`). Utilice el código `scripts/telemetry/check_nexus_audit_outcome.py` para crear archivos JSON y los archivos `docs/examples/nexus_audit_outcomes/`. [scripts/telemetría/check_nexus_audit_outcome.py:1]
2. **التحقق من التنبيهات.** ضمنت `dashboards/alerts/nexus_audit_rules.yml` واداة الاختبار الخاصة بها بقاء عتبات ضوضاء التنبيه وقوالب الحمولة متسقة. يشغل CI الملف `dashboards/alerts/tests/nexus_audit_rules.test.yml` عند كل تغيير؛ وتم اختبار القواعد نفسها يدويا خلال كل نافذة.
3. **التقاط لوحات المراقبة.** قام المشغلون بتصدير لوحات routed-trace de `dashboards/grafana/soranet_sn16_handshake.json` (صحة المصافحة) y نظرة عامة على التليمتري لربط صحة الطوابير بنتائج التدقيق.
4. **ملاحظات المراجعين.** سجلت سكرتارية الحوكمة الاحرف الاولى للمراجعين والقرار وتذاكر التخفيف في [Notas de transición Nexus](./nexus-transition-notes) وفي متتبع فروقات الاعدادات (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## النتائج| ID de seguimiento | النتيجة | الدليل | الملاحظات |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pase | لقطات تنبيه fuego/recuperación (رابط داخلي) + اعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; فروقات التليمتري مسجلة في [Notas de transición Nexus](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | La duración del P95 es de 612 ms (menos de 750 ms). لا يوجد متابعة مطلوبة. |
| `TRACE-TELEMETRY-BRIDGE` | Pase | Utilice el dispositivo `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` para reproducir OTLP en el dispositivo `status.md`. | تطابقت sales الخاصة بتنقيح SDK مع خط Rust الاساس؛ ابلغ diff bot عن صفر فروقات. |
| `TRACE-CONFIG-DELTA` | Pasa (mitigación cerrada) | سجل متتبع الحوكمة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifiesto ملف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifiesto حزمة التليمتري (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | اعاد تشغيل Q2 تجزئة ملف TLS المعتمد واكد عدم وجود متاخرين؛ سجل manifiesto التليمتري نطاق الفتحات 912-936 وبذرة الحمل `NEXUS-REH-2026Q2`. |

انتجت جميع الـ traces على الاقل حدثا واحدا `nexus.audit.outcome` ضمن نوافذها، بما يلبي حواجز Alertmanager (`NexusAuditOutcomeFailure` بقي اخضر طوال الربع).

## المتابعات

- تم تحديث ملحق de seguimiento enrutado ببصمة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; تم اغلاق التخفيف `NEXUS-421` في notas de transición.
- Configuración de la configuración OTLP y diferenciación de la configuración Torii para la configuración del sistema Aplicaciones Android AND4/AND7.
- تاكيد ان بروفة `TRACE-MULTILANE-CANARY` القادمة تعيد استخدام مساعد التليمتري نفسه حتى يستفيد توقيع Q2 من سير العمل المعتمد.

## artefactos فهرس| الاصل | الموقع |
|-------|----------|
| مدقق التليمتري | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Otros productos y servicios | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| resultado de مثال حمولة | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| متتبع فروقات الاعدادات | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| جدول seguimiento enrutado y والملاحظات | [Notas de transición Nexus](./nexus-transition-notes) |

يجب ارفاق هذا التقرير والقطع اعلاه وعمليات تصدير التنبيهات/التليمتري بسجل قرار الحوكمة لاغلاق B1 لهذا الربع.