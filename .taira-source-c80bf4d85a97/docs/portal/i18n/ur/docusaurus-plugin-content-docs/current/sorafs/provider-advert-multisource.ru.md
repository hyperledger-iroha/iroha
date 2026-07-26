---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس فراہم کنندہ کے اعلانات اور نظام الاوقات

اس صفحے میں کیننیکل تفصیلات کا خلاصہ پیش کیا گیا ہے
[`docs/source/sorafs/provider_advert_multisource.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)۔
اس دستاویز کو زبانی آریگرام Norito اور چینجلوگ کے لئے استعمال کریں۔ پورٹل ورژن
آپریشنل ہدایات ، SDK نوٹس اور ٹیلی میٹری لنکس باقی کے لئے رکھتا ہے
رن بکس کا سیٹ SoraFS۔

## سرکٹ Norito میں اضافے

### رینج کی صلاحیت (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - زیادہ سے زیادہ مستقل وقفہ (بائٹس) فی درخواست ، `>= 1`۔
- `min_granularity` - اجازت حاصل کریں ، `1 <= значение <= max_chunk_span`۔
- `supports_sparse_offsets`- ایک درخواست میں غیر ایڈجسٹ آفسیٹس کی اجازت دیتا ہے۔
- `requires_alignment` - اگر سچ ہے تو ، آفسیٹس کو `min_granularity` میں منسلک کیا جانا چاہئے۔
- `supports_merkle_proof` - پور شواہد کی حمایت کی نشاندہی کرتا ہے۔

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` کیننیکل فراہم کرتا ہے
کوڈنگ تاکہ گپ شپ کے پے لوڈ کا بوجھ عین مطابق رہیں۔

### `StreamBudgetV1`
- فیلڈز: `max_in_flight` ، `max_bytes_per_sec` ، اختیاری `burst_bytes`۔
- توثیق کے قواعد (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1` ، `max_bytes_per_sec > 0`۔
  - `burst_bytes` ، اگر بیان کیا گیا ہو تو ، `> 0` اور `<= max_bytes_per_sec` ہونا چاہئے۔

### `TransportHintV1`
- فیلڈز: `protocol: TransportProtocol` ، `priority: u8` (ونڈو 0-15 کنٹرول
  `TransportHintV1::validate`)۔
- جانا جاتا پروٹوکول: `torii_http_range` ، `quic_stream` ، `soranet_relay` ،
  `vendor_reserved`۔
- فراہم کنندہ کے لئے ڈپلیکیٹ پروٹوکول ریکارڈ کو مسترد کردیا گیا ہے۔

### `ProviderAdvertBodyV1` میں اضافے
- اختیاری `stream_budget: Option<StreamBudgetV1>`۔
- اختیاری `transport_hints: Option<Vec<TransportHintV1>>`۔
- دونوں فیلڈز `ProviderAdmissionProposalV1` ، گورننس لفافے ،
  سی ایل آئی فکسچر اور ٹیلی میٹری جےسن۔

## گورننس سے توثیق اور رابطہ

`ProviderAdvertBodyV1::validate` اور `ProviderAdmissionProposalV1::validate`
بدعنوان میٹا ڈیٹا کو مسترد کریں:

- بینڈوتھ کی صلاحیتوں کو صحیح طریقے سے ڈیکوڈ کرنا چاہئے اور حدود کا احترام کرنا چاہئے
  رینج/گرانولریٹی۔
- اسٹریم بجٹ/ٹرانسپورٹ کے اشارے TLV `CapabilityType::ChunkRangeFetch` کی ضرورت ہوتی ہے
  اور اشارے کی ایک خالی فہرست۔
- نقل و حمل کے پروٹوکول اور غلط ترجیحات غلطیوں کا سبب بنتی ہیں
  گپ شپ میلنگ اشتہارات سے پہلے توثیق۔
- داخلہ لفافے رینج میٹا ڈیٹا کے ذریعہ تجویز/اشتہارات کا موازنہ کرتے ہیں
  `compare_core_fields` تاکہ مماثل گپ شپ پے لوڈ کو پہلے سے مسترد کردیا جائے۔

رجعت کی کوریج میں ہے
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`۔

## ٹولز اور فکسچر

- فراہم کنندہ اشتہارات کے پے لوڈ میں `range_capability` ، `stream_budget` شامل ہونا ضروری ہے
  اور `transport_hints`۔ جوابات `/v1/sorafs/providers` اور داخلہ فکسچر کے ذریعے چیک کریں۔
  JSON خلاصہ میں پارسڈ صلاحیت ، اسٹریم بجٹ اور اشارے کی صفوں کو شامل کرنا ہوگا
  ٹیلی میٹک انجسٹ کے لئے۔
- `cargo xtask sorafs-admission-fixtures` اسٹریم بجٹ اور ٹرانسپورٹ کے اشارے دکھاتا ہے
  آپ کے JSON نوادرات میں تاکہ ڈیش بورڈز کی خصوصیت کے نفاذ کو ٹریک کریں۔
- `fixtures/sorafs_manifest/provider_admission/` میں فکسچر اب شامل ہیں:
  - کیننیکل ملٹی سورس اشتہارات ،
  - Torii تاکہ SDKs تعصب کو دوبارہ پیش کرسکیں
    ملٹی پیئر بازیافت کا منصوبہ۔

## آرکسٹریٹر اور Torii کے ساتھ انضمام- Torii `/v1/sorafs/providers` پارسڈ رینج صلاحیتوں کو میٹا ڈیٹا لوٹاتا ہے
  `stream_budget` اور `transport_hints` کے ساتھ۔ ڈاون گریڈ انتباہات جب متحرک ہوتے ہیں
  فراہم کرنے والے نئے میٹا ڈیٹا کے ذریعے اجازت دیتے ہیں ، اور گیٹ وے رینج کے اختتامی مقامات ایک ہی پابندیوں کا اطلاق کرتے ہیں
  براہ راست مؤکلوں کے لئے۔
- ملٹی سورس آرکسٹریٹر (`sorafs_car::multi_fetch`) اب حد کی حدود کا اطلاق کرتا ہے ،
  کام کی تقسیم میں مواقع اور اسٹریم بجٹ کی مساوات۔ یونٹ ٹیسٹ کا احاطہ
  بہت بڑے حصے ، ویرل کی تلاش اور تھروٹلنگ کے معاملات۔
- `sorafs_car::multi_fetch` ڈاؤن گریڈ سگنلز (صف بندی کی غلطیاں ،
  تھروٹلڈ درخواستیں) لہذا آپریٹرز سمجھ سکتے ہیں کہ مخصوص فراہم کنندگان کیوں
  منصوبہ بندی کے دوران چھوٹ گیا تھا۔

## ٹیلی میٹری کا حوالہ

Torii میں رینج بازیافت کا آلہ Grafana ڈیش بورڈ ** SoraFS مشاہدہ بازیافت **
(`dashboards/grafana/sorafs_fetch_observability.json`) اور متعلقہ الرٹ قواعد
(`dashboards/alerts/sorafs_fetch_rules.yml`)۔

| میٹرک | قسم | ٹیگز | تفصیل |
| --------- | ----- | ------- | ------------ |
| `torii_sorafs_provider_range_capability_total` | گیج | `feature` (`providers` ، `supports_sparse_offsets` ، `requires_alignment` ، `supports_merkle_proof` ، `stream_budget` ، `transport_hints`) | فراہم کنندہ جو رینج کی اہلیت کے افعال کی تشہیر کرتے ہیں۔ |
| `torii_sorafs_range_fetch_throttle_events_total` | کاؤنٹر | `reason` (`quota` ، `concurrency` ، `byte_rate`) | پالیسی کے ذریعہ گروپ کردہ تھروٹلنگ کے ساتھ رینج کی کوششیں۔ |
| `torii_sorafs_range_fetch_concurrency_current` | گیج | - | کل تنازعہ کے بجٹ کو استعمال کرنے والے فعال تھریڈز۔ |

پروم کیو ایل کی مثالیں:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

کوٹے کی تصدیق کے لئے تھروٹلنگ کاؤنٹر کا استعمال کریں کو قابل بنانے سے پہلے لاگو ہوتے ہیں
ملٹی سورس آرکیسٹریٹر ڈیفالٹ ، اور مقابلہ ہونے پر انتباہات اٹھائیں
آپ کے بیڑے کے لئے زیادہ سے زیادہ اسٹریم بجٹ اقدار کے قریب پہنچ رہا ہے۔