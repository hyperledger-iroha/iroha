---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/settlement-faq.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: الأسئلة الشائعة حول التسوية
العنوان: تسوية الأسئلة الشائعة
الوصف: الردود على المشغلين تغطي تسوية التوجيه والتحويل XOR والقياس عن بعد وإجراءات التدقيق.
---

تعرض هذه الصفحة الأسئلة الشائعة حول التسوية الداخلية (`docs/source/nexus_settlement_faq.md`) حتى يتمكن قراء الباب من مراجعة المؤشرات المضحكة بدون خطأ في المستودع الأحادي. يشرح هذا التعليق أن Settlement Router يخصص المدفوعات، وتلك المقاييس التي تراقبها، ويعلق على SDK التي تعمل على دمج الحمولات Norito.

## النقاط

1. ** Mappage des Lanes ** - تعلن كل مساحة بيانات عن un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). راجع كتالوج الممرات الأحدث في `docs/source/project_tracker/nexus_config_deltas/`.
2. **تحديد التحويل** - يقوم جهاز التوجيه بتحويل جميع التسويات إلى XOR عبر مصادر السيولة المعتمدة من قبل الإدارة. الممرات الخاصة تمول المخازن المؤقتة XOR؛ لا تنطبق قصات الشعر عندما تكون المخازن المؤقتة مشتقة من السياسة.
3. **القياس عن بعد** - يراقب `nexus_settlement_latency_seconds` وعدادات التحويل ومقاييس قص الشعر. تظهر لوحات المعلومات في `dashboards/grafana/nexus_settlement.json` والتنبيهات في `dashboards/alerts/nexus_audit_rules.yml`.
4. **المعاينة** - أرشفة التكوينات وسجلات جهاز التوجيه وتصدير القياسات عن بعد وتقارير التسوية لعمليات التدقيق.
5. **مسؤوليات SDK** - كل SDK تكشف عن مساعدي التسوية ومعرفات الممرات ومشفرات الحمولات Norito لمحاذاة جهاز التوجيه.

## تدفق المثال

| نوع الخط | Preuves جامع | Ce que cela prove |
|-----------|--------------------|----------------|
| بريفي `xor_hosted_custody` | سجل دو راوتر + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | تخصم المخازن المؤقتة للعملات الرقمية للبنك المركزي (CBDC) من XOR المحدد وتظل قصات الشعر في السياسة. |
| العامة `xor_global` | سجل جهاز التوجيه + مرجع DEX/TWAP + مقاييس زمن الاستجابة/التحويل | تقوم آلية تقاسم السيولة بإصلاح سعر التحويل على TWAP المنشور بدون أي قصة شعر. |
| هايبرد `xor_dual_fund` | قم بتسجيل الدخول إلى جهاز التوجيه لإعادة التقسيم العام مقابل المحمي + أجهزة قياس القياس عن بعد | المزيج محمي / عام يحترم نسب الحكم ويسجل قصة الشعر المطبقة على كل جانب. |

## هل تحتاج إلى المزيد من التفاصيل؟

- الأسئلة الشائعة كاملة: `docs/source/nexus_settlement_faq.md`
- مواصفات جهاز توجيه التسوية: `docs/source/settlement_router.md`
- قواعد اللعبة السياسية CBDC: `docs/source/cbdc_lane_playbook.md`
- عمليات دفتر التشغيل: [العمليات Nexus](./nexus-operations)