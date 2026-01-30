---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-settlement-faq
title: الأسئلة الشائعة للتسوية
description: إجابات موجهة للمشغلين تغطي توجيه التسوية وتحويل XOR والقياس عن بعد وأدلة التدقيق.
---

تعكس هذه الصفحة الأسئلة الشائعة الداخلية للتسوية (`docs/source/nexus_settlement_faq.md`) بحيث يستطيع قراء البوابة مراجعة نفس الإرشادات دون التنقيب في mono-repo. تشرح كيف يعالج Settlement Router المدفوعات، وما المقاييس التي يجب مراقبتها، وكيف ينبغي للـ SDK دمج حمولات Norito.

## أبرز النقاط

1. **تعيين lane** — يعلن كل dataspace عن `settlement_handle` (`xor_global` أو `xor_lane_weighted` أو `xor_hosted_custody` أو `xor_dual_fund`). راجع أحدث كتالوج lane تحت `docs/source/project_tracker/nexus_config_deltas/`.
2. **تحويل حتمي** — يحول الـ router جميع التسويات إلى XOR عبر مصادر السيولة المعتمدة من الحوكمة. تقوم lanes الخاصة بتمويل مخازن XOR مسبقا؛ ولا تطبق haircuts إلا عندما تنحرف المخازن خارج السياسة.
3. **القياس عن بعد** — راقب `nexus_settlement_latency_seconds` وعدادات التحويل ومقاييس haircut. توجد لوحات المتابعة في `dashboards/grafana/nexus_settlement.json` والتنبيهات في `dashboards/alerts/nexus_audit_rules.yml`.
4. **الأدلة** — أرشف الإعدادات وسجلات الـ router وتصديرات القياس عن بعد وتقارير المطابقة لأغراض التدقيق.
5. **مسؤوليات SDK** — يجب على كل SDK توفير أدوات مساعدة للتسوية ومعرفات lane ومشفري حمولات Norito للحفاظ على التكافؤ مع الـ router.

## أمثلة على التدفقات

| نوع lane | الأدلة المطلوبة | ماذا يثبت |
|-----------|--------------------|----------------|
| خاصة `xor_hosted_custody` | سجل الـ router + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | تؤكد أن مخازن CBDC تخصم XOR حتمي وأن haircuts تبقى ضمن السياسة. |
| عامة `xor_global` | سجل الـ router + مرجع DEX/TWAP + مقاييس زمن الاستجابة/التحويل | يثبت أن مسار السيولة المشترك سعّر التحويل وفق TWAP المنشور دون haircut. |
| هجينة `xor_dual_fund` | سجل الـ router يظهر تقسيم public مقابل shielded + عدادات القياس عن بعد | يثبت أن المزج بين shielded/public احترم نسب الحوكمة وسجل haircut المطبق على كل جزء. |

## هل تحتاج مزيدا من التفاصيل؟

- FAQ الكامل: `docs/source/nexus_settlement_faq.md`
- مواصفة Settlement router: `docs/source/settlement_router.md`
- دليل سياسات CBDC: `docs/source/cbdc_lane_playbook.md`
- دليل التشغيل: [عمليات Nexus](./nexus-operations)
