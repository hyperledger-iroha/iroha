---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پرائیویسی میٹرکس پائپ لائن
عنوان: سورانیٹ پرائیویسی میٹرکس پائپ لائن (SNNET-8)
سائڈبار_لیبل: پرائیویسی میٹرکس پائپ لائن
تفصیل: سورانیٹ ریلے اور آرکسٹریٹرز کے لئے پرائیویسی کو محفوظ رکھنے والا ٹیلی میٹری۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` کی عکاسی کرتا ہے۔ جب تک میراثی دستاویزات ریٹائر نہ ہوجائیں دونوں کاپیاں ہم آہنگی میں رکھیں۔
:::

# سورانیٹ پرائیویسی میٹرکس پائپ لائن

SNNET-8 میں رازداری سے واقف ٹیلی میٹری کی سطح متعارف کروائی گئی ہے
ریلے رن ٹائم۔ اب ریلے میں مصافحہ اور سرکٹ کے واقعات میں اضافہ ہوتا ہے
ایک منٹ کی بالٹیاں اور برآمدات صرف موٹے Prometheus کاؤنٹرز ، رکھتے ہوئے
قابل عمل مرئیت کی پیش کش کرتے وقت انفرادی سرکٹس کو ڈیکپل کیا
آپریٹرز کے لئے۔

## جمع کرنے والا خلاصہ

- `tools/soranet-relay/src/privacy.rs` AS میں رن ٹائم عمل درآمد رہتا ہے
  `PrivacyAggregator`۔
- بالٹیاں فی گھڑی منٹ (`bucket_secs` ، پہلے سے طے شدہ 60 سیکنڈ) کی ترتیب دی جاتی ہیں
  وہ ایک پابند رنگ (`max_completed_buckets` ، پہلے سے طے شدہ 120) میں محفوظ ہیں۔ حصص
  جمع کرنے والے اپنے پابند بیک بلاگ کو برقرار رکھتے ہیں (`max_share_lag_buckets` ، پہلے سے طے شدہ 12)
  تاکہ پریو باسی ونڈوز کو حذف شدہ بالٹیوں کے طور پر خالی کردیا جائے
  میموری یا ماسک پھنسے ہوئے جمع کرنے والوں کو لیک کریں۔
- `RelayConfig::privacy` براہ راست `PrivacyConfig` پر نقشہ جات ، ایڈجسٹمنٹ knobs کو بے نقاب کرتے ہوئے
  .
  `max_completed_buckets` ، `max_share_lag_buckets` ، `expected_shares`)۔ رن ٹائم
  پروڈکشن ڈیفالٹس کو برقرار رکھتی ہے جبکہ SNNET-8A کی دہلیز متعارف کراتا ہے
  محفوظ جمع
- رن ٹائم ماڈیول ٹائپڈ مددگاروں کے ساتھ واقعات کو رجسٹر کرتے ہیں:
  `record_circuit_accepted` ، `record_circuit_rejected` ، `record_throttle` ،
  `record_throttle_cooldown` ، `record_capacity_reject` ، `record_active_sample` ،
  `record_verified_bytes` ، اور `record_gar_category`۔

## ریلے اینڈپوائنٹ ایڈمن

آپریٹرز مشاہدات کے لئے ریلے کے ایڈمن سننے والوں سے مشورہ کرسکتے ہیں۔
`GET /privacy/events` کے ذریعے خام۔ اختتامی نقطہ JSON کی طرف سے لوٹاتا ہے
پے لوڈ کے ساتھ نئی لائنیں (`application/x-ndjson`) `SoranetPrivacyEventV1`
اندرونی `PrivacyEventBuffer` سے ظاہر ہوتا ہے۔ بفر واقعات کا انعقاد کرتا ہے
`privacy.event_buffer_capacity` اندراجات (پہلے سے طے شدہ 4096) تک اور ہیں
پڑھنے پر خالی ، لہذا کھرچنیوں کو کافی رائے شماری کرنی ہوگی
خلا کو مت چھوڑیں۔ واقعات ایک ہی مصافحہ ، تھروٹل ،
تصدیق شدہ بینڈوتھ ، ایکٹو سرکٹ اور گار جو Prometheus کاؤنٹرز کو کھانا کھلاتے ہیں ،
بہاو ​​جمع کرنے والوں کو رازداری سے محفوظ بریڈ کرمبس کو محفوظ کرنے کی اجازت دینا
یا محفوظ جمع کرنے والے کام کے بہاؤ کو کھانا کھلائیں۔

## ریلے کنفیگریشن

آپریٹرز میں پرائیویسی ٹیلی میٹری کیڈینس کو ایڈجسٹ کرتے ہیں
سیکشن `privacy` کے ذریعے ریلے کنفیگریشن:

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

فیلڈ ڈیفالٹس SNNET-8 تفصیلات سے مماثل ہیں اور اس پر توثیق کی جاتی ہیں
بوجھ:| فیلڈ | تفصیل | ڈیفالٹ |
| ------- | ------------- | -------- |
| `bucket_secs` | ہر جمع ونڈو کی چوڑائی (سیکنڈ)۔ | `60` |
| `min_handshakes` | بالٹی جاری کرنے سے پہلے کم سے کم شراکت کاروں کی تعداد۔ | `12` |
| `flush_delay_buckets` | فلش سے پہلے انتظار کرنے کے لئے بالٹیاں مکمل کریں۔ | `1` |
| `force_flush_buckets` | حذف شدہ بالٹی جاری کرنے سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` | برقرار رکھی ہوئی بالٹیوں کا بیکلاگ (لامحدود میموری سے پرہیز کریں)۔ | `120` |
| `max_share_lag_buckets` | حذف کرنے سے پہلے کلکٹر کے حصص کے لئے برقرار رکھنے والی ونڈو۔ | `12` |
| `expected_shares` | مل کر پریو کے حصص کی ضرورت ہے۔ | `2` |
| `event_buffer_capacity` | ایڈمن اسٹریم کے لئے واقعات کا NDJSON بیکلاگ۔ | `4096` |

