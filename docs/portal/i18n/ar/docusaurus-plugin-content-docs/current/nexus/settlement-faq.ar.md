---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/settlement-faq.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: الأسئلة الشائعة حول التسوية
العنوان: الأسئلة الشائعة للتسويق
الوصف: جذابة للمشغلين تغطي توجيهات التحويلات XOR والقياس عن بعد وأدلة التدقيق.
---

تهتم هذه الصفحة بالأسئلة الشائعة للتسويق (`docs/source/nexus_settlement_faq.md`) بحيث يمكن قراء البوابة مراجعة نفس الإرشادات دون التنقيب في mono-repo. شرح كيفية مراجعة Settlement Router، وما المعايير التي يجب مراقبتها، وكيف ينبغي للـ SDK دمج حمولات Norito.

## نقاط مميزة

1. **تعيين لين** — أعلن كل dataspace عن `settlement_handle` (`xor_global` أو `xor_lane_weighted` أو `xor_hosted_custody` أو `xor_dual_fund`). إعادة تحديث كتالوج حارة تحت `docs/source/project_tracker/nexus_config_deltas/`.
2. **تحويل حتمي** — تحويل الـ راوتر جميع التسويقيات إلى XOR عبر مصادر متكاملة من الـ التوجيه. تقوم الممرات الخاصة بتمويل مخازن XOR مسبقًا؛ ولا تطبق قصات الشعر إلا عندما تحرف المخازن خارج السياسة.
3. **القياس عن بعد** — راقب `nexus_settlement_latency_seconds` وعدادات التحويل ومقاييس الشعر. توجد لوحات للأعلى في `dashboards/grafana/nexus_settlement.json` والتنبيهات في `dashboards/alerts/nexus_audit_rules.yml`.
4. **الأدلة** — أرشف الإعدادات تثبت الـ جهاز التوجيه وتصدير القياسات بعد وتقارير المطابقة لأغراض التحديد.
5. **مسؤوليات SDK** — يجب على كل SDK توفير أدوات مساعدة للتسويق ومعرفات حارة وشفرة حمولات Norito على الـ تكافؤ مع الـ router.

##مثال على العاصمة

| نوع الممر | الأدلة المطلوبة | ماذا يثبت |
|-----------|--------------------|----------------|
| خاصة `xor_hosted_custody` | سجل الـ router + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | ويؤكد أن مخازن CBDC تخصم XOR حتمي وأن قصات الشعر الحالية ضمن السياسة. |
| عامة `xor_global` | سجل الـ router + مرجع DEX/TWAP + مقاييس زمن التعرض/التحويل | يستمر أن مسار اتجاه مشرق سعّر يتغير حسب TWAP التالي دون قصّة الشعر. |
| هجينة `xor_dual_fund` | سجل الـ راوتر يوضح الرسم العام مقابل المحمي + عدادات القياس عن بعد | تثبت أن العلاقة بين الأشخاص المحميين/الجمهور تحترم بشكل نسبي وتلاحظ قصة الشعر المطبق على كل جزء. |

## هل تحتاج لقراءة من التفاصيل؟

- الأسئلة الشائعة الكاملة: `docs/source/nexus_settlement_faq.md`
- موصفة موجه التسوية : `docs/source/settlement_router.md`
- دليل سياسات CBDC: `docs/source/cbdc_lane_playbook.md`
- دليل التشغيل: [عمليات Nexus](./nexus-operations)