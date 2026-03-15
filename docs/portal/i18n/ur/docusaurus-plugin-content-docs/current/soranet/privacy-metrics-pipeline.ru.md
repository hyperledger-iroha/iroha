---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پرائیویسی میٹرکس پائپ لائن
عنوان: سورانیٹ پرائیویسی میٹرکس پائپ لائن (SNNET-8)
سائڈبار_لیبل: پرائیویسی میٹرکس پائپ لائن
تفصیل: ریلے اور آرکسٹریٹر سورانیٹ کے لئے رازداری کو برقرار رکھتے ہوئے ٹیلی میٹری کا مجموعہ۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` کی عکاسی کرتا ہے۔ پرانی دستاویزات ریٹائر ہونے تک دونوں کاپیاں ہم آہنگ رکھیں۔
:::

# سورانیٹ پرائیویسی میٹرکس پائپ لائن

SNNET-8 رن ٹائم ریلے کے لئے پرائیویسی پر مبنی ٹیلی میٹری کی سطح متعارف کراتا ہے۔ ریلے اب ہینڈ شیک اور سرکٹ کے واقعات کو منٹ کی بالٹیوں میں جمع کرتا ہے اور صرف موٹے Prometheus کاؤنٹرز کو برآمد کرتا ہے ، جس سے انفرادی سرکٹس غیر منسلک رہ جاتے ہیں اور آپریٹرز کو عملی مرئیت دیتے ہیں۔

## اجتماعی جائزہ

- رن ٹائم نفاذ `tools/soranet-relay/src/privacy.rs` میں `PrivacyAggregator` کے طور پر واقع ہے۔
- بالٹیاں دیوار کے وقت کے منٹ (`bucket_secs` ، پہلے سے طے شدہ 60 سیکنڈ) کے ذریعہ ترتیب دی جاتی ہیں اور ایک محدود رنگ میں محفوظ ہوتی ہیں (`max_completed_buckets` ، پہلے سے طے شدہ 120)۔ کلکٹر کے حصص کا اپنا ایک محدود بیکلاگ (`max_share_lag_buckets` ، پہلے سے طے شدہ 12) ہے تاکہ لیگیسی پریو ونڈوز میموری میں لیک ہونے یا ہنگیں جمع کرنے والوں کو نقاب پوش کرنے کے بجائے دبے ہوئے بالٹیوں کے طور پر دوبارہ ترتیب دے۔
- `RelayConfig::privacy` براہ راست `PrivacyConfig` پر نقشہ بناتا ہے ، افتتاحی ترتیبات (`bucket_secs` ، `min_handshakes` ، `flush_delay_buckets` ، `force_flush_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، I18000036X ، I18000036X ، I1800000036X ، I1800000036X ، I1800000036X ، `max_completed_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، `force_flush_buckets`. پروڈکشن رن ٹائم پہلے سے طے شدہ اقدار کو برقرار رکھتا ہے ، اور SNNET-8A محفوظ جمع کی دہلیز کو متعارف کراتا ہے۔
- رن ٹائم ماڈیولز ٹائپڈ مددگاروں کے ذریعہ ریکارڈ ایونٹس: `record_circuit_accepted` ، `record_circuit_rejected` ، `record_throttle` ، `record_throttle_cooldown` ، `record_capacity_reject` ، `record_active_sample` ، `record_active_sample` اور `record_active_sample` اور `record_active_sample` اور `record_active_sample` اور `record_active_sample` اور `record_active_sample` اور `record_active_sample`۔

## ایڈمن اینڈپوائنٹ ریلے

آپریٹرز `GET /privacy/events` کے ذریعے کچے مشاہدات کے لئے ایڈمن سننے والوں کو ریلے میں پول کرسکتے ہیں۔ اختتامی نقطہ واپس لائن سے طے شدہ JSON (`application/x-ndjson`) جس میں پے لوڈ `SoranetPrivacyEventV1` پر مشتمل ہے اندرونی `PrivacyEventBuffer` سے ظاہر ہوتا ہے۔ بفر `privacy.event_buffer_capacity` اندراجات (پہلے سے طے شدہ 4096) تک کے تازہ ترین واقعات کو اسٹور کرتا ہے اور پڑھنے پر صاف ہوجاتا ہے ، لہذا گمشدہ واقعات سے بچنے کے لئے کھرچنے والے کثرت سے پولنگ کرنے کی ضرورت ہوتی ہے۔ واقعات ایک ہی مصافحہ ، تھروٹل ، تصدیق شدہ بینڈوتھ ، فعال سرکٹ اور گار سگنلز کا احاطہ کرتے ہیں جو Prometheus کاؤنٹرز کو کھانا کھاتے ہیں ، جس سے بہاو جمع کرنے والوں کو نجی طور پر بریڈ کرمبس کو محفوظ محفوظ کرنے یا محفوظ جمع کرنے والے کام کے بہاؤ کو کھانا کھلانا پڑتا ہے۔

## ریلے کنفیگریشن

آپریٹرز سیکشن `privacy` کے ذریعے ریلے کنفیگریشن فائل میں نجی ٹیلی میٹری کیڈینس کو تشکیل دیتے ہیں:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

پہلے سے طے شدہ اقدار SNNET-8 تفصیلات کے ساتھ تعمیل کرتی ہیں اور بوٹ پر جانچ پڑتال کی جاتی ہیں:| فیلڈ | تفصیل | ڈیفالٹ |
| ------ | ---------- | ---------------- |
| `bucket_secs` | ہر جمع ونڈو کی چوڑائی (سیکنڈ)۔ | `60` |
| `min_handshakes` | بالٹی سے کاؤنٹرز کے جاری ہونے سے پہلے شرکاء کی کم سے کم تعداد۔ | `12` |
| `flush_delay_buckets` | فلش کرنے کی کوشش سے پہلے مکمل بالٹیاں کی تعداد۔ | `1` |
| `force_flush_buckets` | دبے ہوئے بالٹی کو جاری کرنے سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` | ذخیرہ شدہ بیکلاگ بالٹیاں (میموری کو حدود سے پرے بڑھنے سے روکتی ہیں)۔ | `120` |
| `max_share_lag_buckets` | دبانے تک کلکٹر کے حصص کے انعقاد کے لئے ونڈو۔ | `12` |
| `expected_shares` | ضم ہونے سے پہلے پریو کلکٹر کے حصص کی تعداد۔ | `2` |
| `event_buffer_capacity` | ایڈمن تھریڈ کے لئے بیک بلاگ این ڈی جےسن واقعات۔ | `4096` |

