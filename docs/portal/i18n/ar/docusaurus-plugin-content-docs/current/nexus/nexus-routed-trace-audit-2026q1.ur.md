---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: nexus-routed-trace-audit-2026q1
العنوان: تقرير التتبع الموجه للربع الأول من عام 2026 (B1)
الوصف: `docs/source/nexus_routed_trace_audit_report_2026q1.md` هو هذا، وهو عبارة عن سجل دراسي يرسل نتائج متضمنة في كرتا.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::ملاحظة کینونیکل مأخذ
هذه هي الصفحة `docs/source/nexus_routed_trace_audit_report_2026q1.md`. باقى التراجم هذا لا يمكن قوله.
:::

# تقرير التتبع الموجه للربع الأول من عام 2026 (B1)

دليل البيانات **B1 - خط الأساس لعمليات تدقيق التتبع الموجه والقياس عن بعد** Nexus برنامج التتبع الموجه الذي هو مطلوب بشدة. هذا تقرير الربع الأول من عام 2026 (شهر مارس) الذي نشر تقريرًا عن تقرير مجلس إدارة لانسر للربع الثاني من العام والذي تم نشره في أحدث تقاريره.

## دائرى و ٹائم عبر الإنترنت

| معرف التتبع | ونڈو (UTC) | قصدي |
|----------|-------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | تنشيط متعدد الممرات الرسوم البيانية للدخول إلى الممرات، وقائمة الانتظار القيل والقال وتدفق التنبيه. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | تعمل معالم AND4/AND7 على إعادة تشغيل OTLP، وتكافؤ الروبوتات المختلفة، واستيعاب القياس عن بعد لـ SDK. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | تم قطع RC1 إلى دلتا `iroha_config` المعتمدة من قبل الإدارة واستعداد التراجع. |

يتم إرسال طوبولوجيا شبيهة بالإنتاج إلى أدوات التتبع الموجهة (`nexus.audit.outcome` القياس عن بعد + عدادات Prometheus)، وقواعد مدير التنبيهات والأدلة `docs/examples/` في اكسبورت ہوا۔

## طريق العمل

1. **نقرة نقرية.** لا يتم تنظيم كل الأصوات `nexus.audit.outcome` ومقاييس ذات صلة (`nexus_audit_outcome_total*`). helper `scripts/telemetry/check_nexus_audit_outcome.py` هو ذيل سجل JSON، أيون ويلايت، والحمولة النافعة التي `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **الرتيل ويليشن.** `dashboards/alerts/nexus_audit_rules.yml` وهو عبارة عن أداة اختبار واحدة وهي عبارة عن عتبات ضوضاء التنبيه ونموذج الحمولة الصافية. تم استبدال CI بـ `dashboards/alerts/tests/nexus_audit_rules.test.yml` تشيلاتا؛ هذه هي رولز التي يتم تسليط الضوء عليها أيضًا.
3. **جميع أجهزة الكمبيوتر المحمولة.** لا تحتوي السجلات على `dashboards/grafana/soranet_sn16_handshake.json` (صحة المصافحة) وهي عبارة عن لوحات تتبع موجهة ولوحات معلومات عامة عن القياس عن بعد في الرياضة، مما يؤدي إلى صحة قائمة الانتظار ونتائج التدقيق التي ترتبط بكل شيء.
4. **ملاحظات ريو.** لا تحتوي سجلات الويب على الأحرف الأولى للمراجع، والأصل، وتذاكر التخفيف مثل [ملاحظات الانتقال Nexus](./nexus-transition-notes) ومتعقب دلتا التكوين (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

##نتائج

| معرف التتبع | نتیجہ | ثبوت | أخبار |
|----------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | تمرير | تنبيه إطلاق/استرداد اسکرین شاتس (اندرونی لنک) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` إعادة التشغيل؛ فروق القياس عن بعد [Nexus ملاحظات الانتقال](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ریکارڈ۔ | دخول قائمة الانتظار P95 612 مللي ثانية پر رہا (ہدف <=750 مللي ثانية)۔ لا داعي للقلق. |
| `TRACE-TELEMETRY-BRIDGE` | تمرير | حمولة النتائج المؤرشفة `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` وتجزئة إعادة تشغيل OTLP `status.md`. | أملاح تنقيح SDK خط الأساس الصدأ سے تطابق تھے؛ بوت الفرق ليس له دلتا صفرية. |
| `TRACE-CONFIG-DELTA` | تمرير (التخفيف مغلق) | إدخال تعقب الحوكمة (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + بيان ملف تعريف TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + بيان حزمة القياس عن بعد (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | إعادة التشغيل في الربع الثاني لم يتم تشغيل تجزئة ملف تعريف TLS وعدم تتبع أي عناصر متطرفة؛ بيان القياس عن بعد لا يحتوي على فتحات 912-936 ودرج عبء العمل `NEXUS-REH-2026Q2`. |

لم يتم العثور على كل الآثار حتى الآن من خلال `nexus.audit.outcome` من خلال حواجز حماية Alertmanager (`NexusAuditOutcomeFailure` من خلال كوارتر مايجرين) رہا)۔

## المتابعات

- ملحق التتبع الموجه لتجزئة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` الذي تم إجراؤه؛ التخفيف `NEXUS-421` تلاحظ الانتقال
- مراجعات Android AND4/AND7 توفر أدلة تكافؤ متعددة المخاطر إعادة تشغيل OTLP الخام وTorii القطع الأثرية المختلفة التي تم إصدارها من قبل منسلك.
- يعد التحقق من بروفات `TRACE-MULTILANE-CANARY` ومساعد القياس عن بعد بمثابة استخدام متكرر لسير العمل الذي تم التحقق من صحة تسجيل الخروج منه في الربع الثاني.

## قطعة أثرية انڈیکس

| الأصول | مقام |
|-------|----------|
| مدقق القياس عن بعد | `scripts/telemetry/check_nexus_audit_outcome.py` |
| قواعد واختبارات التنبيه | `dashboards/alerts/nexus_audit_rules.yml`، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| حمولة نتيجة العينة | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| تكوين تعقب دلتا | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| جدول التتبع الموجه والملاحظات | [ملاحظات الانتقال Nexus](./nexus-transition-notes) |

تم إصدار هذا التقرير، وهو أول تقرير عن المصنوعات اليدوية وصادرات التنبيه/القياس عن بعد لسجل قرارات الحوكمة الذي تم من خلاله إنشاء ما يسمى بـ B1 Bend.