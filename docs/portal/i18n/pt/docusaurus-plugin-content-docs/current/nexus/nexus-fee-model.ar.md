---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo de taxa de nexo
título: تحديثات نموذج رسوم Nexus
description: Use o `docs/source/nexus_fee_model.md` para abrir a pista e a pista.
---

:::note المصدر القانوني
Verifique o valor `docs/source/nexus_fee_model.md`. ابق النسختين متوافقتين بينما تهاجر الترجمات اليابانية والعبرية والاسبانية والبرتغالية والفرنسية والروسية والعربية والاوردية.
:::

# تحديثات نموذج رسوم Nexus

يلتقط موجه التسوية الموحد الان ايصالات حتمية لكل lane بحيث يستطيع المشغلون مطابقة خصومات الغاز Use o modelo Nexus.

- للحصول على بنية الموجه كاملة وسياسة المخزن المؤقت ومصفوفة القياس وتسلسل الاطلاق راجع `docs/settlement-router.md`. Você pode usar o NX-3 para usar o NX-3 e o SRE مراقبة الموجه في الانتاج.
- Você pode usar o produto `twap_local_per_xor` e `liquidity_profile` (`tier1`, `tier2`, e `tier3`) e `volatility_class` (`stable`, `elevated`, `dislocated`). Corte de cabelo XOR TWAP e corte de cabelo pista.
- A chave de segurança é `LaneSettlementReceipt`. كل ايصال يخزن معرف المصدر الذي يقدمه المستدعي والمقدار المجهري المحلي وXOR المستحق فورا وXOR المتوقع Corte de cabelo e corte de cabelo (`xor_variance_micro`).
- Verifique se a faixa/espaço de dados está localizada em `lane_settlement_commitments` em `/v2/sumeragi/status`. Verifique os valores `total_local_micro`, `total_xor_due_micro` e `total_xor_after_haircut_micro` para obter mais informações sobre o produto. المطابقة الليلية.
- Você pode usar `total_xor_variance_micro` para obter informações sobre o valor do XOR ou do XOR corte de cabelo), وتوثق `swap_metadata` معلمات التحويل الحتمية (TWAP e epsilon e perfil de liquidez e volatility_class) حتى يتمكن المدققون من التحقق من Você pode fazer isso com cuidado e sem problemas.

يمكن للمستهلكين مراقبة `lane_settlement_commitments` الى جانب اللقطات الحالية لالتزامات lane وdataspace للتحقق من ان مخازن Corte de cabelo وتنفيذ swap تطابق نموذج رسوم Nexus المكون.