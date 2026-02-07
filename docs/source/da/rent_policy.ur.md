---
lang: ur
direction: rtl
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T15:38:30.661606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ڈیٹا کی دستیابی کرایہ اور ترغیبی پالیسی (DA-7)

_ اسٹیٹس: ڈرافٹنگ - مالکان: اکنامکس ڈبلیو جی / ٹریژری / اسٹوریج ٹیم_

روڈ میپ آئٹم ** ڈی اے 7 ** ہر بلاب کے لئے ایک واضح XOR-SENDMININTED کرایہ متعارف کراتا ہے
`/v1/da/ingest` میں پیش کیا گیا ، اس کے علاوہ بونس جو PDP/POTR عملدرآمد کو انعام دیتے ہیں اور
ایگریس نے مؤکلوں کو بازیافت کرنے میں مدد کی۔ اس دستاویز میں ابتدائی پیرامیٹرز کی وضاحت کی گئی ہے ،
ان کی ڈیٹا ماڈل کی نمائندگی ، اور Torii کے ذریعہ استعمال شدہ حساب کتاب کا فلو ،
ایس ڈی کے ، اور ٹریژری ڈیش بورڈز۔

## پالیسی کا ڈھانچہ

پالیسی کو [`DaRentPolicyV1`] (/crates/iroha_data_model/src/da/types.rs) کے طور پر انکوڈ کیا گیا ہے
ڈیٹا ماڈل کے اندر۔ Torii اور گورننس ٹولنگ پالیسی کو برقرار رکھتے ہیں
Norito پے لوڈز تاکہ کرایہ کے حوالے اور مراعات یافتہ لیجرز کو دوبارہ تشکیل دیا جاسکے
عزم کے مطابق اسکیما نے پانچ نوبس کو بے نقاب کیا:

| فیلڈ | تفصیل | ڈیفالٹ |
| ------- | ------------- | --------- |
| `base_rate_per_gib_month` | XOR نے برقرار رکھنے کے ہر مہینے میں فی گیب وصول کیا۔ | `250_000` مائکرو زور (0.25 XOR) |
| `protocol_reserve_bps` | پروٹوکول ریزرو (بیس پوائنٹس) پر جانے والے کرایہ کا حصہ۔ | `2_000` (20 ٪) |
| `pdp_bonus_bps` | بونس فیصد فی کامیاب PDP تشخیص۔ | `500` (5 ٪) |
| `potr_bonus_bps` | بونس فیصد فی کامیاب پوٹر تشخیص۔ | `250` (2.5 ٪) |
| `egress_credit_per_gib` | جب کوئی فراہم کنندہ ڈی اے ڈیٹا کی 1GIB پیش کرتا ہے تو کریڈٹ ادا کیا جاتا ہے۔ | `1_500` مائکرو XOR |

تمام بنیادی نقطہ اقدار کو `BASIS_POINTS_PER_UNIT` (10000) کے خلاف توثیق کیا گیا ہے۔
پالیسی کی تازہ کاریوں کو گورننس کے ذریعے سفر کرنا چاہئے ، اور ہر Torii نوڈ کو بے نقاب کیا جاتا ہے
`torii.da_ingest.rent_policy` کنفیگریشن سیکشن کے ذریعے فعال پالیسی
(`iroha_config`)۔ آپریٹرز `config.toml` میں ڈیفالٹس کو اوور رائڈ کرسکتے ہیں:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

سی ایل آئی ٹولنگ (`iroha app da rent-quote`) وہی Norito/JSON پالیسی آدانوں کو قبول کرتا ہے
اور ایسے نوادرات کا اخراج کرتا ہے جو فعال `DaRentPolicyV1` کو بغیر پہنچے بغیر آئینہ دار ہیں
واپس Torii ریاست میں۔ پالیسی اسنیپ شاٹ کو کسی انجسٹ رن کے لئے استعمال کیا جاتا ہے لہذا
اقتباس تولیدی ہے۔

### کرایہ کی قیمت کو برقرار رکھنا

`iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` کو چلائیں
آن اسکرین سمری اور ایک خوبصورت طباعت شدہ JSON آرٹ فیکٹ دونوں کا اخراج کریں۔ فائل
ریکارڈ `policy_source` ، inlined `DaRentPolicyV1` اسنیپ شاٹ ، کمپیوٹڈ
`DaRentQuote` ، اور ایک اخذ کردہ `ledger_projection` (سیریلائزڈ کے ذریعے
۔
پائپ لائنز جب `--quote-out` ایک نیسٹڈ ڈائرکٹری میں پوائنٹس کرتا ہے تو CLI کوئی بھی تخلیق کرتا ہے
لاپتہ والدین ، ​​لہذا آپریٹر مقامات کو معیاری بناسکتے ہیں جیسے
`artifacts/da/rent_quotes/<timestamp>.json` دوسرے DA ثبوت کے بنڈل کے ساتھ۔
نوادرات کو کرایہ کی منظوری یا مفاہمت کے لئے منسلک کریں لہذا XOR
خرابی (بیس کرایہ ، ریزرو ، PDP/POTR بونس ، اور ایگریس کریڈٹ) ہے
تولیدی خود بخود اوور رائڈ کرنے کے لئے `--policy-label "<text>"` پاس کریں
اخذ کردہ `policy_source` تفصیل (فائل کے راستے ، سرایت شدہ ڈیفالٹ ، وغیرہ)
انسانی پڑھنے کے قابل ٹیگ جیسے گورننس ٹکٹ یا ظاہر ہیش۔ CLI trims
یہ قدر اور خالی/وائٹ اسپیس صرف ڈوروں کو مسترد کرتی ہے لہذا ریکارڈ شدہ ثبوت
قابل اظہار رہتا ہے۔

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```لیجر پروجیکشن سیکشن براہ راست ڈی اے کرایہ لیجر داعش میں کھلاتا ہے: یہ
پروٹوکول ریزرو ، فراہم کنندہ کی ادائیگی ، اور کے لئے مقصود ژور ڈیلٹا کی وضاحت کرتا ہے
فی پروف بونس پولز بغیر کسی بیسپوک آرکیسٹریشن کوڈ کی ضرورت کے۔

### کرایہ لیجر کے منصوبے تیار کرنا

`iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora` چلائیں
مستقل کرایہ کی قیمت کو قابل عمل لیجر ٹرانسفر میں تبدیل کرنے کے لئے۔ حکم
پارسیس ایمبیڈڈ `ledger_projection` ، Norito `Transfer` ہدایات کا اخراج کرتا ہے
جو ٹریژری میں بیس کرایہ جمع کرتے ہیں ، ریزرو/فراہم کنندہ کو راستہ دیتے ہیں
حصے ، اور PDP/POTR بونس پول کو براہ راست ادائیگی کنندہ سے فنڈز فراہم کرتے ہیں۔
آؤٹ پٹ JSON حوالہ میٹا ڈیٹا کو آئینہ دار کرتا ہے لہذا CI اور ٹریژری ٹولنگ کی وجہ ہوسکتی ہے
اسی نوادرات کے بارے میں:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

حتمی `egress_credit_per_gib_micro_xor` فیلڈ ڈیش بورڈز اور ادائیگی کی اجازت دیتا ہے
نظام الاوقات کرایہ کی پالیسی کے ساتھ ایگریس معاوضوں کو سیدھ میں کرتے ہیں جس نے اس کو تیار کیا ہے
اسکرپٹنگ گلو میں پالیسی ریاضی کی بحالی کے بغیر حوالہ۔

## مثال کے اقتباس

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

اقتباس Torii نوڈس ، SDKs ، اور ٹریژری رپورٹس میں تولیدی ہے کیونکہ
اس میں ایڈہاک ریاضی کے بجائے ڈٹرمینسٹک Norito ڈھانچے کا استعمال کیا گیا ہے۔ آپریٹرز کر سکتے ہیں
JSON/CBOR انکوڈڈ `DaRentPolicyV1` کو گورننس کی تجاویز یا کرایہ پر منسلک کریں
کسی بھی بلاب کے لئے کون سے پیرامیٹرز نافذ تھے یہ ثابت کرنے کے لئے آڈٹ۔

## بونس اور ذخائر

- ** پروٹوکول ریزرو: ** `protocol_reserve_bps` XOR ریزرو کی پشت پناہی کرتا ہے
  ایمرجنسی ریپلیکشن اور کم رقم کی واپسی۔ ٹریژری اس بالٹی کو ٹریک کرتا ہے
  الگ الگ یقینی بنانے کے لئے لیجر بیلنس تشکیل شدہ شرح سے میل کھاتا ہے۔
- ** PDP/POTR بونس: ** ہر کامیاب ثبوت کی تشخیص ایک اضافی وصول کرتی ہے
  ادائیگی `base_rent × bonus_bps` سے اخذ کی گئی ہے۔ جب ڈی اے شیڈیولر ثبوت خارج کرتا ہے
  رسیدیں اس میں بنیادی نکاتی ٹیگز شامل ہیں تاکہ مراعات کو دوبارہ چلایا جاسکے۔
- ** ایگریس کریڈٹ: ** فراہم کنندگان ریکارڈ گیب فی منشور پیش کیا گیا ،
  `egress_credit_per_gib` ، اور `iroha app da prove-availability` کے ذریعے رسیدیں جمع کروائیں۔
  کرایہ کی پالیسی فی گب کی رقم کو گورننس کے ساتھ ہم آہنگی میں رکھتی ہے۔

## آپریشنل فلو

1. ** ingest: ** `/v1/da/ingest` فعال `DaRentPolicyV1` ، کوٹس کرایہ پر لوڈ کرتا ہے
   بلاب سائز اور برقرار رکھنے کی بنیاد پر ، اور Norito میں اقتباس کو سرایت کرتا ہے
   منشور جمع کرانے والا ایک بیان پر دستخط کرتا ہے جو کرایہ ہیش اور حوالہ دیتا ہے
   اسٹوریج ٹکٹ ID۔
2
   `DaRentPolicyV1::quote` ، اور کرایہ لیجر (بیس کرایہ ، ریزرو ،
   بونس ، اور متوقع ایگریس کریڈٹ)۔ ریکارڈ شدہ کرایہ کے مابین کوئی تضاد
   اور دوبارہ گنتی قیمتیں CI میں ناکام ہوجاتی ہیں۔
3. ** پروف انعامات: ** جب PDP/POTR شیڈولرز کامیابی کے نشان لگاتے ہیں تو وہ رسید کا اخراج کرتے ہیں
   منشور ڈائجسٹ ، پروف قسم ، اور XOR بونس پر مشتمل ہے
   پالیسی گورننس اسی اقتباس کی بحالی کے ذریعہ ادائیگیوں کا آڈٹ کرسکتی ہے۔
4.
   Torii GIB کی گنتی کو `egress_credit_per_gib` کے ذریعہ ضرب دیتا ہے اور ادائیگی جاری کرتا ہے
   کرایہ یسکرو کے خلاف ہدایات۔

## ٹیلی میٹریTorii نوڈس مندرجہ ذیل Prometheus میٹرکس کے ذریعے کرایہ کے استعمال کو بے نقاب کرتے ہیں (لیبل:
`cluster` ، `storage_class`):

- `torii_da_rent_gib_months_total`- Gib-months کے ذریعہ `/v1/da/ingest` کے ذریعہ حوالہ دیا گیا ہے۔
- `torii_da_rent_base_micro_total` - بیس کرایہ (مائیکرو XOR) انجسٹ میں جمع ہوا۔
- `torii_da_protocol_reserve_micro_total` - پروٹوکول ریزرو شراکت۔
- `torii_da_provider_reward_micro_total`- فراہم کنندہ سائیڈ کرایہ کی ادائیگی۔
- `torii_da_pdp_bonus_micro_total` اور `torii_da_potr_bonus_micro_total` -
  PDP/POTR بونس تالاب Ingest اقتباس سے حاصل کیے گئے ہیں۔

اکنامکس ڈیش بورڈز ان کاؤنٹرز پر انحصار کرتے ہیں تاکہ لیجر آئی ایس آئی ایس ، ریزرو نلکوں کو یقینی بنایا جاسکے ،
اور PDP/POTR بونس کے نظام الاوقات ہر ایک کے لئے پالیسی پیرامیٹرز سے ملتے ہیں
کلسٹر اور اسٹوریج کلاس۔ SoraFS صلاحیت صحت Grafana بورڈ
(`dashboards/grafana/sorafs_capacity_health.json`) اب سرشار پینل پیش کرتا ہے
کرایہ کی تقسیم کے لئے ، PDP/POTR بونس وصول ، اور گیب ماہ کی گرفتاری ، اجازت دیتے ہیں
Torii کلسٹر یا اسٹوریج کلاس کے ذریعہ فلٹر کرنے کے لئے خزانے
حجم اور ادائیگی

## اگلے اقدامات

- ✅ `/v1/da/ingest` رسیدیں اب `rent_quote` کو سرایت کرتی ہیں اور CLI/SDK سطحوں کو حوالہ دیا گیا ہے
  بیس کرایہ ، ریزرو شیئر ، اور PDP/POTR بونس تاکہ جمع کرانے والے اس سے پہلے XOR کی ذمہ داریوں کا جائزہ لے سکتے ہیں
  پے لوڈ کا ارتکاب کرنا۔
- آئندہ ڈی اے ساکھ/آرڈر بک فیڈز کے ساتھ کرایہ لیجر کو مربوط کریں
  یہ ثابت کرنے کے لئے کہ اعلی دستیابی فراہم کرنے والوں کو صحیح ادائیگی مل رہی ہے۔