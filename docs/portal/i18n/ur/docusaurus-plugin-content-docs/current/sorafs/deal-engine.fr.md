---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈیل انجن
عنوان: راگ انجن SoraFS
سائڈبار_لیبل: راگ انجن
تفصیل: SF-8 ٹیوننگ انجن کا جائزہ ، Torii انضمام اور ٹیلی میٹری سطحوں۔
---

::: نوٹ کینونیکل ماخذ
:::

# راگ انجن SoraFS

SF-8 روڈ میپ ٹریک SoraFS Chord انجن کو متعارف کراتا ہے ، فراہم کرتا ہے
اسٹوریج اور بازیافت کے معاہدوں کے لئے ڈٹرمینسٹک اکاؤنٹنگ
صارفین اور سپلائرز۔ معاہدوں کو پے لوڈ Norito کے ذریعے بیان کیا گیا ہے
`crates/sorafs_manifest/src/deal.rs` میں بیان کردہ ، معاہدے کی شرائط کو پورا کرتے ہوئے ،
ہاپ لاکنگ ، امکانی مائکروپیمنٹس اور تصفیہ ریکارڈ۔

ایمبیڈڈ ورکر SoraFS (`sorafs_node::NodeHandle`) اب انسٹینیٹ کرتا ہے
ہر نوڈ عمل کے لئے `DealEngine`۔ انجن:

- `DealTermsV1` کے ذریعے معاہدوں کی توثیق اور ریکارڈ کرتا ہے۔
- جب نقل کے استعمال کی اطلاع دی جاتی ہے تو XOR میں متضاد چارجز جمع کرتا ہے۔
- تعی .ن کے نمونے لینے کے ذریعہ امکانی مائکروپیمنٹ ونڈوز کا اندازہ کرتا ہے
  بلیک 3 پر مبنی ؛ اور
- اشاعت کے ل suitable موزوں لیجر اسنیپ شاٹس اور تصفیہ پے لوڈ تیار کرتا ہے
  گورننس کی

یونٹ ٹیسٹنگ میں توثیق ، ​​مائکروپیمنٹ سلیکشن اور ادائیگی کے بہاؤ کا احاطہ کیا گیا ہے۔
ضابطہ تاکہ آپریٹر اعتماد کے ساتھ APIs کا استعمال کرسکیں۔ ضوابط
اب گورننس پے لوڈ جاری کریں `DealSettlementV1` براہ راست مربوط کریں
SF-12 ریلیز پائپ لائن پر ، اور اوپن لیمٹری سیریز `sorafs.node.deal_*` کو اپ ڈیٹ کریں
(`deal_settlements_total` ، `deal_expected_charge_nano` ، `deal_client_debit_nano` ،
`deal_outstanding_nano` ، `deal_bond_slash_nano` ، `deal_publish_total`) ڈیش بورڈز Torii کے لئے اور
ایس ایل او ایس کی درخواست۔ مندرجہ ذیل کام شروع کردہ سلیشنگ کے آٹومیشن کو نشانہ بناتا ہے
گورننس پالیسی کے ساتھ آڈیٹرز اور منسوخی کے الفاظ کا ہم آہنگی۔

استعمال ٹیلی میٹری میں میٹرکس `sorafs.node.micropayment_*` کا سیٹ بھی کھلایا جاتا ہے:
`micropayment_charge_nano` ، `micropayment_credit_generated_nano` ،
`micropayment_credit_applied_nano` ، `micropayment_credit_carry_nano` ،
`micropayment_outstanding_nano` ، اور ٹکٹ کاؤنٹر
(`micropayment_tickets_processed_total` ، `micropayment_tickets_won_total` ،
`micropayment_tickets_duplicate_total`)۔ یہ کل لاٹری کے بہاؤ کو بے نقاب کرتے ہیں
امکانی طور پر تاکہ آپریٹرز مائکروپیمنٹ کی کمائی سے باہمی تعلق کرسکیں اور
تصفیہ کے نتائج کے ساتھ کریڈٹ کیری اوور۔

## انضمام Torii

Torii استعمال اور ڈرائیو کی اطلاع دینے کے لئے فراہم کنندگان کے لئے وقف شدہ اختتامی نکات کو بے نقاب کرتا ہے
مخصوص وائرنگ کے بغیر معاہدوں کی زندگی کا چکر:- `POST /v1/sorafs/deal/usage` `DealUsageReport` ٹیلی میٹری کو قبول کرتا ہے اور واپسی کرتا ہے
  ڈٹرمینسٹک اکاؤنٹنگ کے نتائج (`UsageOutcome`)۔
- `POST /v1/sorafs/deal/settle` موجودہ ونڈو کو حتمی شکل دیتا ہے ، اور اسٹریم کرتا ہے
  `DealSettlementRecord` کے نتیجے میں BASE64 انکوڈڈ `DealSettlementV1` ہے
  گورننس ڈی اے جی میں اشاعت کے لئے تیار ہے۔
- Torii کا فیڈ `/v1/events/sse` اب ریکارڈنگ کو نشر کرتا ہے
  `SorafsGatewayEvent::DealUsage` ہر استعمال کو جمع کرانے کا خلاصہ (عہد ، GIB گھنٹے کی پیمائش ،
  ٹکٹ کاؤنٹرز ، ڈٹرمینسٹک بوجھ) ، ریکارڈ
  `SorafsGatewayEvent::DealSettlement` جس میں تصفیہ لیجر کا کیننیکل اسنیپ شاٹ شامل ہے
  اس کے ساتھ ساتھ آن ڈسک گورننس آرٹیکٹیکٹ کے بلیک 3 ڈائجسٹ/سائز/بیس 64 ، اور انتباہات
  `SorafsGatewayEvent::ProofHealth` جیسے ہی PDP/POTR کی حد سے تجاوز کیا جاتا ہے (فراہم کنندہ ، ونڈو ،
  ہڑتال/کوولڈاؤن کی حیثیت ، جرمانے کی رقم)۔ صارفین فراہم کنندہ کے ذریعہ فلٹر کرسکتے ہیں
  پولنگ فری ثبوتوں سے نئے ٹیلی میٹری ، قواعد و ضوابط یا صحت کے انتباہات پر رد عمل ظاہر کرنا۔

دونوں اختتامی مقامات نئی ونڈو کے ذریعے کوٹہ فریم ورک SoraFS میں حصہ لیتے ہیں
`torii.sorafs.quota.deal_telemetry` ، آپریٹرز کو جمع کرانے کی شرح کو ایڈجسٹ کرنے کی اجازت دیتا ہے
تعیناتی کے ذریعہ اجازت دی گئی۔