`force_flush_buckets` کے نیچے `flush_delay_buckets` ، سیٹ کریں
صفر پر دہلیز یا ہولڈ گارڈ کو غیر فعال کرنا اب ناکام ہوجاتا ہے
ریلے ٹیلی میٹری کو فلٹر کرنے والے تعیناتیوں سے بچنے کے لئے توثیق۔

حد `event_buffer_capacity` بھی `/admin/privacy/events` کو محدود کرتا ہے ،
اس بات کو یقینی بنانا کہ کھرچنی غیر معینہ مدت تک پیچھے نہیں رہ جاتی ہے۔

## کلکٹر پریو سے حصص

SNNET-8A دوہری جمع کرنے والے تعینات کرتا ہے جو خفیہ شیئرنگ کے ساتھ پریو بالٹیوں کا اخراج کرتے ہیں۔
اب آرکسٹریٹر دونوں کے لئے NDJSON اسٹریم `/privacy/events` کو پارس کرتا ہے
اندراجات `SoranetPrivacyEventV1` حصص کے طور پر `SoranetPrivacyPrioShareV1` ،
انہیں `SoranetSecureAggregator::ingest_prio_share` پر بھیجنا۔ بالٹیاں ہیں
جاری کیا گیا جب `PrivacyBucketConfig::expected_shares` شراکتیں آتی ہیں ،
ریلے میں جمع کرنے والے کے طرز عمل کی عکاسی کرنا۔ حصص کی توثیق کی گئی ہے
بذریعہ بالٹی سیدھ اور ہسٹوگرام کی شکل میں امتزاج کرنے سے پہلے
`SoranetPrivacyBucketMetricsV1`۔ اگر مشترکہ مصافحہ کی گنتی آتی ہے
`min_contributors` کے نیچے ، بالٹی کو `suppressed` کے طور پر برآمد کیا جاتا ہے ، جس کی عکاسی ہوتی ہے
ریلے میں جمع کرنے والے کا سلوک۔ ونڈوز اب حذف ہوگئے
آپریٹرز کے درمیان فرق کرنے کے لئے `suppression_reason` ٹیگ جاری کریں
`insufficient_contributors` ، `collector_suppressed` ، `collector_window_elapsed`
اور ٹیلی میٹری کے فرق کی تشخیص کرتے وقت `forced_flush_window_elapsed`۔ وجہ
`collector_window_elapsed` میں بھی فائر ہوتا ہے جب پریو کے حصص زیادہ دیر تک رہتے ہیں
`max_share_lag_buckets` سے پرے ، بھری ہوئی جمع کرنے والوں کو بغیر دکھائی دیتا ہے
باسی جمع کرنے والوں کو میموری میں رکھیں۔