`force_flush_buckets` کو `flush_delay_buckets` کے نیچے ترتیب دینا ، زیرونگ دہلیز ، یا گارڈ ہولڈ کو غیر فعال کرنا اب تعیناتیوں سے بچنے کے لئے توثیق میں ناکام ہوجاتا ہے جو ریلے کی سطح پر ٹیلی میٹری لیک کریں گے۔

`event_buffer_capacity` حد `/admin/privacy/events` کو بھی محدود کرتی ہے ، اس بات کو یقینی بناتا ہے کہ کھرچنی غیر معینہ مدت تک پیچھے نہیں رہ سکتی ہے۔

## پریو کلیکٹر کے حصص

SNNET-8A دوہری جمع کرنے والوں کو تعینات کرتا ہے جو خفیہ طور پر مشترکہ پریو بالٹیاں جاری کرتے ہیں۔ آرکیسٹریٹر اب NDJSON اسٹریم `/privacy/events` کو ریکارڈز `SoranetPrivacyEventV1` کے لئے تجزیہ کرتا ہے اور `SoranetPrivacyPrioShareV1` کو `SoranetSecureAggregator::ingest_prio_share` کی ہدایت کرتے ہیں۔ جب `PrivacyBucketConfig::expected_shares` کے ذخائر آتے ہیں تو بالٹیاں جاری کی جاتی ہیں ، جو ریلے سلوک کی عکاسی کرتی ہیں۔ `SoranetPrivacyBucketMetricsV1` میں ضم ہونے سے پہلے بالٹی سیدھ اور ہسٹگرام شکل کے خلاف حصص کی توثیق کی جاتی ہے۔ اگر ہینڈ شیکس کی نتیجے میں تعداد `min_contributors` سے نیچے آجاتی ہے تو ، بالٹی کو `suppressed` کے طور پر برآمد کیا جاتا ہے ، جس سے ریلے کے اندر جمع کرنے والے کے طرز عمل کی نقل ہوتی ہے۔ دبے ہوئے ونڈوز اب `insufficient_contributors` ، `collector_suppressed` ، `collector_window_elapsed` ، اور `forced_flush_window_elapsed` کے درمیان فرق کرنے میں مدد کے لئے `suppression_reason` ٹیگ جاری کریں۔ وجہ `collector_window_elapsed` بھی متحرک ہے جب پریو شیئرز `max_share_lag_buckets` سے زیادہ لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے لمبے حصے۔

