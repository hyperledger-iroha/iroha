---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محرك الصفقة
العنوان: التداول التجاري في SoraFS
Sidebar_label: تداول التداولات
الوصف: نظرة عامة على التداولات SF-8 وتكامل Torii وسطح التليميترية.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/deal_engine.md`. احرص على تفاصيل الموقع المتوافق مع ما دامت الوثائق القديمة.
:::

# التداول في SoraFS

تعرف على مسار خريطة الطريق SF-8 محرك التداول في SoraFS، موفراً
محاسبة حتمية لاتفاقات التخزين والاسترجاع بين
العملاء والمنظمون. تُوصَف التوافقات عبر حمولات Norito
المعترف بها في `crates/sorafs_manifest/src/deal.rs`، وتشمل شروط الصفقة،
نتيجة لذلك، تشير الاحتمالية إلى احتمالية الإصابة بالوفاة.

منشئ العامل المضمن في SoraFS (`sorafs_node::NodeHandle`) الآن
مثيلاً من `DealEngine` لكل عملية عقدة. يقوم المحرك بما يلي:

- يحقق من المعاملات ويسجلها باستخدام `DealTermsV1`؛
- يراكم رسومًا مقومة بـ XOR عند زيادة استخدام النسخ المتماثلة؛
- يقيّم نوافذ الآثار، التحليل الاحتمالي باستخدام عينات حتمية
  سجل على BLAKE3؛ و
- نتائج لقطات دفتر الأستاذ واتفاقية مناسبة للنشر الحوكمـي.تغطي المراجعة الموحدة للمصادقة، والنتيجة النهائية للثقافة، وتدفقات ليتمكن من الوصول إليها
المشغّلون من ممارسة واجهات API بثقة. تبعث التسويات الآن حمولات تور `DealSettlementV1`،
وتأرجو مصنعها نشر SF-12، كما تُصحح سلسلة OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`، `deal_expected_charge_nano`، `deal_client_debit_nano`،
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) من أجل اللوحات Torii
و SLOs. وأكملت الكمية المتبقية على slashing التي تبدأ منها المنتهى
وتنسيق دلالات الإلغاء مع البناء.

التغذيه التليمترية تستخدم أيضًا مجموعة المعايير `sorafs.node.micropayment_*`:
`micropayment_charge_nano`، `micropayment_credit_generated_nano`،
`micropayment_credit_applied_nano`، `micropayment_credit_carry_nano`،
`micropayment_outstanding_nano`, وتعديلات المهنة
(`micropayment_tickets_processed_total`، `micropayment_tickets_won_total`،
`micropayment_tickets_duplicate_total`). الإعلان عن تدفق اليانصيب
من المحتمل أن تتعرض للمشغّلين من ربط مكاسبك ونقلها ونقلها
نتائج جيدة.

## تكامل Torii

تعرض Torii نقاط نهاية كي المعدادون مخصص من أجل الاستخدام وتحريك
دورة حياة الصفقة بدون أسلاك مخصصة:- `POST /v2/sorafs/deal/usage` يقبل تليمترية `DealUsageReport` ويعيد
  نتائج المحاسبة حتمية (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` ينهى النافذة الحالية، ويبث
  `DealSettlementRecord` الناتج إلى الجانب `DealSettlementV1` مشفرة بـ base64
  وجاهزًا للنشر في DAG التورم.
- يغذي `/v2/events/sse` في Torii أرشيفات الآن `SorafsGatewayEvent::DealUsage`
  التي تلخص كل تبادل استخدام (العصر، ساعات GiB المثالية، تعدادات الربح،
  الحتمية)، وتسجيلات `SorafsGatewayEvent::DealSettlement`
  التي تحتوي على لقطة دفتر الأستاذ المعتمدة للتسويق مع Digest/الحجم/base64 من BLAKE3
  للقطعة الحوكمية على القرص، وتنبيهات `SorafsGatewayEvent::ProofHealth`
  بعد تجاوز اعتبات PDP/PoTR (المنظم، النافذة، حالة الإضراب/التباطؤ، كمية محدودة).
  يمكن للمستهلكين تصفية حسب المتحكم للتفاعل مع رسائل تذكيرية جديدة أو تسويقيات أو تنبيهات
  صحة البراهين دون الاقتراع.

تشارك نقطتي النهائية في إطار حصص SoraFS عبر النافذة
`torii.sorafs.quota.deal_telemetry` الجديدة، ما يسمح للمشغّلين بضبط معدل النشر
بدلا لكل نشر.