---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈیل انجن
عنوان: ٹرانزیکشن انجن SoraFS
سائڈبار_لیبل: ڈیل انجن
تفصیل: SF-8 ڈیل انجن کا جائزہ ، Torii انضمام اور ٹیلی میٹری سطحوں۔
---

::: نوٹ کینونیکل ماخذ
:::

# ٹرانزیکشن انجن SoraFS

روڈ میپ SF-8 SoraFS ٹرانزیکشن انجن کو متعارف کراتا ہے ، فراہم کرتا ہے
اسٹوریج اور بازیافت کے معاہدوں کا تعی .ن اکاؤنٹنگ
کلائنٹ اور فراہم کرنے والے۔ معاہدوں کو Norito پے لوڈ کے ذریعہ بیان کیا گیا ہے ،
`crates/sorafs_manifest/src/deal.rs` میں بیان کردہ ، لین دین کی شرائط کو پورا کرتے ہوئے ،
بانڈز ، امکانی مائکروپیمنٹس اور تصفیہ کے ریکارڈ کو مسدود کرنا۔

بلٹ میں SoraFS ورکر (`sorafs_node::NodeHandle`) اب تخلیق کرتا ہے
ہر نوڈ عمل کے لئے `DealEngine` کی ایک مثال۔ انجن:

- `DealTermsV1` کے ذریعے لین دین کی توثیق اور رجسٹر کرتا ہے۔
- نقل کے استعمال کے بارے میں رپورٹنگ کرتے وقت XOR میں چارجز چارجز ؛
- احتیاطی مائکروپیمنٹس کی کھڑکیوں کا تخمینہ استعمال کرتے ہوئے
  بلیک 3 پر مبنی نمونے لینے ؛ اور
- گورننس میں اشاعت کے ل suitable موزوں سنیپ شاٹس لیجر اور حساب کتاب کے پے لوڈ تیار کرتا ہے۔

اس بات کو یقینی بنانے کے لئے یونٹ ٹیسٹ کی توثیق ، ​​مائکروپیمنٹ سلیکشن اور تصفیہ کے بہاؤ کا احاطہ کرتا ہے
آپریٹرز اعتماد کے ساتھ APIs کی جانچ کرسکتے ہیں۔ حساب کتاب اب گورننس پے لوڈ جاری کرتے ہیں
`DealSettlementV1` ، براہ راست SF-12 اشاعت پائپ لائن سے منسلک ہوتا ہے ، اور سیریز کو اپ ڈیٹ کرتا ہے
اوپن لیمیٹری `sorafs.node.deal_*`
(`deal_settlements_total` ، `deal_expected_charge_nano` ، `deal_client_debit_nano` ،
`deal_outstanding_nano` ، `deal_bond_slash_nano` ، `deal_publish_total`) ڈیش بورڈز Torii کے لئے اور
SLO نفاذ. مندرجہ ذیل اقدامات شروع کردہ سلیشنگ کو خودکار کرنے پر مرکوز ہیں
آڈیٹرز ، اور گورننس پالیسیوں کے ساتھ منسوخی کے الفاظ کی صف بندی کرنا۔

استعمال ٹیلی میٹری `sorafs.node.micropayment_*` میٹرکس سیٹ کو بھی کھلاتا ہے:
`micropayment_charge_nano` ، `micropayment_credit_generated_nano` ،
`micropayment_credit_applied_nano` ، `micropayment_credit_carry_nano` ،
`micropayment_outstanding_nano` ، نیز ٹکٹ کاؤنٹرز
(`micropayment_tickets_processed_total` ، `micropayment_tickets_won_total` ،
`micropayment_tickets_duplicate_total`)۔ یہ مجموعی طور پر احتمال کو ظاہر کرتے ہیں
لاٹری اسٹریم تاکہ آپریٹرز مائکروپیمنٹ جیت اور
تصفیہ کے نتائج کے ساتھ قرضوں کی منتقلی۔

## انضمام Torii

Torii سرشار اختتامی نکات فراہم کرتا ہے تاکہ فراہم کنندہ استعمال اور طرز عمل بھیج سکیں
خصوصی وائرنگ کے بغیر لین دین کا زندگی کا چکر:- `POST /v1/sorafs/deal/usage` `DealUsageReport` ٹیلی میٹری اور ریٹرن وصول کرتا ہے
  ڈٹرمینسٹک اکاؤنٹنگ کے نتائج (`UsageOutcome`)۔
- `POST /v1/sorafs/deal/settle` موجودہ ونڈو کو ختم کرتا ہے ، اسٹریمنگ
  حتمی `DealSettlementRecord` کے ساتھ ساتھ Base64-encoded `DealSettlementV1` ،
  گورننس ڈی اے جی میں اشاعت کے لئے تیار ہیں۔
- ٹیپ Torii `/v1/events/sse` اب ریکارڈز `SorafsGatewayEvent::DealUsage` کو نشر کرتا ہے ،
  ہر بھیجنے والے استعمال کا خلاصہ (عہد ، پیمائش شدہ گیب گھنٹے ، ٹکٹ کاؤنٹرز ،
  تعصب کے الزامات) ، اندراجات `SorafsGatewayEvent::DealSettlement` ،
  بشمول کیننیکل اسنیپ شاٹ لیجر کے حساب کتاب کے علاوہ بلیک 3 ڈائجسٹ/سائز/بیس 64
  ڈسک پر آرٹیکٹیکٹ کی حکمرانی ، اور جب سے تجاوز کر کے `SorafsGatewayEvent::ProofHealth` کو الرٹ کرتا ہے
  PDP/POTR دہلیز (فراہم کنندہ ، ونڈو ، ہڑتال/کوولڈاؤن ریاست ، ٹھیک رقم)۔ صارفین کر سکتے ہیں
  بغیر کسی پولنگ کے نئے ٹیلی میٹری ، حساب کتاب یا پروف ہیلتھ الرٹس کا جواب دینے کے لئے فراہم کنندہ کے ذریعہ فلٹر کریں۔

دونوں اختتامی مقامات SoraFS کوٹہ فریم ورک میں ایک نئی ونڈو کے ذریعے حصہ لیتے ہیں
`torii.sorafs.quota.deal_telemetry` ، آپریٹرز کو قابل قبول تشکیل دینے کی اجازت دیتا ہے
ہر تعیناتی کے لئے بھیجنے کی تعدد۔