## استقبالیہ اختتامی نقطہ Torii

Torii اب دو HTTP ٹیلی میٹری کے اختتامی مقامات شائع کرتا ہے تاکہ ریلے اور جمع کرنے والے اپنی نقل و حمل کو سرایت کیے بغیر مشاہدات کو آگے بڑھا سکیں:- `POST /v2/soranet/privacy/event` پے لوڈ `RecordSoranetPrivacyEventDto` کو قبول کرتا ہے۔ جسم `SoranetPrivacyEventV1` کے علاوہ ایک اختیاری ٹیگ `source` لپیٹ دیتا ہے۔ Torii فعال ٹیلی میٹری پروفائل کے خلاف درخواست کی جانچ پڑتال کرتا ہے ، ایونٹ کو ریکارڈ کرتا ہے اور HTTP `202 Accepted` کے ساتھ جواب دیتا ہے جس کے ساتھ ساتھ Norito JSON لفافے میں شامل ونڈو (`bucket_start_unix` ، `bucket_duration_secs`) اور ریلے موڈ پر مشتمل ہے۔
- `POST /v2/soranet/privacy/share` پے لوڈ `RecordSoranetPrivacyShareDto` کو قبول کرتا ہے۔ جسم میں `SoranetPrivacyPrioShareV1` اور ایک اختیاری اشارہ `forwarded_by` لے کر چلتا ہے تاکہ آپریٹرز کو جمع کرنے والوں کے دھاگوں کا آڈٹ کرنے کی اجازت دی جاسکے۔ کامیاب گذارشات Norito JSON لفافے کے ساتھ HTTP Grafana واپس کریں ، جمع کرنے والے ، بالٹی ونڈو ، اور دبانے کا اشارہ۔ توثیق کی غلطیوں کو ٹیلی میٹری کے ردعمل `Conversion` پر نقشہ تیار کیا گیا ہے تاکہ جمع کرنے والوں کے مابین عین مطابق غلطی کو سنبھالنے کے ل .۔ آرکسٹریٹر ایونٹ لوپ اب پولنگ ریلے کے وقت ان حصص کو جاری کرتا ہے ، پریو بیٹری Torii کو ریلے پر بالٹیوں کے ساتھ ہم آہنگی میں رکھتا ہے۔

دونوں اختتامی نکات ٹیلی میٹری پروفائل کا احترام کرتے ہیں: جب میٹرکس غیر فعال ہوجاتے ہیں تو وہ `503 Service Unavailable` واپس کرتے ہیں۔ کلائنٹ Norito بائنری (`application/x.norito`) یا Norito JSON (`application/x.norito+json`) لاش بھیج سکتے ہیں۔ سرور خود بخود معیاری Torii ایکسٹریکٹرز کے ذریعے فارمیٹ پر بات چیت کرتا ہے۔

## میٹرکس Prometheus

ہر برآمد شدہ بالٹی میں لیبل `mode` (`entry` ، `middle` ، `exit`) اور `bucket_start` ہوتے ہیں۔ میٹرکس کے مندرجہ ذیل خاندان دستیاب ہیں:

| میٹرک | تفصیل |
| -------- | -------------- |
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` کے ساتھ مصافحہ کی درجہ بندی۔ |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` کے ساتھ تھروٹل کاؤنٹرز۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | جمع شدہ کوولڈاؤن کے دوروں کو تھروٹلڈ ہینڈ ہیکس کے ذریعہ تعاون کیا گیا۔ |
| `soranet_privacy_verified_bytes_total` | اندھے پیمائش کے ثبوت سے ثابت تھروپپٹ۔ |
| `soranet_privacy_active_circuits_{avg,max}` | اوسط اور فی بالٹی فعال سرکٹس کی چوٹی۔ |
| `soranet_privacy_rtt_millis{percentile}` | آر ٹی ٹی پرسنٹائل کا تخمینہ (`p50` ، `p90` ، `p99`)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | ہیشڈ گورننس ایکشن رپورٹ کاؤنٹرز ، کلید زمرہ ڈائجسٹ ہے۔ |
| `soranet_privacy_bucket_suppressed` | بالٹیوں کو روکا جاتا ہے کیونکہ شریک کی حد تک نہیں پہنچی ہے۔ |
| `soranet_privacy_pending_collectors{mode}` | بیٹری جمع کرنے والے کے حصص کو ضم کرنے کے منتظر ، ریلے وضع کے ذریعہ گروپ کیا گیا۔ |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` کے ساتھ دبے ہوئے بالٹیاں کاؤنٹرز تاکہ ڈیش بورڈز رازداری کی ناکامیوں کو منسوب کرسکیں۔ |
| `soranet_privacy_snapshot_suppression_ratio` | آخری نالی (0-1) پر دبے ہوئے/ضم شدہ کا حصہ ، الرٹ بجٹ کے لئے مفید ہے۔ |
| `soranet_privacy_last_poll_unixtime` | حالیہ کامیاب سروے کا UNIX وقت (کلکٹر-آئیڈل الرٹ کو طاقت دیتا ہے)۔ |
| `soranet_privacy_collector_enabled` | گیج ، جو `0` پر گرتا ہے جب رازداری جمع کرنے والا غیر فعال ہوتا ہے یا شروع نہیں ہوتا ہے (کلکٹر سے معذور الرٹ کو طاقت دیتا ہے)۔ |
| `soranet_privacy_poll_errors_total{provider}` | ریلے عرف پولنگ کی غلطیاں (ضابطہ کشائی کی غلطیوں ، HTTP کی ناکامیوں ، یا غیر متوقع اسٹیٹس کوڈز کے ساتھ اضافہ)۔ |

مشاہداتی بالٹیاں خاموش رہتی ہیں ، بغیر کھڑکیوں کو بنائے ڈیش بورڈز کو صاف رکھیں۔

## آپریشنل سفارشات1. ** ڈیش بورڈز ** - اوپر میٹرکس بنائیں ، `mode` اور `window_start` کے ذریعہ گروپ بندی کریں۔ جمع کرنے والوں یا ریلے کی پریشانیوں کی نشاندہی کرنے کے لئے گمشدہ ونڈوز کو اجاگر کریں۔ ممبروں کی کمی اور جمع کرنے والوں کی وجہ سے ہونے والی جگہوں کو جمع کرنے والے افراد کی وجہ سے ہونے والے دباؤ کے درمیان فرق کرنے کے لئے `soranet_privacy_suppression_total{reason}` کا استعمال کریں۔ Grafana میں اب ایک ** "دبانے کی وجوہات (5 میٹر)" ** ان کاؤنٹرز پر مبنی پینل اور ایک ** "دبے ہوئے بالٹی ٪" ** اسٹیٹ جو منتخب کردہ رینج پر `sum(soranet_privacy_bucket_suppressed) / count(...)` کا حساب لگاتا ہے لہذا آپریٹرز بجٹ کی خلاف ورزیوں کو ایک نظر میں دیکھ سکتے ہیں۔ ** "کلکٹر شیئر بیکلاگ" ** سیریز (`soranet_privacy_pending_collectors`) اور ** "اسنیپ شاٹ دبانے کا تناسب" ** اسٹیٹ اسٹیٹ نے خودکار رنز کے دوران پھنسے ہوئے جمع کرنے والوں اور بجٹ بڑھنے کو نمایاں کیا۔
2. ** انتباہ ** - نجی طور پر محفوظ میٹروں سے الارم کو ٹرگر کریں: POW مسترد پھٹ ، کولڈاؤن فریکوئنسی ، RTT بڑھنے اور صلاحیت کو مسترد کرنا۔ چونکہ کاؤنٹر ہر بالٹی کے اندر اجارہ دار ہوتے ہیں ، لہذا شرح کے آسان قواعد اچھی طرح سے کام کرتے ہیں۔
3. ** واقعہ -ردعمل ** - پہلے جمع شدہ ڈیٹا پر انحصار کریں۔ جب آپ کو گہری ڈیبگنگ کی ضرورت ہو تو ، ریلے سے اسنیپ شاٹس بالٹیوں کو دوبارہ چلانے کے لئے کہیں یا خام ٹریفک لاگز جمع کرنے کے بجائے بلائنڈ پیمائش کے ثبوت چیک کریں۔
4. ** ہولڈ ** - کھرچنے والے اکثر `max_completed_buckets` سے زیادہ نہ ہونے کے ل .۔ برآمد کنندگان کو آؤٹ پٹ Prometheus کو کیننیکل ماخذ کے طور پر سمجھنا چاہئے اور جمع کرانے کے بعد مقامی بالٹیوں کو حذف کرنا چاہئے۔

## تجزیات دبانے اور خودکار رنز

SNNET-8 کی قبولیت کا انحصار یہ ظاہر کرنے پر ہے کہ خودکار جمع کرنے والے صحت مند رہتے ہیں اور دبانے کو پالیسی کی حدود میں رکھا جاتا ہے (کسی بھی 30 منٹ کی ونڈو میں relay10 ٪ بالٹی فی ریلے)۔ ضروری ٹولز اب ذخیرے کے ساتھ شامل ہیں۔ آپریٹرز کو انہیں ہفتہ وار رسومات میں تعمیر کرنا چاہئے۔ Grafana میں نئے دبانے والے پینل ذیل میں پروم کیو ایل کے تاثرات کی نقل تیار کرتے ہیں ، جس میں کال ٹیموں کو دستی سوالات کا سہارا لینے سے پہلے براہ راست مرئیت ملتی ہے۔

### دبانے کے جائزے کے لئے پروم کیو ایل کی ترکیبیں

آپریٹرز کو مندرجہ ذیل پروم کیو ایل مددگاروں کو ہاتھ میں رکھنا چاہئے۔ دونوں کا ذکر جنرل ڈیش بورڈ Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) اور الرٹ مینجر قواعد میں کیا گیا ہے:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

اس بات کی تصدیق کرنے کے لئے آؤٹ پٹ تناسب کا استعمال کریں ** "دبے ہوئے بالٹی ٪" ** بجٹ کی دہلیز سے نیچے ہے۔ جب شرکاء کی تعداد غیر متوقع طور پر گرتی ہے تو فوری ردعمل کے لئے انتباہی مینیجر سے اسپائک ڈٹیکٹر سے رابطہ کریں۔

### بالٹیاں آف لائن رپورٹ کے لئے سی ایل آئی

`cargo xtask soranet-privacy-report` ورک اسپیس میں ایک وقت کے NDJSON اپ لوڈز کے لئے دستیاب ہے۔ ایک یا زیادہ ایڈمن ریلے برآمدات کی وضاحت کریں:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

مددگار `SoranetSecureAggregator` کے توسط سے گرفتاری چلاتا ہے ، دبانے کا خلاصہ STDOUT پر پرنٹ کرتا ہے ، اور اختیاری طور پر `--json-out <path|->` کے ذریعہ ایک ساختہ JSON رپورٹ لکھتا ہے۔ یہ براہ راست کلکٹر (`--bucket-secs` ، `--min-contributors` ، `--expected-shares` ، وغیرہ) کی طرح ترتیبات کو مدنظر رکھتا ہے ، جس سے آپریٹرز کو کسی واقعے کا تجزیہ کرتے وقت مختلف دہلیز کے ساتھ تاریخی گرفتاریوں کو دوبارہ چلانے کی اجازت ملتی ہے۔ Grafana کے اسکرین شاٹس کے ساتھ JSON کو منسلک کریں تاکہ SNNET-8 دبانے والے تجزیات پاس قابل آڈٹ رہیں۔### پہلے خودکار سیشن کے لئے چیک لسٹ

گورننس کو ابھی بھی اس بات کا ثبوت درکار ہے کہ پہلے خودکار سیشن نے دبانے والے بجٹ کو پورا کیا۔ مددگار اب `--max-suppression-ratio <0-1>` کو قبول کرتا ہے تاکہ جب دبے ہوئے بالٹیاں درست ونڈو (10 ٪ پہلے سے طے شدہ) سے تجاوز کریں یا جب ابھی تک کوئی بالٹیاں نہ ہوں تو CI یا آپریٹرز تیزی سے ناکام ہوسکیں۔ تجویز کردہ اسٹریم:

1. ایڈمن اینڈ پوائنٹس ریلے اور آرکسٹریٹر اسٹریم `/v2/soranet/privacy/event|share` سے `artifacts/sorafs_privacy/<relay>.ndjson` سے NDJSON برآمد کریں۔
2. پالیسی بجٹ مددگار چلائیں:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   کمانڈ مشاہدہ شدہ تناسب کو پرنٹ کرتا ہے اور غیر صفر کوڈ کے ساتھ باہر نکلتا ہے جب بجٹ سے تجاوز کیا جاتا ہے ** یا ** جب بالٹی ابھی تیار نہیں ہوتی ہے ، اس بات کا اشارہ کرتے ہیں کہ ابھی تک ٹیلی میٹری کو رن کے لئے تیار نہیں کیا گیا ہے۔ براہ راست میٹرکس کو یہ ظاہر کرنا چاہئے کہ `soranet_privacy_pending_collectors` صفر میں ضم ہوجاتا ہے اور `soranet_privacy_snapshot_suppression_ratio` رن کے دوران اسی بجٹ سے نیچے رہتا ہے۔
3. ڈیفالٹ ٹرانسپورٹ کو تبدیل کرنے سے پہلے JSON آؤٹ پٹ اور CLI لاگ ان SNNET-8 شواہد پیکیج کے ساتھ محفوظ شدہ دستاویزات بنائیں تاکہ جائزہ لینے والے عین مطابق نمونے کو دوبارہ پیش کرسکیں۔

## اگلے اقدامات (Snnet-8a)

- حصص کے استقبال کو رن ٹائم سے مربوط کرکے ڈبل پریو جمع کرنے والوں کو مربوط کریں تاکہ ریلے اور جمع کرنے والے مستقل پے لوڈز `SoranetPrivacyBucketMetricsV1` جاری کریں۔ *(ہو گیا - `crates/sorafs_orchestrator/src/lib.rs` اور متعلقہ ٹیسٹ میں `ingest_privacy_payload` دیکھیں۔)*
- مشترکہ Prometheus ڈیش بورڈ JSON اور انتباہی قواعد شائع کریں جس میں دبانے والے فرقوں ، صحت جمع کرنے والوں اور گمنامی کا احاطہ کیا گیا ہے۔ *(ہو گیا - `dashboards/grafana/soranet_privacy_metrics.json` ، `dashboards/alerts/soranet_privacy_rules.yml` ، `dashboards/alerts/soranet_policy_rules.yml` اور چیک دیکھیں۔)*
- `privacy_metrics_dp.md` میں بیان کردہ تفریق پرائیویسی انشانکن نمونے تیار کریں ، جس میں تولیدی نوٹ بک اور گورننس ڈائجسٹ بھی شامل ہے۔ *(DONE - نوٹ بک اور نمونے `scripts/telemetry/run_privacy_dp.py` کے ذریعہ تیار کیے گئے ہیں C CI ریپر `scripts/telemetry/run_privacy_dp_notebook.sh` ورک فلو `.github/workflows/release-pipeline.yml` کے ذریعے نوٹ بک پر عمل درآمد I `docs/source/status/soranet_privacy_dp_digest.md` میں محفوظ شدہ ڈائجسٹ۔)

موجودہ ریلیز SNNET-8 کا بنیادی حصہ فراہم کرتی ہے: تعصب پسند ، نجی طور پر محفوظ ٹیلی میٹری جو براہ راست موجودہ Prometheus سکریپرس اور ڈیش بورڈز سے مربوط ہوتی ہے۔ تفریق پرائیویسی انشانکن نمونے کی جگہ موجود ہے ، ریلیز پائپ لائن ورک فلو نوٹ بک کی آؤٹ پٹ کو تازہ ترین رکھتا ہے ، اور باقی کام پہلے خودکار رن کی نگرانی اور دبانے والے انتباہ تجزیات کو بڑھانے پر مرکوز ہے۔