## ingesion اختتامی نقطہ Torii

Torii اب گیٹڈ ٹیلی میٹری کے ساتھ دو HTTP اختتامی نکات کو بے نقاب کرتا ہے تاکہ ریلے اور
جمع کرنے والے بیسپوک ٹرانسپورٹ کو سرایت کیے بغیر مشاہدات کو آگے بڑھاتے ہیں:- `POST /v1/soranet/privacy/event` ایک پے لوڈ کو قبول کرتا ہے
  `RecordSoranetPrivacyEventDto`۔ باڈی سوٹ ایک `SoranetPrivacyEventV1` لپیٹتا ہے
  اس کے علاوہ ایک اختیاری لیبل `source`۔ Torii کے خلاف درخواست کی توثیق کرتا ہے
  فعال ٹیلی میٹری پروفائل ، ایونٹ کو ریکارڈ کرتا ہے اور HTTP کے ساتھ جواب دیتا ہے
  `202 Accepted` ایک Norito json لفافہ کے ساتھ ونڈو پر مشتمل ہے
  بالٹی (`bucket_start_unix` ، `bucket_duration_secs`) اور دی سے حساب کیا گیا
  ریلے وضع۔
- `POST /v1/soranet/privacy/share` ایک پے لوڈ `RecordSoranetPrivacyShareDto` کو قبول کرتا ہے۔
  جسم میں `SoranetPrivacyPrioShareV1` اور ایک اختیاری اشارہ `forwarded_by` ہے
  آپریٹرز کو جمع کرنے والے کے بہاؤ کا آڈٹ کرنے کے لئے۔ کامیاب فراہمی
  Norito JSON لفافے کے ساتھ HTTP `202 Accepted` واپس کریں
  کلکٹر ، بالٹی ونڈو اور دبانے کا اشارہ۔ کی ناکامی
  ایک ٹیلی میٹری کے ردعمل `Conversion` کو محفوظ رکھنے کے لئے توثیق کا نقشہ
  جمع کرنے والوں میں عین مطابق غلطی سے نمٹنے کے۔ آرکسٹریٹر ایونٹ لوپ
  اب ان حصص کو پولنگ کرتے وقت ، جمع کرنے والے کو برقرار رکھتے ہوئے ، جمع کرتا ہے
  Torii پریو آن ریلے بالٹیوں کے ساتھ ہم آہنگ ہے۔

دونوں نکات ٹیلی میٹری پروفائل کا احترام کرتے ہیں: وہ 3 503 سروس جاری کرتے ہیں
دستیاب نہیں جب میٹرکس غیر فعال ہوجاتے ہیں۔ کلائنٹ بھیج سکتے ہیں
Norito بائنری (`application/x.norito`) یا Norito JSON (`application/x.norito+json`) لاشیں ؛
سرور معیاری فائل ایکسٹریکٹرز کے ذریعہ خود بخود فارمیٹ پر بات چیت کرتا ہے۔
Torii۔

## میٹرکس Prometheus

ہر برآمد شدہ بالٹی میں ٹیگ `mode` (`entry` ، `middle` ، `exit`) اور ہیں
`bucket_start`۔ میٹرکس کے مندرجہ ذیل خاندان جاری کیے گئے ہیں:

