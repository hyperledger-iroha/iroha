---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-routed-trace-audit-2026q1
العنوان: تقرير تدقيق router-trace للربع الأول من عام 2026 (B1)
الوصف: نسخة تعويضات الليمتري الفصلية.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::ملاحظة المصدر الساقط
احترام هذه الصفحة `docs/source/nexus_routed_trace_audit_report_2026q1.md`. ابق النسختين متوافقتين حتى يصل إلى الترجمات المتبقية.
:::

# تقرير دقيق Routed-Trace للربع الأول من عام 2026 (B1)

عنصر خريطة الطريق **B1 - خط الأساس لعمليات تدقيق التتبع والقياس عن بعد** يتطلب تفاصيل فصلية خطط التوجيه في Nexus. يوثق هذا التقرير نافذة تدقيق الربع الأول من عام 2026 (يناير-مارس) ليتمكن مجلس ال ولا يعتمد على تعديل التليمتري قبل تدريبات الربع الثاني.

## النطاق والجدول الزمني

| معرف التتبع | نافذة (التوقيت العالمي المنسق) | الهدف |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | التحقق من مخططات قبول المسار، والقيل والقال الطوابير، وتعبئة التنبيهات قبل تفعيل المسار المتعدد. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | التحقق من إعادة تشغيل OTLP، وتكافؤ diff bot، وWestiaعاب تليمتري SDK قبل المراحل AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | تاكيد فروقات `iroha_config` معتمد من الـ تور والاستعداد للرجوع قبل قطع RC1. |

كلمات دليلية:

##الدروسية

1. **جمع التليمتري.** اصدرت كل الكون المهيكل `nexus.audit.outcome` والمقاييس المصاحبة (`nexus_audit_outcome_total*`). قام المساعد `scripts/telemetry/check_nexus_audit_outcome.py` بمتابعة سجل JSON والتحقق من حالة الحدث وارشفة الحمولة تحت `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **الإشعار من التنبيهات.** ضمنت `dashboards/alerts/nexus_audit_rules.yml` واداة الاختبار الخاصة بها البقاء عتبات الضوضاء التنبيه وقوالب البلوتوث ملتقطة. لسبب CI الملف `dashboards/alerts/tests/nexus_audit_rules.test.yml` عند كل تغيير؛ وتم اختبارها بالكامل خلال كل نافذة.
3. **التقاط لوحات الدخول.** قام بتشغيلون بتصدير لوحات router-trace من `dashboards/grafana/soranet_sn16_handshake.json` (صحة المصافحة) ولوحات نظرة عامة على الليمتري لربط صحة الطوابير بنتائج العزل.
4. **ملاحظات المراجعين.** أهداف سكرتارية الويب الاحرف الاولى للمراجعين والقرار ولا تذكر في [Nexus الانتقال ملاحظات](./nexus-transition-notes) وفي متتبع فروقات الاعدادات (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

##النتائج

| معرف التتبع | النتيجة | الدليل | مذكرة |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | تمرير | لقطات تنبيه fire/recover (رابط داخلي) + إعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; فروقات التايمر مسجلة في [Nexus مذكرات الانتقال](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | تبقى P95 لقبول الطوابير عند 612 مللي ثانية (الهدف <=750 مللي ثانية). لا يوجد متابعة مطلوبة. |
| `TRACE-TELEMETRY-BRIDGE` | تمرير | محفوظة محفوظة `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` مع إعادة تشغيل بصمة الإصبع OTLP مسجلة في `status.md`. | تطابقت الأملاح الخاصة بتنقيح SDK مع خط الصدأ الاساس؛ بلغ ابلغ فرق بوت عن صفر فروقات. |
| `TRACE-CONFIG-DELTA` | تمرير (التخفيف مغلق) | سجل تتبع الالتيمتري (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + ملف المانيفست TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + المانيفست حزمة الليمتري (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | اعاد تشغيل Q2 تجزئة ملف TLS فقط وعدم وجود متاخرين؛ سجل مانيفست الليمتري نطاقات الفتحات 912-936 وبذرة الحمل `NEXUS-REH-2026Q2`. |

انتجت جميع الـ آثار على الصحيحا واحدا `nexus.audit.outcome` ضمن نوافذها، بما في ذلك حواجز Alertmanager (`NexusAuditOutcomeFailure` الباقي اخضر جزئيا).

##المتابعات

- تم تحديث تثبيت بصمة التتبع الموجهة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; لم يتم الانتهاء من `NEXUS-421` في مذكرات النقل.
- بسبب خسارة في تشغيل اعادات OTLP الخام و diff الخاصة بـ Torii بالارشيف لتعزيز التكامل التكافؤ لمراجعات Android AND4/AND7.
- تاكيد ان بروفة `TRACE-MULTILANE-CANARY` القادمة إشراك مساعد الليمتري بنفسه حتى توقيع Q2 من سير العمل مؤهل.

## فهرس التحف

| الاصل | الموقع |
|-------|----------|
| المدرب التليمتري | `scripts/telemetry/check_nexus_audit_outcome.py` |
| قواعد وتنبيهات الاختبار | `dashboards/alerts/nexus_audit_rules.yml`، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال حمولة النتيجة | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| تتبع فروقات الاعدادات | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| جدول التتبع والملاحظات | [ملاحظات الانتقال Nexus](./nexus-transition-notes) |

يجب أن يبدأ هذا التقرير والقطع أولاه لبدء إطلاق التنبيهات/التليمتري بسجل يحكم الغلق B1 لهذا القطاع الذكي.