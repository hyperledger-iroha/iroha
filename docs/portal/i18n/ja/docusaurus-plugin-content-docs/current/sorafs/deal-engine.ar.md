---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ディールエンジン
タイトル: محرك الصفقات في SoraFS
サイドバーラベル: 重要
説明: SF-8 Torii وسطح التليمترية。
---

:::note ノート
テストは `docs/source/sorafs/deal_engine.md` です。 حرص على إبقاء الموقعين متوافقين ما دامت الوثائق القديمة نشطة。
:::

# محرك الصفقات في SoraFS

SF-8 SF-8 SoraFS ، موفراً
محاسبة حتمية لاتفاقات التخزين والاسترجاع بين
ありがとう。テスト Norito
المعرّفة في `crates/sorafs_manifest/src/deal.rs`، وتشمل شروط الصفقة،
重要な情報を確認してください。

SoraFS (`sorafs_node::NodeHandle`) 日本語版
مثيلاً من `DealEngine` لكل عملية عقدة. بما يلي:

- يتحقق من الصفقات ويسجلها باستخدام `DealTermsV1`؛
- يراكم رسومًا مقومة بـ XOR عند الإبلاغ عن استخدام النسخ المتماثل؛
- يقيّم نوافذ المدفوعات المصغرة الاحتمالية باستخدام أخذ عينات حتمية
  BLAKE3 さんو
- 台帳の管理。

ニュース ニュース ニュース ニュース ニュース ニュース ニュース ニュース ニュース
API を使用してください。評価 `DealSettlementV1`،
SF-12 の評価 OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`、`deal_expected_charge_nano`、`deal_client_debit_nano`、
`deal_outstanding_nano`、`deal_bond_slash_nano`、`deal_publish_total`) 重要な Torii
SLO。 وتركّز العناصر اللاحقة على أتمتة スラッシュ التي يبدأها المدققون
ログインしてください。

回答: `sorafs.node.micropayment_*`:
`micropayment_charge_nano`、`micropayment_credit_generated_nano`、
`micropayment_credit_applied_nano`、`micropayment_credit_carry_nano`、
`micropayment_outstanding_nano`, ログイン
(`micropayment_tickets_processed_total`、`micropayment_tickets_won_total`、
`micropayment_tickets_duplicate_total`)。 تكشف هذه الإجماليات عن تدفق اليانصيب
حتمالي لتمكين المشغّلين من ربط مكاسب المدفوعات المصغرة وترحيل الرصيد
ありがとうございます。

## テスト Torii

تعرض Torii نقاط نهاية مخصصة كي يتمكن المزوّدون من الإبلاغ عن الاستخدام وتحريك
配線手順:

- `POST /v2/sorafs/deal/usage` يقبل تليمترية `DealUsageReport` ويعيد
  (`UsageOutcome`)。
- `POST /v2/sorafs/deal/settle` ينهى النافذة الحالية، ويبث
  `DealSettlementRecord` セキュリティ `DealSettlementV1` セキュリティ Base64
  DAG は、次のことを意味します。
- يغذي `/v2/events/sse` في Torii الآن سجلات `SorafsGatewayEvent::DealUsage`
  التي تلخص كل إرسال استخدام (epoch، ساعات GiB المقاسة، عدّادات التذاكر،
  (`SorafsGatewayEvent::DealSettlement`)
  台帳のダイジェスト/ベース64 BLAKE3
  और देखें
  PDP/PoTR (المزوّد، النافذة، حالة ストライク/クールダウン مبلغ العقوبة)。
  يمكن للمستهلكين التصفية حسب المزوّد للتفاعل مع تليمترية جديدة أو تسويات أو تنبيهات
  投票。

يشارك كلا نقطتي النهاية في إطار حصص SoraFS عبر نافذة
`torii.sorafs.quota.deal_telemetry` الجديدة، ما يسمح للمشغّلين بضبط معدل الإرسال
重要です。