| میٹرک | تفصیل |
| -------- | --------------- |
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` کے ساتھ ہینڈ شیک درجہ بندی۔ |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` کے ساتھ تھروٹل کاؤنٹرز۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | تھروٹنڈ ہینڈ ہیکس کے ذریعہ تعاون کیا گیا کوولڈاؤن کے دوروں میں شامل کیا گیا۔ |
| `soranet_privacy_verified_bytes_total` | اندھے پیمائش کے ثبوتوں کی تصدیق شدہ بینڈوتھ۔ |
| `soranet_privacy_active_circuits_{avg,max}` | اوسط اور چوٹی کے فعال سرکٹس فی بالٹی۔ |
| `soranet_privacy_rtt_millis{percentile}` | آر ٹی ٹی پرسنٹائل تخمینہ (`p50` ، `p90` ، `p99`)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | GAR کاؤنٹرز کیٹیگری ڈائجسٹ کے ذریعہ ہیش ہوئی۔ |
| `soranet_privacy_bucket_suppressed` | بالٹیوں کو روکا گیا کیونکہ شراکت دار کی دہلیز پوری نہیں ہوئی تھی۔ |
| `soranet_privacy_pending_collectors{mode}` | ریلے موڈ کے ذریعہ گروپ بندی کرنے کے لئے زیر التواء جمع کرنے والے جمع کرنے والوں کو شیئر کریں۔ |
| `soranet_privacy_suppression_total{reason}` | رازداری کے فرق کو منسوب کرنے کے لئے `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` کے ساتھ بالٹی کاؤنٹرز کو حذف کردیا گیا۔ |
| `soranet_privacy_snapshot_suppression_ratio` | آخری ڈرین (0-1) کا دبے ہوئے/خارج ہونے والے تناسب ، انتباہ بجٹ کے لئے مفید ہے۔ |
| `soranet_privacy_last_poll_unixtime` | آخری کامیاب سروے کا UNIX ٹائم اسٹیمپ (کلکٹر آئیڈل الرٹ کو کھانا کھلاتا ہے)۔ |
| `soranet_privacy_collector_enabled` | جب رازداری جمع کرنے والا غیر فعال ہوجاتا ہے یا شروع کرنے میں ناکام ہوجاتا ہے تو `0` پر گر جاتا ہے (کلکٹر سے متاثرہ الرٹ کو ایندھن دیتا ہے)۔ |
| `soranet_privacy_poll_errors_total{provider}` | پولنگ کی ناکامیوں نے ریلے عرفی ناموں کے ذریعہ گروپ کیا (ڈیکوڈ کی غلطیوں ، ایچ ٹی ٹی پی کی ناکامیوں یا غیر متوقع کوڈز کے ذریعہ اضافہ ہوا)۔ |ڈیش بورڈز کو برقرار رکھنے کے ساتھ مشاہدات کے بغیر بالٹیاں خاموش رہیں
زیرو کے ساتھ ونڈوز بنائے بغیر صاف کریں۔

## آپریشنل گائیڈ

1. ** ڈیش بورڈز ** - `mode` کے ذریعہ مذکورہ بالا میٹرکس کو گروپ کیا گیا ہے اور
   `window_start`۔ جمع کرنے والوں میں مسائل کو ظاہر کرنے کے لئے گمشدہ ونڈوز کو نمایاں کرتا ہے
   یا ریلے تمیز کرنے کے لئے `soranet_privacy_suppression_total{reason}` استعمال کریں
   کلیکٹر کے ذریعہ دبانے والے شراکت کاروں کی کمی۔ کا اثاثہ
   Grafana میں اب ایک سرشار پینل شامل ہے ** "دبانے کی وجوہات (5 میٹر)" **
   ان کاؤنٹرز کے علاوہ ایک ** "دبے ہوئے بالٹی ٪" ** اسٹیٹ کے ذریعہ تقویت یافتہ
   انتخاب کے ذریعہ `sum(soranet_privacy_bucket_suppressed) / count(...)` کا حساب لگاتا ہے
   لہذا آپریٹرز ایک نظر میں بجٹ کے فرق کو دیکھتے ہیں۔ سیریز
   ** کلکٹر شیئر بیک بلاگ ** (`soranet_privacy_pending_collectors`) اور اسٹیٹ
   ** اسنیپ شاٹ دبانے کا تناسب ** جھلکیاں پھنسے ہوئے جمع کرنے والوں اور بجٹ بڑھے
   خودکار رنز کے دوران۔
2. ** انتباہ ** - رازداری سے محفوظ کاؤنٹرز سے الارم کو متحرک کرتا ہے: مسترد چوٹیوں
   POW ، Coldown فریکوئینسی ، RTT بڑھنے اور صلاحیت کو مسترد کرتا ہے۔ جیسا کہ
   کاؤنٹر ہر بالٹی ، شرح پر مبنی قواعد کے اندر اجارہ دار ہوتے ہیں
   وہ اچھی طرح سے کام کرتے ہیں۔
