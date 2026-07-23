---
lang: ar
direction: rtl
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 532c57a0dae54224af0d30640edf8a3cbc8ac9a1df7d73b563bd16c3a635aec1
source_last_modified: "2026-01-08T19:45:50.411145+00:00"
translation_last_reviewed: 2026-01-08
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
- يجب أن تحمل كل معاملة الحقل المهيكل والملزم بالتوقيع `fee_payment`
  (`FeePaymentIntent`). يحدد هذا الحقل جهة الدفع، إما سلطة المعاملة أو برنامج
  راع واحد بعينه ومراجعته غير القابلة للتغيير، ويتضمن الحدود القصوى الموقعة
  لمكونات الرسوم وحد gas موجبا عند الحاجة. تُرفض مفاتيح البيانات الوصفية
  القديمة `fee_sponsor` و`gas_limit` و`gas_asset_id`.
- اطلب التسعير قبل التوقيع: ابن الحمولة غير الموقعة كاملة، واجعل سلطتها
  تصادق على `POST /v1/fees/quote`، وافحص القصد المقترح، واستبدل
  `payload.fee_payment` فقط، ثم وقّع الحمولة نفسها وأرسلها. التسعير لقطة
  ملاحظة وليس حجزا؛ ويعيد القبول فحص حالة السجل الحالية.
- يدعم settlement المباشر الدفع من السلطة أو من برنامج راع واحد بعينه. أما
  settlement القائم على receipts (`lane_relay_burn`) فهو مقصور على راع
  بعينه: تُرفض رسوم Nexus المدفوعة من السلطة بالرمز
  `relay_capacity_unavailable` لأن رصيد السلطة ليس قفل مصدر receipts موثقا.
- كل معاملة تدفع gas تسجل `LaneSettlementReceipt`. يحفظ كل ايصال معرّف المصدر الذي يقدمه المستدعي،
  مقدار الميكرو المحلي، قيمة XOR المستحقة فورا، وقيمة XOR المتوقعة بعد haircut،
  هامش الامان المحقق (`xor_variance`)، والطابع الزمني للكتلة بالميلي ثانية.
- تنفيذ الكتلة يجمع الايصالات لكل lane/dataspace وينشرها عبر `lane_settlement_commitments` في
  `/v1/sumeragi/status`. تظهر المجاميع `total_local_amount` و`total_xor_due` و
  `total_xor_after_haircut` مجمعة على مستوى الكتلة لتصديرات التسوية الليلية.
- عداد جديد `total_xor_variance` يتتبع مقدار هامش الامان المستهلك (الفرق بين XOR المستحق
  والتوقع بعد haircut)، و`swap_metadata` يوثق معلمات التحويل الحتمية (TWAP، epsilon،
  liquidity profile، و volatility_class) لكي يتمكن المدققون من التحقق من مدخلات التسعير
  بصورة مستقلة عن تهيئة وقت التشغيل.

يمكن للمستهلكين متابعة `lane_settlement_commitments` جنبا الى جنب مع لقطات commitments الحالية
للـ lane والـ dataspace للتحقق من تطابق مخازن الرسوم، طبقات haircut، وتنفيذ swap مع نموذج
رسوم Nexus المهيأ.

</div>
