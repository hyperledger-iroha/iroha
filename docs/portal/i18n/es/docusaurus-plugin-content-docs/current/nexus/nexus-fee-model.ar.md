---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-tarifa-nexus
título: تحديثات نموذج رسوم Nexus
descripción: نسخة مطابقة لـ `docs/source/nexus_fee_model.md` توثق ايصالات تسوية الـ lane وواجهات المطابقة.
---

:::nota المصدر القانوني
Utilice el botón `docs/source/nexus_fee_model.md`. ابق النسختين متوافقتين بينما تهاجر الترجمات اليابانية والعبرية والاسبانية والبرتغالية والفرنسية والروسية والعربية والاوردية.
:::

# تحديثات نموذج رسوم Nexus

يلتقط موجه التسوية الموحد الان ايصالات حتمية لكل lane بحيث يستطيع المشغلون مطابقة خصومات الغاز مع نموذج Número Nexus.- للحصول على بنية الموجه كاملة وسياسة المخزن المؤقت ومصفوفة القياس وتسلسل الاطلاق راجع `docs/settlement-router.md`. يشرح هذا الدليل كيف ترتبط المعلمات الموثقة هنا بتسليم خارطة الطريق NX-3 y ينبغي لفرق SRE مراقبة الموجه في الانتاج.
- تضمن تهيئة اصل الغاز (`pipeline.gas.units_per_gas`) قيمة عشرية `twap_local_per_xor` و`liquidity_profile` (`tier1`, `tier2`, او `tier3`) y `volatility_class` (`stable`, `elevated`, `dislocated`). Para obtener más información, consulte el enlace XOR TWAP y corte de pelo en el carril.
- Haga clic en el botón `LaneSettlementReceipt`. كل ايصال يخزن معرف المصدر الذي يقدمه المستدعي والمقدار المجهري المحلي وXOR المستحق فورا وXOR المتوقع بعد corte de pelo والتباين المحقق (`xor_variance_micro`) وطابع زمن الكتلة بالمللي ثانية.
- تنفيذ الكتلة يجمع الايصالات لكل lane/dataspace y عبر `lane_settlement_commitments` في `/v1/sumeragi/status`. Los dispositivos `total_local_micro`, `total_xor_due_micro` y `total_xor_after_haircut_micro` están conectados a una fuente de alimentación.
- عداد جديد `total_xor_variance_micro` يتتبع مقدار هامش الامان المستهلك (الفرق بين XOR المستحق والتوقع بعد haircut), y وتوثق `swap_metadata` معلمات التحويل الحتمية (TWAP, épsilon, perfil de liquidez y volatility_class) حتى يتمكن المدققون من التحقق من مدخلات التسعير بشكل مستقل عن اعدادات وقت التشغيل.يمكن للمستهلكين مراقبة `lane_settlement_commitments` الى جانب اللقطات الحالية لالتزامات lane anddataspace للتحقق من ان مخازن الرسوم وطبقات corte de pelo وتنفيذ intercambio تطابق نموذج رسوم Nexus المكون.