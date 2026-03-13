---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: محرك الصفقة
العنوان: محرك الاتفاقات SoraFS
Sidebar_label: محرك الاتفاقات
الوصف: مجموعة محرك الاتفاق SF-8 وتكامل Torii وأسطح القياس عن بعد.
---

:::ملاحظة المصدر الكنسي
:::

# محرك الاتفاقات SoraFS

يقدم مسار خريطة الطريق SF-8 المحرك الاتفاقي SoraFS، المورد
موازنة محددة لصفقات التخزين والاسترداد بين
العملاء والموردين. يتم توضيح الاتفاقات عبر الحمولات Norito
محدد في `crates/sorafs_manifest/src/deal.rs`، يشمل شروط الاتفاقية،
تأمين السندات، والمدفوعات الصغيرة المحتملة، والتسجيلات التنظيمية.

العامل SoraFS يغلق (`sorafs_node::NodeHandle`) على الفور
`DealEngine` لكل عملية معالجة. المحرك :

- التحقق من الاتفاقات وتسجيلها عبر `DealTermsV1`؛
- تراكم الرسوم المستحقة على XOR عندما يكون استخدام النسخ متناسبًا؛
- تقييم نوافذ الدفع الصغير المحتملة من خلال عملية تحديد التخفيض
  قاعدة على BLAKE3 ; وآخرون
- إنتاج لقطات من دفتر الأستاذ وحمولات التنظيم المتكيفة مع النشر
  دي الحكم.تشمل الاختبارات الوحدوية التحقق من الصحة واختيار الدفعات الصغيرة وتدفق الأموال
التنظيم الذي يسمح للمشغلين بممارسة واجهات برمجة التطبيقات بثقة. القواعد
يحدث خلل في حمولات الإدارة `DealSettlementV1`، فهو متكامل بشكل مباشر
في خط أنابيب النشر SF-12، ونواصل اليوم سلسلة OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`، `deal_expected_charge_nano`، `deal_client_debit_nano`،
`deal_outstanding_nano`، `deal_bond_slash_nano`، `deal_publish_total`) للوحات المعلومات Torii وغيرها
تطبيق SLO. تتيح الأعمال التالية أتمتة عملية القطع على قدم المساواة
المدققون وتنسيق دلالات الإلغاء مع سياسة الحكم.

استخدام الطاقة عن بعد بالإضافة إلى مجموعة المقاييس `sorafs.node.micropayment_*` :
`micropayment_charge_nano`، `micropayment_credit_generated_nano`،
`micropayment_credit_applied_nano`، `micropayment_credit_carry_nano`،
`micropayment_outstanding_nano`، وحاسبو التذاكر
(`micropayment_tickets_processed_total`، `micropayment_tickets_won_total`،
`micropayment_tickets_duplicate_total`). يعرض هذا كله تدفق اليانصيب
من المحتمل أن يتمكن المشغلون من تصحيح مكاسب الدفعات الصغيرة و
ترحيل الائتمان مع نتائج التنظيم.

## التكامل Torii

يعرض Torii نقاط النهاية الأخيرة ليقوم الموردون بالإشارة إلى الاستخدام والقيادة
دورة حياة الاتفاقات بدون أسلاك محددة:- `POST /v2/sorafs/deal/usage` اقبل التليفزيون `DealUsageReport` وأعد الاتصال
  des résultats de comptabilité déterministes (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` إنهاء النافذة الرئيسية، عبر البث المباشر
  `DealSettlementRecord` الناتج عن `DealSettlementV1` المشفر في base64
  جاهزة للنشر في DAG de Governance.
- التغذية `/v2/events/sse` de Torii منتشرة في حالة خلل في التسجيلات
  `SorafsGatewayEvent::DealUsage` يستأنف كل نوع من الاستخدام (الفترة، ساعات قياس GiB،
  حاسبو التذاكر، الرسوم المحددة)، التسجيلات
  `SorafsGatewayEvent::DealSettlement` الذي يتضمن اللقطة القانونية لدفتر الأستاذ
  ainsi que le Digest/taille/base64 BLAKE3 de l'artefact de goovernance sur disque, et des notifications
  `SorafsGatewayEvent::ProofHealth` لأن الأجزاء التالية من PDP/PoTR معطلة (المورد، النافذة،
  حالة الإضراب/التهدئة، جبل العقوبة). يمكن للعملاء التصفية حسب المورد
  Pour réagir à de nouvelles télémétries, des règlements ou des notifications de santé des prophes sans Polling.

تشارك نقطتا النهاية في إطار الحصص SoraFS عبر النافذة الجديدة
`torii.sorafs.quota.deal_telemetry`، يسمح لمشغلي ضبط كمية التغذية
autorisé par déploiment.