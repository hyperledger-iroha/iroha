---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-fee-model
タイトル: تحديثات نموذج رسوم Nexus
説明: `docs/source/nexus_fee_model.md` はレーン وواجهات المطابقة です。
---

:::メモ
テストは `docs/source/nexus_fee_model.md` です。 بق النسختين متوافقتين بينما تهاجر الترجمات اليابانية والسبانية والبرتغاليةログインしてください。
:::

# حديثات نموذج رسوم Nexus

يلتقط موجه التسوية الموحد الان ايصالات حتمية لكل lane بحيث يستطيع المشغلون مطابقة خصومات Nexus です。

- ログインしてください - ログインしてください`docs/settlement-router.md`。 يشرح هذا الدليل كيف ترتبط المعلمات الموثقة هنا بتسليم خارطة الطريق NX-3 وكيف ينبغي لفرق SRE重要です。
- 評価 - 評価 (`pipeline.gas.units_per_gas`) 評価 `twap_local_per_xor` و`liquidity_profile` (`tier1`, `tier2`、`tier3`)、`volatility_class` (`stable`、`elevated`、`dislocated`)。ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット ヘアカット ヘアカットレーン。
- `LaneSettlementReceipt` を参照してください。 كل ايصال يخزن معرف المصدر المذي يقدمه المستدعي والمقدار المجهري المحلي وXOR المستحق فورا وXORヘアカット والتباين المحقق (`xor_variance_micro`) وطابع زمن الكتلة بالمللي ثانية。
- レーン/データスペース `lane_settlement_commitments` في `/v2/sumeragi/status`。 `total_local_micro` و`total_xor_due_micro` و`total_xor_after_haircut_micro` على مستوى الكتلة من اجل صادراتありがとうございます。
- عداد جديد `total_xor_variance_micro` يتتبع مقدار هامش الامان المستهلك (الفرق بين XOR المستحق والتوقع بعدヘアカット) (ヘアカット) `swap_metadata` معلمات التحويل الحتمية (TWAP وepsilon وliquidity profile وvolatility_class) حتى يتمكن المدققون من التحقق من重要な情報を確認してください。

يمكن للمستهلكين مراقبة `lane_settlement_commitments` الى جانب اللقطات الحالية لالتزامات レーン وdataspace للتحقق من ان مخازنヘアカットとスワップ Nexus を交換します。