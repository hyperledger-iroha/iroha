---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈیل انجن
عنوان: SoraFS معاہدہ انجن
سائڈبار_لیبل: معاہدہ انجن
تفصیل: SF-8 معاہدے کے انجن کا خلاصہ ، Torii اور ٹیلی میٹری سطحوں کے ساتھ انضمام۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/deal_engine.md` کی عکاسی کرتا ہے۔ جب لیگیسی دستاویزات ابھی بھی متحرک ہیں تو دونوں مقامات کو سیدھ میں رکھیں۔
:::

#SoraFS معاہدہ انجن

SF-8 روٹ لائن SoraFS ڈیل انجن کو متعارف کراتا ہے ، جو فراہم کرتا ہے
اسٹوریج اور بازیافت کے معاہدوں کے لئے ڈٹرمینسٹک اکاؤنٹنگ
کلائنٹ اور سپلائرز۔ معاہدوں کو پے لوڈ Norito کے ساتھ بیان کیا گیا ہے
`crates/sorafs_manifest/src/deal.rs` میں بیان کردہ ، جس میں معاہدے کی شرائط کا احاطہ کیا گیا ہے ،
بانڈ مسدود کرنا ، امکانی مائکروپیمنٹس اور تصفیہ ریکارڈ۔

SoraFS (`sorafs_node::NodeHandle`) سرایت شدہ کارکن اب انسٹیٹیٹ کرتا ہے a
`DealEngine` ہر نوڈ عمل کے لئے۔ انجن:

- `DealTermsV1` کا استعمال کرتے ہوئے معاہدوں کی توثیق اور رجسٹر کرتا ہے۔
- نقل کے استعمال کی اطلاع دیتے وقت XOR سے منسلک چارجز جمع کرتا ہے۔
- تعی .ن کے نمونے لینے کا استعمال کرتے ہوئے امکانی مائکروپیمنٹ ونڈوز کا اندازہ کرتا ہے
  بلیک 3 پر مبنی ؛ اور
- اشاعت کے قابل لیجر اسنیپ شاٹس اور تصفیہ پے لوڈ تیار کرتا ہے
  گورننس کی

یونٹ ٹیسٹ میں توثیق ، ​​مائکروپیمنٹ کا انتخاب ، اور ادائیگی کے بہاؤ کا احاطہ کیا گیا ہے۔
تصفیہ تاکہ تاجر اعتماد کے ساتھ APIs کا استعمال کرسکیں۔
بستیوں میں اب گورننس پے لوڈز `DealSettlementV1` جاری کرتے ہیں ،
براہ راست SF-12 پبلشنگ پائپ لائن سے مربوط ہونا ، اور سیریز کو اپ ڈیٹ کرنا
اوپن لیمیٹری `sorafs.node.deal_*`
(`deal_settlements_total` ، `deal_expected_charge_nano` ، `deal_client_debit_nano` ،
`deal_outstanding_nano` ، `deal_bond_slash_nano` ، `deal_publish_total`) Torii کے لئے اور
SLOS کی درخواست. مندرجہ ذیل اقدامات آٹومیشن کو ختم کرنے پر توجہ دیتے ہیں
آڈیٹرز کے ذریعہ اور گورننس پالیسی کے ساتھ منسوخی کے الفاظ کو مربوط کرنے میں شروع کیا گیا۔

استعمال ٹیلی میٹری `sorafs.node.micropayment_*` میٹرکس سیٹ کو بھی کھلاتا ہے:
`micropayment_charge_nano` ، `micropayment_credit_generated_nano` ،
`micropayment_credit_applied_nano` ، `micropayment_credit_carry_nano` ،
`micropayment_outstanding_nano` ، اور ٹکٹ کاؤنٹر
(`micropayment_tickets_processed_total` ، `micropayment_tickets_won_total` ،
`micropayment_tickets_duplicate_total`)۔ یہ کل لاٹری کے بہاؤ کو بے نقاب کرتے ہیں
لہذا آپریٹرز مائکروپیمنٹ جیت اور باہمی تعلق رکھتے ہیں
تصفیہ کے نتائج کے ساتھ کریڈٹ کیری اوور۔

Torii کے ساتھ ## انضمام

Torii فراہم کرنے والوں کے لئے استعمال کی اطلاع دینے اور سائیکل کو چلانے کے لئے سرشار اختتامی نکات کو بے نقاب کرتا ہے
کسٹم وائرنگ کے بغیر معاہدہ زندگی:- `POST /v2/sorafs/deal/usage` `DealUsageReport` ٹیلی میٹری کو قبول کرتا ہے اور واپسی کرتا ہے
  ڈٹرمینسٹک اکاؤنٹنگ کے نتائج (`UsageOutcome`)۔
- `POST /v2/sorafs/deal/settle` موجودہ ونڈو کو ختم کرتا ہے ، منتقل کرتے ہوئے
  اس کے نتیجے میں `DealSettlementRecord` BASE64 میں `DealSettlementV1` کے ساتھ
  گورننس ڈی اے جی میں اشاعت کے لئے تیار ہیں۔
- Torii سے `/v2/events/sse` کو فیڈ کریں اب ریکارڈز `SorafsGatewayEvent::DealUsage` منتقل کرتا ہے
  اس کا خلاصہ ہر استعمال جمع کرانے (عہد ، ماپا گیب گھنٹے ، ٹکٹ کاؤنٹرز ،
  تعصب کے الزامات) ، ریکارڈ `SorafsGatewayEvent::DealSettlement`
  جس میں آبادکاری لیجر کے علاوہ ڈائجسٹ/سائز/بیس 64 کی کیننیکل اسنیپ شاٹ شامل ہے
  ڈسک گورننس آرٹیکٹیکٹ بلیک 3 ، اور الرٹس `SorafsGatewayEvent::ProofHealth`
  جب بھی PDP/POTR کی دہلیز سے تجاوز کیا جاتا ہے (سپلائر ، ونڈو ، ہڑتال/کولڈاؤن کی حیثیت ،
  جرمانے کی رقم)۔ صارفین فراہم کنندہ کے ذریعہ فلٹر کرسکتے ہیں
  نیا ٹیلی میٹری ، بستیوں یا ٹیسٹ کے بغیر پولنگ کے ٹیسٹ کے انتباہات۔

دونوں اختتامی مقامات نئی ونڈو کے ذریعے SoraFS کوٹہ فریم ورک میں حصہ لیتے ہیں
`torii.sorafs.quota.deal_telemetry` ، آپریٹرز کو بھیجنے کی شرح کو ایڈجسٹ کرنے کی اجازت دیتا ہے
فی تعیناتی کی اجازت ہے۔