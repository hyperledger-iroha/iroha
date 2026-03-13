---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس فراہم کنندگان اور شیڈولنگ سے اشتہارات

اس صفحے میں کیننیکل تفصیلات کا خلاصہ پیش کیا گیا ہے
[`docs/source/sorafs/provider_advert_multisource.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)۔
اس دستاویز کو زبانی Norito اسکیموں اور چینجلوگس کے لئے استعمال کریں۔ پورٹل کی کاپی
آپریٹر کی رہنمائی ، SDK نوٹس ، اور ٹیلی میٹری حوالہ جات باقی کے قریب رکھتا ہے
رن بکس SoraFS کی۔

Norito اسکیم میں اضافہ

### رینج کی صلاحیت (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - ہر درخواست میں سب سے بڑا مسلسل مدت (بائٹس) ، `>= 1`۔
- `min_granularity` - ریزولوشن کی تلاش ، `1 <= valor <= max_chunk_span`۔
- `supports_sparse_offsets` - کسی درخواست میں غیر متضاد آفسیٹس کی اجازت دیتا ہے۔
- `requires_alignment` - جب سچ ہے تو ، آفسیٹس کو `min_granularity` کے ساتھ ہم آہنگ ہونا چاہئے۔
- `supports_merkle_proof` - POR گواہ کی حمایت کی نشاندہی کرتا ہے۔

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
- ہر فراہم کنندہ کے مطابق ڈپلیکیٹ پروٹوکول اندراجات کو مسترد کردیا جاتا ہے۔

### `ProviderAdvertBodyV1` میں اضافے
- `stream_budget` اختیاری: `Option<StreamBudgetV1>`۔
- `transport_hints` اختیاری: `Option<Vec<TransportHintV1>>`۔
- دونوں فیلڈز اب `ProviderAdmissionProposalV1` ، گورننس لفافے ،
  ٹیلی میٹری سی ایل آئی اور JSON فکسچر۔

## گورننس کے ساتھ توثیق اور ربط

`ProviderAdvertBodyV1::validate` اور `ProviderAdmissionProposalV1::validate`
خراب شدہ میٹا ڈیٹا کو مسترد کریں:

- رینج کی صلاحیتوں کو ڈی اوڈ اور اسپین/گرانولریٹی حدود کی تعمیل کرنی ہوگی۔
- اسٹریم بجٹ/ٹرانسپورٹ کے اشارے میں TLV `CapabilityType::ChunkRangeFetch` کی ضرورت ہوتی ہے
  اسی اور خالی اشارے کی فہرست۔
- ڈپلیکیٹ ٹرانسپورٹ پروٹوکول اور غلط ترجیحات توثیق کی غلطیاں پیدا کرتی ہیں
  اس سے پہلے کہ اشتہارات گپ شپ ہوں۔
- داخلہ لفافے تجویز/اشتہارات کا موازنہ میٹا ڈیٹا کے ذریعے کرتے ہیں
  `compare_core_fields` تاکہ مختلف گپ شپ پے لوڈ کو جلدی سے مسترد کردیا جائے۔

کوریج کی واپسی جاری ہے
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`۔

## ٹولنگ اور فکسچر

- فراہم کنندہ کے اشتہار پے لوڈز میں میٹا ڈیٹا `range_capability` شامل ہونا ضروری ہے ،
  `stream_budget` اور `transport_hints`۔ `/v2/sorafs/providers` کے جوابات کے ذریعے توثیق کریں
  اور داخلہ فکسچر ؛ JSON کے خلاصے میں پارسڈ صلاحیت ، ندی کو شامل کرنا چاہئے
  ٹیلی میٹری کے لئے بجٹ اور اشارے کی صفیں۔
- `cargo xtask sorafs-admission-fixtures` ندی کے بجٹ اور ٹرانسپورٹ کے اشارے دکھاتا ہے
  آپ کے JSON نمونے میں سے تاکہ ڈیش بورڈز اس خصوصیت کو اپنانے کا سراغ لگاسکیں۔
- `fixtures/sorafs_manifest/provider_admission/` کے تحت فکسچر میں اب شامل ہیں:
  - کیننیکل ملٹی ارگین اشتہارات ،
  - `multi_fetch_plan.json` SDK سوٹ کے لئے بازیافت کرنے کے منصوبے کو دوبارہ پیش کرنے کے لئے
    ڈٹرمینسٹک ملٹی پیئر۔

## آرکسٹریٹر اور Torii کے ساتھ انضمام- Torii `/v2/sorafs/providers` پارسڈ رینج میٹا ڈیٹا کے ساتھ ساتھ لوٹاتا ہے
  `stream_budget` اور `transport_hints`۔ ڈاون گریڈ انتباہات جب ٹرگر کرتے ہیں
  فراہم کرنے والے نئے میٹا ڈیٹا کو چھوڑ دیتے ہیں ، اور گیٹ وے رینج کے اختتامی مقامات نئے میٹا ڈیٹا کو لاگو کرتے ہیں۔
  براہ راست صارفین کے لئے ایک ہی پابندیاں۔
- ملٹی سورس آرکسٹریٹر (`sorafs_car::multi_fetch`) اب ڈیٹا کی حدود کا اطلاق کرتا ہے
  کام تفویض کرتے وقت رینج ، صلاحیتوں اور اسٹریم بجٹ کی صف بندی۔ یونٹ
  ٹیسٹوں میں بہت بڑے حص ، ے ، ویرل سیکر اور تھروٹلنگ کے منظرنامے شامل ہیں۔
- `sorafs_car::multi_fetch` ڈاؤن گریڈ سگنلز (صف بندی کی غلطیاں ،
  آپریٹرز کو یہ معلوم کرنے کے لئے کہ مخصوص فراہم کنندگان کو ٹریک کرنے کے لئے تھروٹلڈ درخواستیں
  منصوبہ بندی کے دوران نظرانداز کیا گیا۔

## ٹیلی میٹری کا حوالہ

Torii رینج بازیافت آلہ Grafana ڈیش بورڈ کو کھانا کھلاتا ہے
** SoraFS مشاہدہ کی بازیافت ** (`dashboards/grafana/sorafs_fetch_observability.json`) اور
ایسوسی ایٹڈ الرٹ قواعد (`dashboards/alerts/sorafs_fetch_rules.yml`)۔

| میٹرک | قسم | لیبل | تفصیل |
| --------- | ------ | -------- | ----------- |
| `torii_sorafs_provider_range_capability_total` | گیج | `feature` فراہم کنندہ جو رینج کی اہلیت کی خصوصیات کی تشہیر کرتے ہیں۔ |
| `torii_sorafs_range_fetch_throttle_events_total` | کاؤنٹر | `reason` (`quota` ، `concurrency` ، `byte_rate`) | تھروٹلڈ رینج بازیافت کی کوششوں کو پالیسی کے ذریعہ گروپ کیا گیا۔ |
| `torii_sorafs_range_fetch_concurrency_current` | گیج | - | مشترکہ مسابقت کے بجٹ کو استعمال کرنے والے محفوظ فعال اسٹریمز۔ |

پروم کیو ایل کی مثالیں:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

چالو کرنے سے پہلے کوٹہ نفاذ کی تصدیق کے لئے تھروٹلنگ کاؤنٹر کا استعمال کریں
جب مقابلہ قریب آتا ہے تو ملٹی سورس آرکسٹریٹر ڈیفالٹ اور الرٹ کرنے کے لئے
آپ کے بیڑے کے اسٹریم بجٹ میں سے زیادہ سے زیادہ۔