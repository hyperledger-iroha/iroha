---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نموذج رسوم العلاقة
العنوان: Actualizaciones del modelo de tarifas de Nexus
الوصف: Espejo de `docs/source/nexus_fee_model.md`، الذي يوثق إيصالات تصفية الممرات وأسطح التوفيق.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/nexus_fee_model.md`. احتفظ بنسخ من السفراء المختارين من خلال التقاليد اليابانية والعبرية والإسبانية والبرتغالية والفرنسية والروسية والعربية والأردية المهاجرة.
:::

# تحديثات نموذج التعريفة Nexus

يلتقط جهاز توجيه التصفية الموحد الآن الإيصالات المحددة حسب المسار حتى يتمكن المشغلون من التوفيق بين ديون الغاز مقابل نموذج التعريفة Nexus.

- لبنية جهاز التوجيه الكاملة، وسياسة المخزن المؤقت، ومصفوفة القياس عن بعد، وتأمين التشغيل، و`docs/settlement-router.md`. تم شرح هذا الدليل مثل المعلمات الموثقة التي يتم ربطها بخريطة الطريق NX-3 القابلة للدمج، كما يجب على SREs مراقبة جهاز التوجيه والإنتاج.
- يتضمن تكوين الغاز النشط (`pipeline.gas.units_per_gas`) رقمًا عشريًا `twap_local_per_xor`، و`liquidity_profile` (`tier1`، و`tier2`، و`tier3`)، وواحدًا `volatility_class` (`stable`، `elevated`، `dislocated`). يتم دعم هذه العصابات من خلال جهاز توجيه التصفية بحيث تتطابق عملية التصفية الناتجة عن XOR مع TWAP Canonico ومستوى قص الشعر على المسار.
- كل معاملة يتم تسجيل الغاز فيها `LaneSettlementReceipt`. كل ما يتم استلامه هو المعرف الأصلي المقدم من المتصل، والميكرو مونتو المحلي، وXOR المطلوب على الفور، وXOR المتوقع بعد قص الشعر، والتباين الذي تم تحقيقه (`xor_variance_micro`)، وعلامة وقت الكتلة بالملايين.
- تنفيذ الكتل المجمعة على المسار/مساحة البيانات والنشر عبر `lane_settlement_commitments` و`/v2/sumeragi/status`. توضح الإجماليات `total_local_micro`، و`total_xor_due_micro`، و`total_xor_after_haircut_micro`، إجمالي الكتلة لصادرات التوفيق الليلية.
- حاسب جديد `total_xor_variance_micro` له هامش أمان للاستهلاك (الفرق بين XOR المستحق والتوقعات بعد قص الشعر)، و`swap_metadata` يوثق معلمات تحديد التحويل (TWAP، epsilon، ملف تعريف السيولة، وvolatility_class) لذلك يمكن للمراجعين التحقق من إدخالات التنظيف بشكل مستقل عن التكوين في وقت التشغيل.

يمكن للمستهلكين ملاحظة `lane_settlement_commitments` جنبًا إلى جنب مع اللقطات الموجودة لالتزامات المسار ومساحة البيانات للتحقق من أن المخازن المؤقتة للتعريفات ومستويات القطع وتنفيذ المقايضة تتزامن مع نموذج تعريفات Nexus الذي تم تكوينه.