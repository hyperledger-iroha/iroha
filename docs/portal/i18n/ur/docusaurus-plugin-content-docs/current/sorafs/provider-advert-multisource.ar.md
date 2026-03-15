---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس فراہم کنندہ اشتہارات اور شیڈولنگ

اس صفحے میں معیار کا خلاصہ کیا گیا ہے
[`docs/source/sorafs/provider_advert_multisource.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)۔
اس دستاویز کو Norito اسکیما وربیٹیم کے لئے استعمال کریں اور لاگ ان کو تبدیل کریں۔ پورٹل ورژن آپریٹرز کی ہدایات رکھتا ہے
ایس ڈی کے نوٹ اور ٹیلی میٹری حوالہ جات باقی SoraFS آپریٹنگ دستورالعمل کے قریب ہیں۔

## Norito اسکیم کے اضافے

### رینج کی گنجائش (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - فی درخواست سب سے بڑی منسلک رینج (بائٹس) ، `>= 1`۔
- `min_granularity` - تلاش کی درستگی ، `1 <= القيمة <= max_chunk_span`۔
- `supports_sparse_offsets` - ایک ہی درخواست میں منقطع آفسیٹس کی اجازت دیتا ہے۔
- `requires_alignment` - جب فعال ہوجائے تو ، آفسیٹس کو `min_granularity` کے ساتھ لائن لگانا چاہئے۔
- `supports_merkle_proof` - POR گواہ کی حمایت کی نشاندہی کرتا ہے۔

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` قانونی انکوڈنگ کو نافذ کریں
تاکہ گپ شپ پے لوڈ ناگزیر رہیں۔

### `StreamBudgetV1`
- فیلڈز: `max_in_flight` ، `max_bytes_per_sec` ، اور `burst_bytes` اختیاری ہیں۔
- توثیق کے قواعد (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1` ، `max_bytes_per_sec > 0`۔
  - `burst_bytes` جب موجود ہو تو `> 0` اور `<= max_bytes_per_sec` ہونا چاہئے۔

### `TransportHintV1`
- فیلڈز: `protocol: TransportProtocol` ، `priority: u8` (ونڈو 0-15 نافذ ہے
  `TransportHintV1::validate`)۔
- جانا جاتا پروٹوکول: `torii_http_range` ، `quic_stream` ، `soranet_relay` ،
  `vendor_reserved`۔
- ہر فراہم کنندہ کے لئے ڈپلیکیٹ پروٹوکول اندراجات مسترد کردیئے جاتے ہیں۔

### اضافے `ProviderAdvertBodyV1`
- `stream_budget` اختیاری: `Option<StreamBudgetV1>`۔
- `transport_hints` اختیاری: `Option<Vec<TransportHintV1>>`۔
دونوں فیلڈز اب `ProviderAdmissionProposalV1` اور گورننس ریپرس سے گزرتے ہیں
  سی ایل آئی اور ٹیلی میٹرک JSON فکسچر۔

## توثیق اور گورننس سے منسلک

`ProviderAdvertBodyV1::validate` اور `ProviderAdmissionProposalV1::validate`
وہ غلط میٹا ڈیٹا کو مسترد کرتے ہیں:

اسکوپ کی صلاحیتوں کو پیک نہیں کرنا چاہئے اور دائرہ کار/گرانولریٹی کی حدود کو پورا کرنا چاہئے۔
- اسٹریم بجٹ/ٹرانسپورٹ کے اشارے سے مماثل TLV قدر کی ضرورت ہوتی ہے
  `CapabilityType::ChunkRangeFetch` اور اشارے کی فہرست خالی نہیں ہے۔
- ڈپلیکیٹ پروٹوکول اور غلط ترجیحات اشتہارات کو نشر کرنے سے پہلے توثیق کی غلطیوں کو بڑھاتی ہیں۔
- `compare_core_fields` کے ذریعے ڈومین ڈیٹا کے ل proposal تجویز/اشتہارات کے لفافوں کا موازنہ کرتا ہے
  تاکہ نان میچنگ گپ شپ پے لوڈ کو جلد ہی مسترد کردیا جائے۔

رجعت کی کوریج پر واقع ہے
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`۔

## ٹولز اور فکسچر

- پے لوڈ میں فراہم کنندہ کے اشتہارات `range_capability` ، `stream_budget` ، اور `transport_hints` شامل ہونا ضروری ہے۔
  `/v1/sorafs/providers` جوابات کے ذریعے چیک کریں اور فکسچر کو قبول کریں۔ شامل ہونا ضروری ہے
  پارسڈ صلاحیت ، نشریاتی بجٹ ، اور ٹیلی میٹرک ادخال کے اشارے سروں کے جے ایس او این کے خلاصے۔
- `cargo xtask sorafs-admission-fixtures` اس کے اندر اسٹریم بجٹ اور ٹرانسپورٹ کے اشارے دکھاتا ہے
  اس خصوصیت کی تعمیر جاری رکھنے کے لئے نگرانی بورڈ کی نگرانی کے لئے JSON کو نوادرات۔
- `fixtures/sorafs_manifest/provider_admission/` کے تحت فکسچر میں اب شامل ہیں:
  - معیاری ملٹی سورس اشتہارات ،
  -`multi_fetch_plan.json` SDKs کے لئے کثیر الجہتی بازیافت کے منصوبے کو طے کرنے کے لئے طے کرنا۔

## فارمیٹر اور Torii کا انضمام- Torii `/v1/sorafs/providers` تجزیہ کردہ رینج پاور ڈیٹا کے ساتھ واپس کرتا ہے
  `stream_budget` اور `transport_hints`۔ جب حذف ہوجاتا ہے تو ڈاون گریڈ انتباہات اٹھائے جاتے ہیں
  نئے ڈیٹا فراہم کرنے والے اور پورٹل اسکوپ پوائنٹس براہ راست صارفین کی طرح ہی پابندیوں کا اطلاق کرتے ہیں۔
- ملٹی سورس فارمیٹر (`sorafs_car::multi_fetch`) حدود کی حدود اور سیدھ کو نافذ کرتا ہے
  کام تفویض کرتے وقت صلاحیتوں اور اسٹریم بجٹ۔ یونٹ ٹیسٹ کور منظرنامے
  بہت بڑا حصہ ، ویرل تلاش ، اور کمی۔
- `sorafs_car::multi_fetch` ڈاؤن لوڈ سگنلز (سیدھ میں ناکامی ،
  کم احکامات) آپریٹرز کو یہ جاننے کے قابل بناتے ہیں کہ منصوبہ بندی کے دوران کچھ فراہم کنندگان کو کیوں خارج کردیا گیا۔

## ٹیلی میٹرک حوالہ

Torii Grafana بورڈ ** SoraFS میں مشاہدہ کرنے کی بازیافت ** میں اسکوپ بازیافت میٹر کھاتا ہے
(`dashboards/grafana/sorafs_fetch_observability.json`) اور ساتھ والے انتباہ کے قواعد
(`dashboards/alerts/sorafs_fetch_rules.yml`)۔

| اسکیل | قسم | ٹیگز | تفصیل |
| --------- | ------- | -------- | ------- |
| `torii_sorafs_provider_range_capability_total` | گیج | `feature` (`providers` ، `supports_sparse_offsets` ، `requires_alignment` ، `supports_merkle_proof` ، `stream_budget` ، `transport_hints`) | فراہم کرنے والے پیمانے کی صلاحیت کی خصوصیات کی تشہیر کرتے ہیں۔ |
| `torii_sorafs_range_fetch_throttle_events_total` | کاؤنٹر | `reason` (`quota` ، `concurrency` ، `byte_rate`) | پالیسی کے ذریعہ ڈومین کے لئے بازیافت کی کوششوں کو کم کیا گیا ہے۔ |
| `torii_sorafs_range_fetch_concurrency_current` | گیج | - | کنٹرول شدہ فعال اسٹریمز مشترکہ ہم وقت سازی کا بجٹ استعمال کرتے ہیں۔ |

پروم کیو ایل کی مثالیں:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

اس بات کی تصدیق کے لئے ڈسکاؤنٹ کاؤنٹر کا استعمال کریں کہ ملٹی سورس کوآرڈینیٹر ڈیفالٹس کو چالو کرنے سے پہلے کوٹے کا اطلاق کیا گیا ہے۔
اور الرٹ جب ہم آہنگی آپ کے بیڑے میں نشریاتی بجٹ کی حدود کے قریب پہنچ رہی ہے۔