---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نموذج رسوم العلاقة
العنوان: تحديثات نموذج الرسومي Nexus
الوصف: نسخة مطابقة لـ `docs/source/nexus_fee_model.md` توثق ايصالات اتفاقية الـ حارة وواجهات المطابقة.
---

:::ملاحظة المصدر الساقط
احترام هذه الصفحة `docs/source/nexus_fee_model.md`. ابق النسختين متوافقتين بينما تهاجر الترجمات اليابانية والعبرية والاسبانية والبرتغالية والفرنسية والروسية والعربية والاوردية.
:::

# تحديثات الموديل الرسومي Nexus

يتطلع إلى تشغيل الموحد الان ايصالات حتمية لكل حارة بحيث يستطيع تشغيل خصومات الغاز مع موديل رسومي Nexus.

- للحصول على أسس الموجه كاملة وسياسة مخزنة مؤقتا ومصفوفة القياس وتسلسل الاطلاق `docs/settlement-router.md`. يشرح هذا الدليل كيف ترتبط المعلمات الموثقة هنا بتسليم خارطة الطريق NX-3 وكيف ينبغي لفرق SRE مراقبة الموجه في الإنتاج.
- تتضمن تهيئة اصل الغاز (`pipeline.gas.units_per_gas`) قيمة عشرية `twap_local_per_xor` و`liquidity_profile` (`tier1`, `tier2`, او `tier3`) و`volatility_class` (`stable`، `elevated`، `dislocated`). تغذي هذه المؤشرات المتشوقة بحيث تطابق تحديد XOR حيث يتوقع TWAP وطبقة الشعر الخاصة بالـ حارة.
- كلفترى تدفع الغاز `LaneSettlementReceipt`. كل ايصال يخزن معرف المصدر الذي يقدمه المستدعي والمقدار المجهري المحلي وXOR المستحق فورا وXOR التخصصية بعد قص الشعر والمراقب المحقق (`xor_variance_micro`) وطابعة زهرة الحلوى بالملي ثانية.
- التكامل الشامل لليصالات لكل حارة/مساحة بيانات وينشرها عبر `lane_settlement_commitments` في `/v2/sumeragi/status`. تم عرض الاجماليات `total_local_micro` و`total_xor_due_micro` و`total_xor_after_haircut_micro` مجمعة على مستوى الشفافية من اجل مستندات المطابقة الذهبية.
- عداد جديد `total_xor_variance_micro` يتتبع مقدار تخفيضات الامان المستهلك (الفرق بين XOR المستحق والتوقع بعد قص الشعر)، وتوثق `swap_metadata` معلمات عكس الحتمية (TWAP wepsilon وliquidity Profile وvolatility_class) حتى يتوقع المون من التحقق من إيصالات التسعير بشكل مستقل عن تعدادات وقت التشغيل.

يمكن للمستهلكين مراقبة `lane_settlement_commitments` إلى جانب الجزء الحالي من التزامات حارة وdataspace المعتمدة من ان مخزون الصور وتطبيقات قصات الشعر المتطورة مبادلة تطابق نموذج الرسم Nexus المكون.