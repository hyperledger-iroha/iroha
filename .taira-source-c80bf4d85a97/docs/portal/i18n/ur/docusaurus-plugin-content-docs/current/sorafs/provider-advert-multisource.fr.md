---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس وینڈر کے اعلانات اور شیڈولنگ

اس صفحے میں کیننیکل تفصیلات کو شامل کیا گیا ہے
[`docs/source/sorafs/provider_advert_multisource.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)۔
اس دستاویز کو زبانی Norito اسکیموں اور چینجلوگس کے لئے استعمال کریں۔ پورٹل کی کاپی
آپریٹر کی ہدایات ، SDK نوٹ اور ٹیلی میٹری حوالہ جات ہر چیز کے قریب رکھتا ہے
رن بکس SoraFS۔

## اسکیما Norito میں اضافے

### رینج کی گنجائش (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - فی درخواست ، سب سے بڑی متناسب حد (بائٹس) ، `>= 1`۔
- `min_granularity` - SEEC کی قرارداد ، `1 <= valeur <= max_chunk_span`۔
- `supports_sparse_offsets`- ایک ہی سوال میں غیر متضاد آفسیٹس کی اجازت دیتا ہے۔
- `requires_alignment` - اگر سچ ہے تو ، آفسیٹس کو `min_granularity` کے ساتھ سیدھ میں رکھنا چاہئے۔
- `supports_merkle_proof` - پور کوکیز کے لئے تعاون کی نشاندہی کرتا ہے۔

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
- دونوں فیلڈز اب `ProviderAdmissionProposalV1` ، لفافے کے ذریعے گزرتے ہیں
  گورننس ، سی ایل آئی فکسچر اور ٹیلی میٹرک جےسن۔

## توثیق اور گورننس سے لنک

`ProviderAdvertBodyV1::validate` اور `ProviderAdmissionProposalV1::validate`
ناقص تشکیل شدہ میٹا ڈیٹا کو مسترد کریں:

- حد کی صلاحیتوں کو رینج/گرانولریٹی کی حدود کو ڈی کوڈ کرنا اور ان کا احترام کرنا چاہئے۔
- اسٹریم بجٹ / ٹرانسپورٹ کے اشارے میں TLV `CapabilityType::ChunkRangeFetch` کی ضرورت ہوتی ہے
  اسی طرح اور اشارے کی غیر خالی فہرست۔
- ڈپلیکیٹ ٹرانسپورٹ پروٹوکول اور غلط ترجیحات غلطیاں پیدا کرتی ہیں
  اشتہارات کی نشریات سے پہلے توثیق۔
- داخلہ لفافے رینج میٹا ڈیٹا کے لئے تجویز/اشتہارات کا موازنہ کرتے ہیں
  `compare_core_fields` تاکہ مماثل گپ شپ پے لوڈ کو جلد ہی مسترد کردیا جائے۔

رجعت کی کوریج میں پایا جاتا ہے
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`۔

## ٹولز اور فکسچر- سپلائر AD پے لوڈز میں میٹا ڈیٹا `range_capability` شامل ہونا ضروری ہے ،
  `stream_budget` اور `transport_hints`۔ `/v1/sorafs/providers` جوابات اور اس کے ذریعے توثیق کریں
  انٹیک فکسچر ؛ JSON کے خلاصے میں پارس کی صلاحیت ، اسٹریم بجٹ شامل ہونا چاہئے
  اور ٹیلی میٹرک ادخال کے لئے اشارے کی میزیں۔
- Torii اسٹریم بجٹ اور ٹرانسپورٹ کے اشارے کو بے نقاب کرتا ہے
  اس کے JSON نمونے تاکہ ڈیش بورڈز اس خصوصیت کو اپنانے کا سراغ لگائیں۔
- `fixtures/sorafs_manifest/provider_admission/` کے تحت فکسچر میں اب شامل ہیں:
  - کیننیکل ملٹی سورس اشتہارات ،
  - `multi_fetch_plan.json` تاکہ SDK سویٹس بازیافت کے منصوبے کو دوبارہ چلاسکیں
    ڈٹرمینسٹک ملٹی پیئر۔

## آرکسٹریٹر اور Torii کے ساتھ انضمام

- Torii `/v1/sorafs/providers` رینج رینج کی گنجائش میٹا ڈیٹا کے ساتھ تجزیہ کیا گیا
  `stream_budget` اور `transport_hints`۔ ڈاون گریڈ انتباہات جب متحرک ہوتے ہیں
  فراہم کرنے والے نئے میٹا ڈیٹا ، اور گیٹ وے رینج کے اختتامی مقامات کو چھوڑ دیتے ہیں
  براہ راست صارفین کے لئے وہی رکاوٹیں لگائیں۔
- ملٹی سورس آرکسٹریٹر (`sorafs_car::multi_fetch`) اب کی حدود کا اطلاق کرتا ہے
  کام تفویض کرتے وقت رینج ، صلاحیت کی صف بندی اور بہاؤ کے بجٹ۔
  یونٹ کے ٹیسٹ بہت بڑے حصے کے منظرناموں کا احاطہ کرتے ہیں ، بکھرے ہوئے اور تلاش کرتے ہیں
  تھروٹلنگ۔
- `sorafs_car::multi_fetch` ڈاؤن لوڈ سگنلز (سیدھ میں ناکامی ،
  تھروٹلڈ سوالات) تاکہ آپریٹرز کچھ فراہم کرنے والے کیوں تلاش کرسکیں
  منصوبہ بندی کے دوران نظرانداز کیا گیا۔

## ٹیلی میٹری کا حوالہ

Torii کی رینج بازیافت کا آلہ ڈیش بورڈ Grafana کو طاقت دیتا ہے
** SoraFS مشاہدہ کی بازیافت ** (`dashboards/grafana/sorafs_fetch_observability.json`) اور
ایسوسی ایٹڈ الرٹ قواعد (`dashboards/alerts/sorafs_fetch_rules.yml`)۔

| میٹرک | قسم | لیبل | تفصیل |
| --------- | ------ | -------------- | --------------- |
| `torii_sorafs_provider_range_capability_total` | گیج | `feature` (`providers` ، `supports_sparse_offsets` ، `requires_alignment` ، `supports_merkle_proof` ، `stream_budget` ، `transport_hints`) | رینج صلاحیت کی خصوصیات کا اعلان کرنے والے دکاندار۔ |
| `torii_sorafs_range_fetch_throttle_events_total` | کاؤنٹر | `reason` (`quota` ، `concurrency` ، `byte_rate`) | پالیسی کے ذریعہ ساحل سمندر کی بازیافت کی کوششیں۔ |
| `torii_sorafs_range_fetch_concurrency_current` | گیج | - | فعال اسٹریمز ہم آہنگی شیئر بجٹ کو استعمال کرتے رہتے ہیں۔ |

پروم کیو ایل کی مثالیں:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

قابل بنانے سے پہلے کوٹہ نفاذ کی تصدیق کے لئے تھروٹلنگ کاؤنٹر کا استعمال کریں
ملٹی سورس آرکسٹریٹر کی پہلے سے طے شدہ اقدار ، اور جب مقابلہ ہوتا ہے تو الرٹ
آپ کو اپنے بیڑے کے لئے زیادہ سے زیادہ اسٹریم بجٹ کے قریب لاتا ہے۔