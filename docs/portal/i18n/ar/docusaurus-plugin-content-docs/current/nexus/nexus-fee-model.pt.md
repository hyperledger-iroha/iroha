---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نموذج رسوم العلاقة
العنوان: Atualizacoes do modelo de Taxas do Nexus
الوصف: اسم `docs/source/nexus_fee_model.md`، توثيق وصفات تصفية الممرات وأسطح التسوية.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تعكس `docs/source/nexus_fee_model.md`. احتفظ بالأدعية المنسوخة على النحو التالي مثل المترجمين اليابانيين والعبريين والإسبانيين والبرتغاليين والفرنسيين والروسيين والعرب والأردية.
:::

# Atualizacoes do modelo de Taxas do Nexus

سيلتقط مدير التصفية الموحد الآن حصصًا محددة حسب المسار حتى يتمكن المشغلون من التوفيق بين ديون الغاز مع نموذج الضرائب Nexus.

- لبنية الدوار الكاملة، وسياسة المخزن المؤقت، ومصفوفة القياس عن بعد، وتسلسل الطرح، حتى `docs/settlement-router.md`. يشرح هذا الدليل كيف أن المعلمات الموثقة تتصل بتفاصيل خريطة الطريق NX-3، كما يجب على SREs مراقبة أو تدوير المنتج.
- تكوين الغاز (`pipeline.gas.units_per_gas`) بما في ذلك الرقم العشري `twap_local_per_xor`، أو `liquidity_profile` (`tier1`، أو `tier2`، أو `tier3`)، وما إلى ذلك. `volatility_class` (`stable`، `elevated`، `dislocated`). تم تجهيز هذه العناصر بدوارة تصفية بحيث تتوافق نتيجة تقطيع XOR مع TWAP Canon ومستوى قص الشعر في المسار.
- كل تحويل يتم تسجيله بالغاز هو `LaneSettlementReceipt`. كل ما يستلمه مخزن أو معرّف الأصل الناتج عن شارة الاتصال، أو القيمة الصغيرة المحلية، أو XOR يتم توزيعه على الفور، أو XOR منتظر بعد قصة الشعر، أو التباين المحقق (`xor_variance_micro`)، أو الطابع الزمني للكتلة على ملايين المئات.
- تنفيذ الكتل التي يتم جمعها من خلال المسار/مساحة البيانات ونشرها عبر `lane_settlement_commitments` في `/v1/sumeragi/status`. كل ما في الأمر هو `total_local_micro`، `total_xor_due_micro` و`total_xor_after_haircut_micro`، لا يوجد كتلة للتصدير في أيام التوفيق.
- حساب جديد `total_xor_variance_micro` يحدد مقدار هامش أمان المستهلك (الفرق بين قسم XOR وتوقعات ما بعد قص الشعر)، و`swap_metadata` يوثق معلمات المحادثة الحتمية (TWAP، epsilon، ملف تعريف السيولة e) volatility_class) حتى يتمكن المدققون من التحقق من إدخالات المنتج بشكل مستقل عن التكوين في وقت التشغيل.

يمكن للمستهلكين مرافقة `lane_settlement_commitments` جنبًا إلى جنب مع اللقطات الموجودة لالتزامات المسار ومساحة البيانات للتحقق من أن المخازن المؤقتة للضرائب ومستويات قص الشعر والتنفيذ تتوافق مع نموذج الضرائب الذي تم تكوينه Nexus.