3. ** واقعہ کا جواب ** - پہلے اعداد و شمار پر انحصار کرتا ہے۔ جب ضرورت ہو
   گہری ڈیبگنگ ، ریلے سے بالٹیوں کے اسنیپ شاٹس کھیلنے کو کہیں یا
   ٹریفک لاگز جمع کرنے کے بجائے اندھے پیمائش کے ثبوتوں کا معائنہ کریں
   کچا
4
   `max_completed_buckets`۔ برآمد کنندگان کو آؤٹ پٹ Prometheus AS کا علاج کرنا چاہئے
   کیننیکل ماخذ اور مقامی بالٹیوں کو ایک بار بھیج دیا گیا۔

## حذف تجزیات اور خودکار پھانسی

SNNET-8 قبولیت کا انحصار یہ ظاہر کرنے پر ہے کہ خودکار جمع کرنے والے ہیں
صحت مند رہیں اور یہ دباؤ پالیسی کی حدود میں رہتا ہے
(<= کسی بھی 30 منٹ کی ونڈو میں فی ریلے میں بالٹیاں کا 10 ٪)۔ ٹولنگ
ضروری پہلے ہی ریپو میں شامل ہے۔ آپریٹرز کو اسے ان میں ضم کرنا ہوگا
ہفتہ وار رسومات۔ Grafana پر نئے خالی پینل کی عکاسی ہوتی ہے
اس سے پہلے آن کال ٹیموں کو براہ راست مرئیت دیتے ہوئے ذیل میں پروم کیو ایل کے ٹکڑوں
دستی سوالات کا سہارا۔

### دباؤ کا جائزہ لینے کے لئے پروم کیو ایل کی ترکیبیں

آپریٹرز کے پاس مندرجہ ذیل پروم کیو ایل مددگار ہونا چاہئے۔ دونوں
مشترکہ ڈیش بورڈ (`dashboards/grafana/soranet_privacy_metrics.json`) میں حوالہ دیا گیا
اور الرٹ مینجر قواعد میں:

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

اس بات کی تصدیق کرنے کے لئے تناسب کا استعمال کریں کہ ** "دبے ہوئے بالٹی ٪" ** اسٹیٹ کو برقرار رکھا گیا ہے
پالیسی بجٹ کے نیچے ؛ اسپائک ڈٹیکٹر کو مربوط کریں
جب ٹیکس دہندگان کی گنتی ذیل میں آتی ہے تو فوری تاثرات کے لئے الرٹ مینجر
غیر متوقع طریقہ

### بالٹی آف لائن رپورٹنگ سی ایل آئی

