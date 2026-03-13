---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: نموذج رسوم العلاقة
العنوان: لجنة تصنيع النماذج Nexus
الوصف: Zercalo `docs/source/nexus_fee_model.md`، وثائقي عن الحياة في الممر والتوجيه العلوي.
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/nexus_fee_model.md`. المزيد من النسخ المتزامنة، عبر الهجرة إلى اليابانية، الإيفريت، الأسبانية، البرتغالية، الفرنسية، الروسية والعربية والأوردية.
:::

# لجنة نماذج التسليم Nexus

يقوم أحد أجهزة التوجيه بإصلاح درجة تحديد المسافة في الممر بحيث يتمكن المشغلون من فحص الغاز باستخدام نموذج اللجنة Nexus.

- لجهاز توجيه معماري كامل، وحواجز سياسية، وأجهزة قياس المسافة، والطرح الأحدث. `docs/settlement-router.md`. هذا أمر رائع، مثل المعلمات، والوصف، والمرتبطة بخريطة الطريق المرسلة NX-3، وكيفية مراقبة SRE لجهاز التوجيه في بيع.
- يتضمن نشاط تكوين الغاز (`pipeline.gas.units_per_gas`) الاسم المناسب `twap_local_per_xor`، `liquidity_profile` (`tier1`، `tier2` أو `tier3`) و`volatility_class` (`stable`، `elevated`، `dislocated`). يتم وضع هذا العلم في جهاز توجيه التسوية، وهو جهاز توجيه XOR الذي يدعم TWAP القانوني وقص شعر جديد للممر.
- كل معاملة غاز مملوء بالغاز، اكتب `LaneSettlementReceipt`. في حالة وجود إيصال استلام، مُعرِّف تعريفي مُسبق، مجموع صغير محلي، XOR إلى لوحة غير منتظمة، بجانب XOR بعد قص الشعر، والتباين الفعلي (`xor_variance_micro`) وكتلة زمنية صغيرة بالمللي ثانية.
- قم بتجميع الإيصالات في الممر/مساحة البيانات ونشرها عبر `lane_settlement_commitments` في `/v2/sumeragi/status`. توضح هذه العناصر `total_local_micro` و`total_xor_due_micro` و`total_xor_after_haircut_micro`، وهي مغمورة في كتلة للتدفئة الليلية.
- تم حذف الكمبيوتر الجديد `total_xor_variance_micro`، حيث تم إلغاء الحماية الآمنة (الفرق بين XOR الجديد و المراقبة بعد الحلاقة)، `swap_metadata` توثق محادثات تحديد معلمات (TWAP، epsilon، ملف تعريف السيولة وvolatility_class)، لمدققي الحسابات يمكنك التحقق من المعلمات غير المرغوب فيها لتكوين النطاق.

يمكن للمشتركين الاطلاع على `lane_settlement_commitments` بعيدًا عن التزامات الصور الخاصة بالمسار ومساحة البيانات لقراءة تلك المخازن المؤقتة عمولة، قصة شعر رائعة ومبادلة معززة لنموذج قوي للجنة Nexus.