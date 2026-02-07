---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی ارگین سپلائر اعلانات اور منصوبہ بندی

اس صفحے میں کیننیکل تفصیلات کو دور کیا جاتا ہے
[`docs/source/sorafs/provider_advert_multisource.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)۔
اس دستاویز کو Norito وربیٹیم اسکیمیٹکس اور چینجلوگس کے لئے استعمال کریں۔ پورٹل کاپی
آپریٹرز گائیڈ ، ایس ڈی کے نوٹس اور ٹیلی میٹری حوالہ جات باقی کے قریب رکھتا ہے
SoraFS رن بوکس کی۔

## اسکیمیٹک Norito میں اضافے

### رینج کی صلاحیت (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - ہر درخواست ، `>= 1` ، سب سے بڑا متضاد دور (بائٹس)۔
- `min_granularity` - تلاش کی قرارداد ، `1 <= valor <= max_chunk_span`۔
- `supports_sparse_offsets`- کسی درخواست میں غیر متضاد آفسیٹس کی اجازت دیتا ہے۔
- `requires_alignment` - جب سچ ہے تو ، آفسیٹس کو `min_granularity` کے ساتھ سیدھ میں رکھنا چاہئے۔
- `supports_merkle_proof` - پور ٹوکن سپورٹ کی نشاندہی کرتا ہے۔

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` کیننیکل انکوڈنگ کا اطلاق کریں
تاکہ گپ شپ کے پے لوڈ بقیہ رہیں۔

### `StreamBudgetV1`
- فیلڈز: `max_in_flight` ، `max_bytes_per_sec` ، `burst_bytes` اختیاری۔
- توثیق کے قواعد (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1` ، `max_bytes_per_sec > 0`۔
  - `burst_bytes` ، جب موجود ہو تو ، `> 0` اور `<= max_bytes_per_sec` ہونا چاہئے۔

### `TransportHintV1`
- فیلڈز: `protocol: TransportProtocol` ، `priority: u8` (ونڈو 0-15 کے ذریعہ لاگو ہوتا ہے
  `TransportHintV1::validate`)۔
- جانا جاتا پروٹوکول: `torii_http_range` ، `quic_stream` ، `soranet_relay` ،
  `vendor_reserved`۔
- فراہم کنندہ کے ذریعہ ڈپلیکیٹ پروٹوکول اندراجات کو مسترد کردیا جاتا ہے۔

### `ProviderAdvertBodyV1` میں اضافے
- `stream_budget` اختیاری: `Option<StreamBudgetV1>`۔
- `transport_hints` اختیاری: `Option<Vec<TransportHintV1>>`۔
- دونوں فیلڈز اب `ProviderAdmissionProposalV1` ، گورننس لفافے ،
  سی ایل آئی فکسچر اور ٹیلی میٹرک جےسن۔

## گورننس کے ساتھ توثیق اور ربط

`ProviderAdvertBodyV1::validate` اور `ProviderAdmissionProposalV1::validate`
خراب شدہ میٹا ڈیٹا کو مسترد کریں:

- رینج کی صلاحیتوں کو ضابطہ کشائی کرنا چاہئے اور مدت/گرانولریٹی کی حدود کو پورا کرنا چاہئے۔
- اسٹریم بجٹ / ٹرانسپورٹ کے اشارے میں TLV `CapabilityType::ChunkRangeFetch` کی ضرورت ہوتی ہے
  مماثل اور غیر خالی اشارے کی فہرست۔
- ڈپلیکیٹ ٹرانسپورٹ پروٹوکول اور غلط ترجیحات توثیق کی غلطیاں پیدا کرتی ہیں
  اشتہارات کی گپ شپ سے پہلے۔
- داخلہ لفافے رینج میٹا ڈیٹا کے لئے تجویز/اشتہارات کا موازنہ کرتے ہیں
  `compare_core_fields` تاکہ غلط بیانی شدہ گپ شپ پے لوڈ کو جلد ہی مسترد کردیا جائے۔

رجعت کی کوریج جاری ہے
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`۔

## ٹولنگ اور فکسچر- وینڈر اشتہار پے لوڈ میں میٹا ڈیٹا `range_capability` شامل ہونا ضروری ہے ،
  `stream_budget` اور `transport_hints`۔ `/v1/sorafs/providers` اور کے جوابات کے ذریعے توثیق کریں
  انٹیک فکسچر ؛ JSON ڈائجسٹوں میں پارسڈ صلاحیت شامل کی ضرورت ہے ،
  ٹیلی میٹری انجشن کے لئے اسٹریم بجٹ اور اشارے کی صفیں۔
- Torii اسٹریم بجٹ اور ٹرانسپورٹ کے اشارے کو بے نقاب کرتا ہے
  آپ کے JSON نمونے تاکہ ڈیش بورڈز اس خصوصیت کو اپنانے کی پیروی کریں۔
- `fixtures/sorafs_manifest/provider_admission/` کے تحت فکسچر میں اب شامل ہیں:
  - کیننیکل ملٹی ارگین اشتہارات ،
  - ڈاون گریڈ ٹیسٹنگ کے ل an ایک متبادل غیر منقطع شکل ، اور
  - `multi_fetch_plan.json` SDK سویٹس کے لئے بازیافت کا منصوبہ کھیلنے کے لئے
    ڈٹرمینسٹک ملٹی پیئر۔

## آرکسٹریٹر اور Torii کے ساتھ انضمام

- Torii `/v1/sorafs/providers` پارسڈ رینج کی صلاحیت میٹا ڈیٹا کے ساتھ ساتھ
  `stream_budget` اور `transport_hints`۔ ڈاون گریڈ انتباہات جب متحرک ہوتے ہیں
  فراہم کنندگان نئے میٹا ڈیٹا کو نظرانداز کرتے ہیں ، اور گیٹ وے رینج کے اختتامی مقامات کا اطلاق ہوتا ہے
  براہ راست مؤکلوں کے لئے ایک ہی پابندیاں۔
- ملٹی ارگین آرکیسٹریٹر (`sorafs_car::multi_fetch`) اب حدود کو نافذ کرتا ہے
  کام تفویض کرتے وقت درجہ بندی ، صلاحیتوں اور اسٹریم بجٹ کی صف بندی۔ یونٹ ٹیسٹ
  وہ بڑے حص ch ے ، ویرل تلاش اور تھروٹلنگ منظرناموں کا احاطہ کرتے ہیں۔
- `sorafs_car::multi_fetch` ڈاؤن گریڈ سگنلز کا اخراج کرتا ہے (سیدھ میں غلطیاں ،
  آپریٹرز کو یہ معلوم کرنے کے لئے کہ انہیں کیوں چھوڑ دیا گیا
  منصوبہ بندی کے دوران مخصوص سپلائرز۔

## ٹیلی میٹری کا حوالہ

Torii رینج بازیافت آلہ کار Grafana ڈیش بورڈ
** SoraFS مشاہدہ کی بازیافت ** (`dashboards/grafana/sorafs_fetch_observability.json`) اور
ایسوسی ایٹڈ الرٹ قواعد (`dashboards/alerts/sorafs_fetch_rules.yml`)۔

| میٹرک | قسم | ٹیگز | تفصیل |
| --------- | ------ | ----------- | ------------- |
| `torii_sorafs_provider_range_capability_total` | گیج | `feature` (`providers` ، `supports_sparse_offsets` ، `requires_alignment` ، `supports_merkle_proof` ، `stream_budget` ، `transport_hints`) | سپلائرز ایڈورٹائزنگ رینج کی صلاحیت کی خصوصیات۔ |
| `torii_sorafs_range_fetch_throttle_events_total` | کاؤنٹر | `reason` (`quota` ، `concurrency` ، `byte_rate`) | پالیسی کے ذریعہ تھروٹلنگ کے ساتھ رینج کی کوششیں۔ |
| `torii_sorafs_range_fetch_concurrency_current` | گیج | - | محفوظ فعال اسٹریمز جو مشترکہ ہم آہنگی کا بجٹ استعمال کرتے ہیں۔ |

پروم کیو ایل کی مثالیں:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

چالو کرنے سے پہلے استعمال کرنے کی مشکلات کی تصدیق کے لئے تھروٹلنگ کاؤنٹر کا استعمال کریں
جب ہم آہنگی کم ہوتی ہے تو ملٹی ارگین آرکسٹریٹر ڈیفالٹ اور الرٹس
اپنے بیڑے کے اسٹریم بجٹ کے زیادہ سے زیادہ قریب جائیں۔