ورک اسپیس نے NDJSON ٹریپس کے لئے `cargo xtask soranet-privacy-report` کو بے نقاب کیا
وقت کی پابندی. اسے ریلے کی ایک یا زیادہ ایڈمن برآمدات کی طرف اشارہ کریں:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```مددگار `SoranetSecureAggregator` کے ساتھ گرفتاری پر کارروائی کرتا ہے ، اس کا خلاصہ پرنٹ کرتا ہے
stdout پر حذف کریں اور اختیاری طور پر ایک ساختی JSON رپورٹ کے ذریعے لکھیں
`--json-out <path|->`۔ براہ راست کلکٹر کی طرح اسی نوبس کا احترام کریں
(`--bucket-secs` ، `--min-contributors` ، `--expected-shares` ، وغیرہ) ، اجازت دینا
جب کسی مسئلے کی تحقیقات کرتے ہو تو مختلف دہلیز کے تحت تاریخی گرفتاری کھیلیں۔
Grafana کے اسکرین شاٹس کے ساتھ JSON کو منسلک کریں تاکہ تجزیات گیٹ
SNNET-8 قابل اظہار ہے۔

### خودکار پہلی رن چیک لسٹ

گورننس کے لئے اب بھی اس بات کا ثبوت درکار ہے کہ پہلی خودکار عملدرآمد نے اس سے ملاقات کی
دبانے والا بجٹ۔ مددگار اب `--max-suppression-ratio <0-1>` کو قبول کرتا ہے
تاکہ سی آئی ایس یا آپریٹرز تیزی سے ناکام ہوجائیں جب حذف شدہ بالٹیاں اس سے زیادہ ہوجائیں
اجازت ونڈو (پہلے سے طے شدہ 10 ٪) یا جب ابھی تک کوئی بالٹیاں موجود نہیں ہیں۔ بہاؤ
سفارش کی:

1. ریلے اور اسٹریم کے ایڈمن اختتامی نقطہ سے Ndjson برآمد کریں
   `/v1/soranet/privacy/event|share` آرکسٹریٹر سے
   `artifacts/sorafs_privacy/<relay>.ndjson`۔
2. پالیسی بجٹ کے ساتھ مددگار چلائیں:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   کمانڈ مشاہدہ شدہ تناسب پرنٹ کرتا ہے اور صفر کے علاوہ کسی دوسرے کوڈ کے ساتھ باہر نکلتا ہے
   جب بجٹ سے تجاوز کیا جاتا ہے ** یا ** جب بالٹیاں تیار نہیں ہوتی ہیں تو اس کی نشاندہی ہوتی ہے
   یہ ٹیلی میٹری ابھی تک پھانسی کے لئے تیار نہیں کی گئی ہے۔ پیمائش میں
   براہ راست `soranet_privacy_pending_collectors` صفر اور کی طرف ڈریننگ دکھانا چاہئے
   `soranet_privacy_snapshot_suppression_ratio` اسی کے تحت رہنا
   بجٹ جبکہ رن پر عمل درآمد ہوتا ہے۔
3. SNNET-8 ثبوت کے بنڈل کے ساتھ آؤٹ پٹ JSON اور CLI لاگ کو محفوظ شدہ دستاویزات
   پہلے سے طے شدہ ٹرانسپورٹ کو تبدیل کرنے سے پہلے تاکہ جائزہ لینے والے کر سکیں
   عین مطابق نمونے کو دوبارہ پیش کریں۔

## اگلے اقدامات (Snnet-8a)

- دوہری پریو جمع کرنے والوں کو مربوط کریں ، رن ٹائم سے حصص کی تعلیم کو مربوط کریں
  پے لوڈ کو خارج کرنے کے لئے ریلے اور جمع کرنے والوں کے لئے `SoranetPrivacyBucketMetricsV1`
  مستقل. *(ہو گیا - `ingest_privacy_payload` IN دیکھیں
  `crates/sorafs_orchestrator/src/lib.rs` اور اس سے وابستہ ٹیسٹ۔)*
- مشترکہ Prometheus ڈیش بورڈ اور الرٹ قواعد شائع کریں جو احاطہ کرتے ہیں
  دبانے والے فرق ، کلکٹر صحت اور گمنامی براؤن آؤٹ۔ *(ہو گیا - دیکھیں
  `dashboards/grafana/soranet_privacy_metrics.json` ،
  `dashboards/alerts/soranet_privacy_rules.yml` ،
  `dashboards/alerts/soranet_policy_rules.yml` ، اور توثیق فکسچر۔)*
- بیان کردہ تفریق پرائیویسی انشانکن نمونے تیار کریں
  `privacy_metrics_dp.md` ، بشمول تولیدی نوٹ بک اور ہضم
  گورننس *(ہو گیا - نوٹ بک + نمونے کے ذریعہ تیار کردہ
  `scripts/telemetry/run_privacy_dp.py` ؛ CI ریپر
  `scripts/telemetry/run_privacy_dp_notebook.sh` نوٹ بک کے ذریعے چلاتا ہے
  ورک فلو `.github/workflows/release-pipeline.yml` ؛ گورننس ڈائجسٹ میں محفوظ شدہ دستاویزات
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

موجودہ ورژن SNNET-8 بیس کو فراہم کرتا ہے: عین مطابق اور محفوظ ٹیلی میٹری
رازداری کے لئے جو کھرچنی اور ڈیش بورڈز Prometheus کے ساتھ براہ راست مربوط ہوتا ہے
موجودہ تفریق رازداری کے انشانکن نمونے تیار ہیں ،
ریلیز ورک فلو نوٹ بک کی آؤٹ پٹ کو تازہ رکھتا ہے ، اور کام کرتا ہے
باقی فوکس پہلے خودکار عملدرآمد کی نگرانی اور توسیع پر مرکوز ہے
دبانے والے انتباہات کا تجزیہ۔