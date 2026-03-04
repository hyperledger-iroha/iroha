---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-routed-trace-audit-2026q1
titre : تقرير تدقيق routed-trace للربع Q1 2026 (B1)
description: نسخة مطابقة لـ `docs/source/nexus_routed_trace_audit_report_2026q1.md` تغطي نتائج تدريبات التليمتري الفصلية.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note المصدر القانوني
Il s'agit de la référence `docs/source/nexus_routed_trace_audit_report_2026q1.md`. ابق النسختين متوافقتين حتى تصل الترجمات المتبقية.
:::

# تقرير تدقيق Routed-Trace للربع Q1 2026 (B1)

**B1 - Routed-Trace Audits & Telemetry Baseline** est disponible pour routed-trace dans Nexus. يوثق هذا التقرير نافذة تدقيق Q1 2026 (يناير-مارس) لكي يستطيع مجلس الحوكمة اعتماد وضع التليمتري قبل تدريبات اطلاق Q2.

## النطاق والجدول الزمني

| Identifiant de trace | النافذة (UTC) | الهدف |
|--------------|--------------|---------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Il s'agit d'une voie à voies multiples et de potins sur plusieurs voies. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Il s'agit d'un logiciel OTLP et d'un bot diff, ainsi que d'un SDK pour AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Utilisez le câble `iroha_config` pour le RC1. |

Vous pouvez utiliser la fonction routed-trace (système `nexus.audit.outcome` + système Prometheus) Le gestionnaire d'alertes est compatible avec `docs/examples/`.

## المنهجية1. **جمع التليمتري.** اصدرت كل العقد الحدث المهيكل `nexus.audit.outcome` والمقاييس المصاحبة (`nexus_audit_outcome_total*`). Le code `scripts/telemetry/check_nexus_audit_outcome.py` est compatible avec JSON et le code `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **التحقق من التنبيهات.** ضمنت `dashboards/alerts/nexus_audit_rules.yml` واداة الاختبار الخاصة بها بقاء عتبات ضوضاء التنبيه وقوالب الحمولة متسقة. يشغل CI الملف `dashboards/alerts/tests/nexus_audit_rules.test.yml` عند كل تغيير؛ وتم اختبار القواعد نفسها يدويا خلال كل نافذة.
3. **التقاط لوحات المراقبة.** قام المشغلون byتصدير لوحات routed-trace by `dashboards/grafana/soranet_sn16_handshake.json` (صحة المصافحة) et لوحات نظرة عامة على التليمتري لربط صحة الطوابير بنتائج التدقيق.
4. **ملاحظات المراجعين.** سجلت سكرتارية الحوكمة الاحرف الاولى للمراجعين والقرار وتذاكر التخفيف في [Notes de transition Nexus](./nexus-transition-notes) et les notes de transition (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## النتائج| Identifiant de trace | النتيجة | الدليل | الملاحظات |
|----------|---------|--------------|-------|
| `TRACE-LANE-ROUTING` | Passer | لقطات تنبيه fire/recover (رابط داخلي) + اعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; فروقات التليمتري مسجلة في [Notes de transition Nexus](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | Pour P95, la durée de vie est de 612 ms (longueur <= 750 ms). لا يوجد متابعة مطلوبة. |
| `TRACE-TELEMETRY-BRIDGE` | Passer | Utilisez `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` pour replay OTLP avec `status.md`. | Ajouter des sels à un SDK pour Rust ابلغ diff bot عن صفر فروقات. |
| `TRACE-CONFIG-DELTA` | Pass (atténuation fermée) | سجل متتبع الحوكمة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifeste ملف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifeste حزمة التليمتري (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | اعاد تشغيل Q2 تجزئة ملف TLS المعتمد واكد عدم وجود متاخرين؛ Veuillez contacter le manifeste au 912-936 et au numéro `NEXUS-REH-2026Q2`. |

Les traces sont également associées à `nexus.audit.outcome` et à Alertmanager (`NexusAuditOutcomeFailure` pour اخضر طوال الربع).

## المتابعات

- Vous utilisez routed-trace via TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ; J'utilise `NEXUS-421` pour les notes de transition.
- La fonction OTLP est également compatible avec la fonction diff Torii pour la fonction de configuration. Pour Android AND4/AND7.
- تاكيد ان بروفة `TRACE-MULTILANE-CANARY` القادمة تعيد استخدام مساعد التليمتري نفسه حتى يستفيد توقيع Q2 من سير العمل المعتمد.

## فهرس artefacts| الاصل | الموقع |
|-------|--------------|
| مدقق التليمتري | `scripts/telemetry/check_nexus_audit_outcome.py` |
| قواعد وتنبيهات الاختبار | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال حمولة résultat | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| متتبع فروقات الاعدادات | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Comment routed-trace | [Notes de transition Nexus](./nexus-transition-notes) |

يجب ارفاق هذا التقرير والقطع اعلاه وعمليات تصدير التنبيهات/التليمتري بسجل قرار الحوكمة لاغلاق B1 لهذا الربع.