---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle de frais-nexus
titre : تحديثات نموذج رسوم Nexus
description: نسخة مطابقة لـ `docs/source/nexus_fee_model.md` توثق ايصالات تسوية الـ lane وواجهات المطابقة.
---

:::note المصدر القانوني
Il s'agit de la référence `docs/source/nexus_fee_model.md`. ابق النسختين متوافقتين بينما تهاجر الترجمات اليابانية والعبرية والاسبانية والبرتغالية والفرنسية والروسية والعربية والاوردية.
:::

# تحديثات نموذج رسوم Nexus

يلتقط موجه التسوية الموحد الان ايصالات حتمية لكل lane بحيث يستطيع المشغلون مطابقة خصومات الغاز مع Utilisez Nexus.- للحصول على بنية الموجه كاملة وسياسة المخزن المؤقت ومصفوفة القياس وتسلسل الاطلاق راجع `docs/settlement-router.md`. يشرح هذا الدليل كيف ترتبط المعلمات الموثقة هنا بتسليم خارطة الطريق NX-3 et لفرق SRE مراقبة الموجه في الانتاج.
- تتضمن تهيئة اصل الغاز (`pipeline.gas.units_per_gas`) pour `twap_local_per_xor` et `liquidity_profile` (`tier1`, `tier2`, et `tier3`) et `volatility_class` (`stable`, `elevated`, `dislocated`). Vous pouvez utiliser la coupe de cheveux XOR pour TWAP et la coupe de cheveux sur la voie.
- كل معاملة تدفع الغاز تسجل `LaneSettlementReceipt`. كل ايصال يخزن معرف المصدر الذي يقدمه المستدعي والمقدار المجهري المحلي وXOR المستحق فورا وXOR المتوقع بعد coupe de cheveux والتباين المحقق (`xor_variance_micro`) وطابع زمن الكتلة بالمللي ثانية.
- La gestion des voies/espaces de données est basée sur `lane_settlement_commitments` et `/v1/sumeragi/status`. تعرض الاجماليات `total_local_micro` و`total_xor_due_micro` و`total_xor_after_haircut_micro` مجمعة مستوى الكتلة من اجل صادرات المطابقة الليلية.
- عداد جديد `total_xor_variance_micro` يتتبع مقدار هامش الامان المستهلك (الفرق بين XOR المستحق والتوقع بعد haircut)، وتوثق `swap_metadata` Fichiers de données (TWAP, profil de liquidité et profil de liquidité et classe de volatilité) مستقل عن اعدادات وقت التشغيل.يمكن للمستهلكين مراقبة `lane_settlement_commitments` الى جانب اللقطات الحالية لالتزامات lane وdataspace للتحقق من ان مخازن الرسوم وطبقات coupe de cheveux et échange تطابق نموذج رسوم Nexus المكون.