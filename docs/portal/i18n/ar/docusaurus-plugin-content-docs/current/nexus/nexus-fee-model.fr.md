---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نموذج رسوم العلاقة
العنوان: Mises a jour du modele de frais Nexus
الوصف: Miroir de `docs/source/nexus_fee_model.md`، توثيق قواعد تنظيم الممرات وأسطح التسوية.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/nexus_fee_model.md`. قم بحفظ نسختين من الترجمات المتوافقة مع هجرة الترجمات اليابانية والعبرية والإسبانية والبرتغالية والفرنسية والروسية والعربية والأردوية.
:::

# Miss a jour du modele de frais Nexus

يوحد مسار التنظيم الالتقاط مع الاحتفاظ بالمحددات المحددة حسب المسار حتى يتمكن المشغلون من تسوية ديون الغاز بنموذج الرسوم Nexus.

- من أجل البنية الكاملة للمسار، وسياسة المخزن المؤقت، ومصفوفة القياس عن بعد، وتسلسل النشر، انظر `docs/settlement-router.md`. يشرح هذا الدليل التعليق على معلمات المستندات التي يتم ربطها بالقابلية للعيش في خريطة الطريق NX-3 والتعليق على SRE التي تقوم بمراقبة المسار أثناء الإنتاج.
- يتضمن تكوين الغاز النشط (`pipeline.gas.units_per_gas`) رقمًا عشريًا `twap_local_per_xor`، و`liquidity_profile` (`tier1`، و`tier2`، و`tier3`)، وآخرون `volatility_class` (`stable`، `elevated`، `dislocated`). تشير هذه المؤشرات إلى مسار التحكم الذي يشير إلى أن الغلاف XOR يتوافق مع TWAP canonique ولون قص الشعر في المسار.
- كل معاملة تدفعها لتسجيل الغاز `LaneSettlementReceipt`. يمكنك العثور على مخزون من مصدر التعريف المقدم من المتقدم، والتركيب المحلي الصغير، وXOR للتحكم الفوري، وXOR يحضر بعد قص الشعر، والتباين المحقق (`xor_variance_micro`)، وبيانات كتلة الكتلة بالمللي ثانية.
- يؤدي تنفيذ الكتل إلى تجميع العناصر المخصصة للمسار/مساحة البيانات ونشرها عبر `lane_settlement_commitments` في `/v1/sumeragi/status`. تعرض جميع العناصر `total_local_micro` و`total_xor_due_micro` و`total_xor_after_haircut_micro` بالإضافة إلى الكتلة من أجل صادرات المصالحة الليلية.
- كمبيوتر جديد `total_xor_variance_micro` يناسب هامش الاستهلاك الآمن (الفرق بين XOR وانتظار ما بعد الحلاقة)، و`swap_metadata` يوثق معلمات التحويل المحددة (TWAP، epsilon، ملف تعريف السيولة، وفئة التقلب) حتى يتمكن المدققون من التحقق مداخل التغطية مستقلة عن تكوين التنفيذ.

يمكن للمستهلكين متابعة `lane_settlement_commitments` من خلال اللقطات الموجودة في التزامات المسار ومساحة البيانات للتحقق من تكوين مخازن الرسوم المؤقتة وألواح قص الشعر وتنفيذ المبادلة المقابلة لنموذج التكاليف Nexus.