---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-fee-model
title: تحديثات نموذج رسوم Nexus
description: نسخة مطابقة لـ `docs/source/nexus_fee_model.md` توثق ايصالات تسوية الـ lane وواجهات المطابقة.
---

:::note المصدر القانوني
تعكس هذه الصفحة `docs/source/nexus_fee_model.md`. ابق النسختين متوافقتين بينما تهاجر الترجمات اليابانية والعبرية والاسبانية والبرتغالية والفرنسية والروسية والعربية والاوردية.
:::

# تحديثات نموذج رسوم Nexus

يلتقط موجه التسوية الموحد الان ايصالات حتمية لكل lane بحيث يستطيع المشغلون مطابقة خصومات الغاز مع نموذج رسوم Nexus.

- للحصول على بنية الموجه كاملة وسياسة المخزن المؤقت ومصفوفة القياس وتسلسل الاطلاق راجع `docs/settlement-router.md`. يشرح هذا الدليل كيف ترتبط المعلمات الموثقة هنا بتسليم خارطة الطريق NX-3 وكيف ينبغي لفرق SRE مراقبة الموجه في الانتاج.
- تتضمن تهيئة اصل الغاز (`pipeline.gas.units_per_gas`) قيمة عشرية `twap_local_per_xor` و`liquidity_profile` (`tier1`, `tier2`, او `tier3`) و`volatility_class` (`stable`, `elevated`, `dislocated`). تغذي هذه العلامات موجه التسوية بحيث تطابق تسعيرة XOR الناتجة TWAP القانوني وطبقة haircut الخاصة بالـ lane.
- كل معاملة تدفع الغاز تسجل `LaneSettlementReceipt`. كل ايصال يخزن معرف المصدر الذي يقدمه المستدعي والمقدار المجهري المحلي وXOR المستحق فورا وXOR المتوقع بعد haircut والتباين المحقق (`xor_variance_micro`) وطابع زمن الكتلة بالمللي ثانية.
- تنفيذ الكتلة يجمع الايصالات لكل lane/dataspace وينشرها عبر `lane_settlement_commitments` في `/v1/sumeragi/status`. تعرض الاجماليات `total_local_micro` و`total_xor_due_micro` و`total_xor_after_haircut_micro` مجمعة على مستوى الكتلة من اجل صادرات المطابقة الليلية.
- عداد جديد `total_xor_variance_micro` يتتبع مقدار هامش الامان المستهلك (الفرق بين XOR المستحق والتوقع بعد haircut)، وتوثق `swap_metadata` معلمات التحويل الحتمية (TWAP وepsilon وliquidity profile وvolatility_class) حتى يتمكن المدققون من التحقق من مدخلات التسعير بشكل مستقل عن اعدادات وقت التشغيل.

يمكن للمستهلكين مراقبة `lane_settlement_commitments` الى جانب اللقطات الحالية لالتزامات lane وdataspace للتحقق من ان مخازن الرسوم وطبقات haircut وتنفيذ swap تطابق نموذج رسوم Nexus المكون.
