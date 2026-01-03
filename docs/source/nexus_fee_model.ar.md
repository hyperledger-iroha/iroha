---
lang: ar
direction: rtl
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e02872dbcb6d92d8be4d40fc2864f28fc6564391640a6ea67768a1f837b57e0f
source_last_modified: "2025-11-15T20:09:59.438546+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_fee_model.md -->

# تحديثات نموذج رسوم Nexus

يلتقط موجّه التسوية الموحّد الان ايصالات حتمية لكل lane كي يتمكن المشغلون من
مطابقة خصومات gas مع نموذج رسوم Nexus.

- لمراجعة معمارية الموجّه كاملة، سياسة المخازن المؤقتة، مصفوفة القياس عن بعد، وتسلسل الاطلاق،
  راجع `docs/settlement-router.md`. يوضح الدليل كيف ترتبط المعلمات الموثقة هنا بتسليم NX-3 في
  خارطة الطريق وكيف ينبغي لـ SREs مراقبة الموجّه في الانتاج.
- تهيئة اصل gas (`pipeline.gas.units_per_gas`) تتضمن قيمة عشرية `twap_local_per_xor`،
  و`liquidity_profile` (`tier1`, `tier2`, او `tier3`)، و`volatility_class` (`stable`, `elevated`,
  `dislocated`). تغذي هذه الاعلام موجّه التسوية كي يطابق تسعير XOR الناتج TWAP القياسي و
  طبقة haircut الخاصة بالـ lane.
- كل معاملة تدفع gas تسجل `LaneSettlementReceipt`. يحفظ كل ايصال معرّف المصدر الذي يقدمه المستدعي،
  مقدار الميكرو المحلي، قيمة XOR المستحقة فورا، وقيمة XOR المتوقعة بعد haircut،
  هامش الامان المحقق (`xor_variance_micro`)، والطابع الزمني للكتلة بالميلي ثانية.
- تنفيذ الكتلة يجمع الايصالات لكل lane/dataspace وينشرها عبر `lane_settlement_commitments` في
  `/v1/sumeragi/status`. تظهر المجاميع `total_local_micro` و`total_xor_due_micro` و
  `total_xor_after_haircut_micro` مجمعة على مستوى الكتلة لتصديرات التسوية الليلية.
- عداد جديد `total_xor_variance_micro` يتتبع مقدار هامش الامان المستهلك (الفرق بين XOR المستحق
  والتوقع بعد haircut)، و`swap_metadata` يوثق معلمات التحويل الحتمية (TWAP، epsilon،
  liquidity profile، و volatility_class) لكي يتمكن المدققون من التحقق من مدخلات التسعير
  بصورة مستقلة عن تهيئة وقت التشغيل.

يمكن للمستهلكين متابعة `lane_settlement_commitments` جنبا الى جنب مع لقطات commitments الحالية
للـ lane والـ dataspace للتحقق من تطابق مخازن الرسوم، طبقات haircut، وتنفيذ swap مع نموذج
رسوم Nexus المهيأ.

</div>
