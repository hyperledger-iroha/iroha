---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈیل انجن
عنوان: SoraFS معاہدہ انجن
سائڈبار_لیبل: معاہدہ انجن
تفصیل: SF-8 معاہدے کے انجن کا جائزہ ، Torii اور ٹیلی میٹری سطحوں کے ساتھ انضمام۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/deal_engine.md` کی آئینہ دار ہے۔ دونوں مقامات کو سیدھے رکھیں جبکہ متبادل دستاویزات فعال رہیں۔
:::

# SoraFS معاہدہ انجن

SF-8 روڈ میپ ٹریک SoraFS معاہدہ انجن متعارف کراتا ہے ، فراہم کرتا ہے
اسٹوریج اور بازیافت کے معاہدوں کے لئے ڈٹرمینسٹک اکاؤنٹنگ
صارفین اور فراہم کنندہ۔ معاہدوں کو پے لوڈ Norito کے ساتھ بیان کیا گیا ہے
`crates/sorafs_manifest/src/deal.rs` میں بیان کردہ ، معاہدے کی شرائط کا احاطہ کرتے ہوئے ،
بانڈ مسدود کرنا ، امکانی مائکروپیمنٹس اور تصفیہ ریکارڈ۔

SoraFS (`sorafs_node::NodeHandle`) بلٹ ان ورکر اب انسٹیٹیٹ کرتا ہے
`DealEngine` ہر نوڈ عمل کے لئے۔ انجن:

- `DealTermsV1` کا استعمال کرتے ہوئے معاہدوں کی توثیق اور ریکارڈ کرتا ہے۔
- جب نقل کے استعمال کی اطلاع دی جاتی ہے تو XOR میں متضاد الزامات جمع کرتا ہے۔
- تعی .ن کے نمونے لینے کا استعمال کرتے ہوئے امکانی مائکروپیمنٹ ونڈوز کا اندازہ کرتا ہے
  بلیک 3 پر مبنی ؛ اور
- اشاعت کے ل suitable موزوں لیجر اسنیپ شاٹس اور تصفیہ پے لوڈ تیار کرتا ہے
  گورننس کی

یونٹ ٹیسٹ کور کی توثیق ، ​​مائکروپیمنٹ سلیکشن اور تصفیہ بہاؤ کے لئے بہاؤ
لہذا آپریٹر اعتماد کے ساتھ APIs کا استعمال کرسکتے ہیں۔ اب لیکویڈیشن جاری کرتے ہیں
`DealSettlementV1` گورننس پے لوڈ ، براہ راست سے منسلک ہوتا ہے
ایس ایف -12 اشاعت ، اور اوپن لیمٹری سیریز `sorafs.node.deal_*` کو اپ ڈیٹ کریں
(`deal_settlements_total` ، `deal_expected_charge_nano` ، `deal_client_debit_nano` ،
`deal_outstanding_nano` ، `deal_bond_slash_nano` ، `deal_publish_total`) Torii کے لئے اور
SLOS کی درخواست. مندرجہ ذیل آئٹمز آٹومیشن کو ختم کرنے پر مرکوز ہیں
آڈیٹرز اور گورننس پالیسی کے ساتھ منسوخی کے الفاظ کو مربوط کرنے میں۔

استعمال ٹیلی میٹری اب `sorafs.node.micropayment_*` میٹرک سیٹ میں بھی کھلاتی ہے:
`micropayment_charge_nano` ، `micropayment_credit_generated_nano` ،
`micropayment_credit_applied_nano` ، `micropayment_credit_carry_nano` ،
`micropayment_outstanding_nano` ، اور ٹکٹ کاؤنٹر
(`micropayment_tickets_processed_total` ، `micropayment_tickets_won_total` ،
`micropayment_tickets_duplicate_total`)۔ یہ کل لاٹری کے بہاؤ کو بے نقاب کرتے ہیں
امکانی طور پر تاکہ آپریٹرز مائکروپیمنٹ کی کمائی سے باہمی تعلق کرسکیں اور
تصفیہ کے نتائج کے ساتھ کریڈٹ کیری اوور۔

Torii کے ساتھ ## انضمام

Torii فراہم کرنے والوں کے لئے استعمال اور طرز عمل کی اطلاع دینے کے لئے وقف شدہ اختتامی نکات کو بے نقاب کرتا ہے
درزی ساختہ وائرنگ لیس معاہدہ لائف سائیکل:- `POST /v1/sorafs/deal/usage` `DealUsageReport` ٹیلی میٹری کو قبول کرتا ہے اور واپسی کرتا ہے
  ڈٹرمینسٹک اکاؤنٹنگ کے نتائج (`UsageOutcome`)۔
- `POST /v1/sorafs/deal/settle` موجودہ ونڈو کو ختم کرتا ہے ، منتقل کرتے ہوئے
  اس کے نتیجے میں `DealSettlementRecord` BASE64 میں `DealSettlementV1` کے ساتھ مل کر
  گورننس ڈی اے جی میں اشاعت کے لئے تیار ہے۔
- `/v1/events/sse` فیڈ Torii سے اب `SorafsGatewayEvent::DealUsage` ریکارڈز منتقل کرتا ہے
  ہر استعمال کو پیش کرنے کا خلاصہ (ایپچ ، ماپا گیب گھنٹے ، ٹکٹ کاؤنٹرز ،
  تعصب کے الزامات) ، ریکارڈ `SorafsGatewayEvent::DealSettlement`
  جس میں تصفیہ لیجر پلس ڈائجسٹ/سائز/بیس 64 کی کیننیکل اسنیپ شاٹ شامل ہے
  ڈسک پر گورننس آرٹیکٹیکٹ بلیک 3 ، اور `SorafsGatewayEvent::ProofHealth` الرٹس
  جب بھی PDP/POTR کی دہلیز سے تجاوز کیا جاتا ہے (فراہم کنندہ ، ونڈو ، ہڑتال/کولڈاؤن کی حیثیت ،
  جرمانے کی قیمت)۔ صارفین فراہم کنندہ کے ذریعہ فلٹر کرسکتے ہیں تاکہ نئے پر رد عمل ظاہر کیا جاسکے
  بغیر کسی پولنگ کے ٹیلی میٹری ، بستیوں یا پروف ہیلتھ الرٹس۔

دونوں اختتامی مقامات نئی ونڈو کے ذریعے SoraFS کوٹہ فریم ورک میں حصہ لیتے ہیں
`torii.sorafs.quota.deal_telemetry` ، آپریٹرز کو بھیجنے کی شرح کو ایڈجسٹ کرنے کی اجازت دیتا ہے
تعیناتی کے ذریعہ اجازت