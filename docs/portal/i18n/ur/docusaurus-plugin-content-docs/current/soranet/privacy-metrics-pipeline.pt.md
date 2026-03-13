---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پرائیویسی میٹرکس پائپ لائن
عنوان: سورانیٹ پرائیویسی میٹرکس پائپ لائن (SNNET-8)
سائڈبار_لیبل: پرائیویسی میٹرکس پائپ لائن
تفصیل: سورانیٹ ریلے اور آرکسٹریٹرز کے لئے پرائیویسی کو محفوظ رکھنے والے ٹیلی میٹری کا مجموعہ۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

# سورانیٹ پرائیویسی میٹرکس پائپ لائن

SNNET-8 نے پرائیویسی سے واقف ٹیلی میٹری کی سطح کو ریلے رن ٹائم سے متعارف کرایا ہے۔ ریلے اب ہینڈ شیک اور سرکٹ کے واقعات کو ایک منٹ کی بالٹیوں میں جمع کرتا ہے اور صرف موٹے Prometheus کاؤنٹرز کو برآمد کرتا ہے ، اور انفرادی سرکٹس کو ڈوپل کرتے رہتے ہیں جبکہ آپریٹرز کو قابل عمل مرئیت فراہم کرتے ہیں۔

## اجتماعی جائزہ

- رن ٹائم نفاذ `tools/soranet-relay/src/privacy.rs` میں `PrivacyAggregator` کے بطور ہے۔
- بالٹیاں فی گھڑی منٹ (`bucket_secs` ، پہلے سے طے شدہ 60 سیکنڈ) کو تبدیل کردیئے جاتے ہیں اور ایک محدود رنگ میں (`max_completed_buckets` ، پہلے سے طے شدہ 120) میں محفوظ ہیں۔ کلکٹر شیئرز اپنا محدود بیک بلاگ (`max_share_lag_buckets` ، پہلے سے طے شدہ 12) برقرار رکھتے ہیں تاکہ پرانی پریو ونڈوز کو میموری کو لیک کرنے یا پھنسے ہوئے جمع کرنے والوں کو نقاب پوش کرنے کی بجائے دبے ہوئے بالٹیوں کے طور پر خالی کردیا جائے۔
- `RelayConfig::privacy` براہ راست `PrivacyConfig` پر نقشہ بناتا ہے ، ایڈجسٹمنٹ KNOBS (`bucket_secs` ، `min_handshakes` ، `flush_delay_buckets` ، `force_flush_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، `flush_delay_buckets` `expected_shares`)۔ پروڈکشن رن ٹائم ڈیفالٹس کو برقرار رکھتا ہے جبکہ SNNET-8A محفوظ جمع کرنے کی دہلیز متعارف کراتا ہے۔
- رن ٹائم ماڈیول ٹائپڈ مددگاروں کے توسط سے واقعات کو رجسٹر کرتے ہیں: `record_circuit_accepted` ، `record_circuit_rejected` ، `record_throttle` ، `record_throttle_cooldown` ، `record_capacity_reject` ، `record_active_sample` ، `record_active_sample` ، Prometheus۔

## ریلے ایڈمن اینڈ پوائنٹ

آپریٹرز `GET /privacy/events` کے ذریعے خام مشاہدات کے لئے ریلے ایڈمن سننے والوں سے استفسار کرسکتے ہیں۔ اختتامی نقطہ نئے لائن سے طے شدہ JSON (`application/x-ndjson`) پر مشتمل ہے جس میں `SoranetPrivacyEventV1` پے لوڈز شامل ہیں جو داخلی `PrivacyEventBuffer` سے آئینہ دار ہیں۔ بفر `privacy.event_buffer_capacity` اندراجات (پہلے سے طے شدہ 4096) تک کے تازہ ترین واقعات کو اسٹور کرتا ہے اور پڑھنے پر سوھا جاتا ہے ، لہذا خامیوں سے بچنے کے لئے کھرچنیوں کو کثرت سے کافی حد تک رائے شماری کرنی ہوگی۔ واقعات میں ایک ہی مصافحہ ، تھروٹل ، تصدیق شدہ بینڈوتھ ، ایکٹو سرکٹ ، اور GAR سگنل شامل ہیں جو Prometheus کاؤنٹرز کو کھانا کھاتے ہیں ، جس سے بہاو جمع کرنے والوں کو رازداری سے محفوظ بریڈ کرمبس فائل کرنے یا محفوظ اجتماعی ورک فلوز کو کھانا کھلانا پڑتا ہے۔

## ریلے کنفیگریشن

آپریٹرز `privacy` سیکشن کے ذریعہ ریلے کنفیگریشن فائل میں رازداری کے ٹیلی میٹری کیڈینس کو ایڈجسٹ کرتے ہیں:

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

فیلڈ ڈیفالٹس SNNET-8 تفصیلات کے مطابق ہیں اور لوڈنگ کے بعد اس کی توثیق کی جاتی ہے:| فیلڈ | تفصیل | معیاری |
| ------- | ----------- | -------- |
| `bucket_secs` | ہر جمع ونڈو کی چوڑائی (سیکنڈ)۔ | `60` |
| `min_handshakes` | بالٹی سے پہلے شراکت کاروں کی کم سے کم تعداد کاؤنٹرز کا اخراج کرسکتی ہے۔ | `12` |
| `flush_delay_buckets` | فلش کی کوشش کرنے سے پہلے انتظار کرنے کے لئے مکمل بالٹیاں۔ | `1` |
| `force_flush_buckets` | دبے ہوئے بالٹی جاری کرنے سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` | برقرار رکھی ہوئی بالٹیوں کا بیکلاگ (لامحدود میموری کو روکتا ہے)۔ | `120` |
| `max_share_lag_buckets` | حذف کرنے سے پہلے کلکٹر کے حصص کے لئے برقرار رکھنے والی ونڈو۔ | `12` |
| `expected_shares` | مماثل سے پہلے پریو کلکٹر کے حصص کی ضرورت ہے۔ | `2` |
| `event_buffer_capacity` | ایڈجسن ایونٹ کا بیک بلاگ ایڈمن اسٹریم کے لئے۔ | `4096` |

`force_flush_buckets` `flush_delay_buckets` سے کم ترتیب دینا ، دہلیز کو دوبارہ ترتیب دینا ، یا برقرار رکھنے والے گارڈ کو غیر فعال کرنا اب تعیناتیوں کو روکنے کے لئے توثیق میں ناکام ہوجاتا ہے جو ریلے ٹیلی میٹری کو لیک کریں گے۔

`event_buffer_capacity` حد `/admin/privacy/events` کو بھی محدود کرتی ہے ، اس بات کو یقینی بناتے ہوئے کہ کھرچنے والے غیر معینہ مدت تک تاخیر نہیں کرسکتے ہیں۔

## پریو کلیکٹر کے حصص

SNNET-8A دوہری جمع کرنے والے تعینات کرتا ہے جو خفیہ شیئرنگ کے ساتھ پریو بالٹیوں کا اخراج کرتے ہیں۔ آرکیسٹریٹر اب این ڈی جےسن اسٹریم `/privacy/events` کو اندراجات `SoranetPrivacyEventV1` کے لئے پارس کرتا ہے اور `SoranetPrivacyPrioShareV1` کو شیئر کرتا ہے ، جس سے وہ `SoranetSecureAggregator::ingest_prio_share` پر بھیجتے ہیں۔ جب شراکتیں آتی ہیں تو ، ریلے کے طرز عمل کی عکس بندی کرتے ہیں۔ `SoranetPrivacyBucketMetricsV1` میں جوڑنے سے پہلے بالٹی سیدھ اور ہسٹگرام شکل کے لئے حصص کی توثیق کی جاتی ہے۔ اگر مشترکہ ہینڈ شیک کی گنتی `min_contributors` سے نیچے آجاتی ہے تو ، بالٹی کو `suppressed` کے طور پر برآمد کیا جاتا ہے ، جو ریلے میں جمع کرنے والے کے طرز عمل کی عکس بندی کرتا ہے۔ دبے ہوئے ونڈوز اب ایک `suppression_reason` لیبل خارج کرتے ہیں تاکہ آپریٹرز ٹیلی میٹری کے خلیجوں کی تشخیص کرتے وقت `insufficient_contributors` ، `collector_suppressed` ، `collector_window_elapsed` ، اور `forced_flush_window_elapsed` کے درمیان فرق کرسکیں۔ `collector_window_elapsed` شکل بھی اس وقت متحرک ہوجاتی ہے جب پریو کے حصص `max_share_lag_buckets` سے آگے جاتے ہیں ، جس سے پھنسے جمع کرنے والوں کو پرانے جمع کرنے والوں کو میموری میں چھوڑنے کے بغیر مرئی بناتا ہے۔

## Torii ingestion اختتامی نکات

Torii اب ٹیلی میٹری گیٹنگ کے ساتھ دو HTTP اختتامی نکات کو بے نقاب کرتا ہے تاکہ ریلے اور جمع کرنے والے بیسپوک ٹرانسپورٹ کو سرایت کیے بغیر مشاہدات کو آگے بڑھا سکیں:- `POST /v2/soranet/privacy/event` `RecordSoranetPrivacyEventDto` پے لوڈ کو قبول کرتا ہے۔ جسم `SoranetPrivacyEventV1` کے علاوہ ایک اختیاری `source` لیبل لپیٹتا ہے۔ Torii فعال ٹیلی میٹری پروفائل کے خلاف درخواست کی توثیق کرتا ہے ، ایونٹ کو لاگ کرتا ہے ، اور HTTP `202 Accepted` کے ساتھ جواب دیتا ہے جس کے ساتھ ساتھ Norito JSON لفافے میں حساب شدہ ونڈو (`bucket_start_unix` ، `bucket_duration_secs`) پر مشتمل ہے۔
- `POST /v2/soranet/privacy/share` `RecordSoranetPrivacyShareDto` پے لوڈ کو قبول کرتا ہے۔ جسم میں `SoranetPrivacyPrioShareV1` اور ایک اختیاری `forwarded_by` اشارہ ہے تاکہ آپریٹر جمع کرنے والے کے بہاؤ کا آڈٹ کرسکیں۔ کامیاب گذارشات HTTP `202 Accepted` کو Norito JSON لفافے کے ساتھ لوٹائیں ، جمع کرنے والے ، بالٹی ونڈو ، اور دبانے کا اشارہ۔ توثیق کی ناکامیوں کا نقشہ ایک `Conversion` ٹیلی میٹری کے ردعمل کا نقشہ جمع کرنے والوں کے مابین عین مطابق غلطی سے نمٹنے کے لئے۔ آرکسٹریٹر ایونٹ لوپ اب ریلے کو پولنگ کرتے وقت ان حصص کو خارج کرتا ہے ، جس میں Torii PRIO Acculator کو ریلے میں بالٹیوں کے ساتھ ہم آہنگ رکھا جاتا ہے۔

دونوں نکات ٹیلی میٹری پروفائل کا احترام کرتے ہیں: جب میٹرکس غیر فعال ہوجاتے ہیں تو وہ `503 Service Unavailable` خارج کرتے ہیں۔ کلائنٹ Norito بائنری (`application/x.norito`) یا Norito JSON (`application/x.norito+json`) لاش بھیج سکتے ہیں۔ سرور خود بخود معیاری Torii ایکسٹریکٹرز کے ذریعہ فارمیٹ پر بات چیت کرتا ہے۔

## میٹرکس Prometheus

ہر برآمد شدہ بالٹی میں لیبل `mode` (`entry` ، `middle` ، `exit`) اور `bucket_start` لے جاتا ہے۔ مندرجہ ذیل میٹرک خاندان آؤٹ پٹ ہیں:

| میٹرک | تفصیل |
| -------- | --------------- |
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` کے ساتھ ہینڈ شیک درجہ بندی۔ |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` کے ساتھ تھروٹل کاؤنٹرز۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | تھروٹنڈ ہینڈ ہیکس کے ذریعہ مجموعی طور پر کوولڈاؤن کے دورانیے۔ |
| `soranet_privacy_verified_bytes_total` | بلائنڈ پیمائش ٹیسٹ سے تصدیق شدہ بینڈوتھ۔ |
| `soranet_privacy_active_circuits_{avg,max}` | اوسط اور چوٹی کے فعال سرکٹس فی بالٹی۔ |
| `soranet_privacy_rtt_millis{percentile}` | آر ٹی ٹی پرسنٹائل تخمینہ (`p50` ، `p90` ، `p99`)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | گورننس ایکشن کی رپورٹ کے کاؤنٹرز کیٹیگری ڈائجسٹ کے ذریعہ |
| `soranet_privacy_bucket_suppressed` | بالٹیوں کا انعقاد اس لئے ہے کہ شراکت دار کی دہلیز کو پورا نہیں کیا گیا تھا۔ |
| `soranet_privacy_pending_collectors{mode}` | جمع کرنے والا جمع کرنے والے جمع کرنے والوں کو مشترکہ طور پر مشترکہ طور پر ، ریلے وضع کے ذریعہ گروپ کیا گیا ہے۔ |
| `soranet_privacy_suppression_total{reason}` | رازداری کے فرق کو منسوب کرنے کے لئے ڈیش بورڈز کے لئے `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` کے ساتھ دبے ہوئے بالٹی کاؤنٹرز۔ |
| `soranet_privacy_snapshot_suppression_ratio` | آخری ڈرین (0-1) کا دبے ہوئے/نالے کا تناسب ، جو انتباہ بجٹ کے لئے مفید ہے۔ |
| `soranet_privacy_last_poll_unixtime` | آخری کامیاب سروے کا UNIX ٹائم اسٹیمپ (کلکٹر آئیڈل الرٹ کو کھانا کھلاتا ہے)۔ |
| `soranet_privacy_collector_enabled` | گیج جو `0` بن جاتا ہے جب رازداری جمع کرنے والا غیر فعال ہوتا ہے یا شروع کرنے میں ناکام ہوجاتا ہے (کلکٹر سے معذور انتباہ کو کھانا کھلاتا ہے)۔ |
| `soranet_privacy_poll_errors_total{provider}` | پولنگ کی ناکامیوں نے ریلے عرفی ناموں کے ذریعہ گروپ کیا (ڈیکوڈ کی غلطیوں ، ایچ ٹی ٹی پی کی ناکامیوں ، یا غیر متوقع اسٹیٹس کوڈز میں اضافہ)۔ |مشاہدات کے بغیر بالٹیاں خاموش رہتی ہیں ، بغیر صفر ونڈوز بنائے ڈیش بورڈز کو صاف رکھتے ہیں۔

## آپریشنل رہنمائی

1. ** ڈیش بورڈز ** - `mode` اور `window_start` کے ذریعہ اوپر کی پیمائش کو پلاٹ کریں۔ کلیکٹر یا ریلے کے مسائل کو ظاہر کرنے کے لئے گمشدہ ونڈوز کو اجاگر کریں۔ خلاء کی اسکریننگ کرتے وقت کلیکٹر سے چلنے والے حذفوں سے گمشدہ شراکت کاروں کو ممتاز کرنے کے لئے `soranet_privacy_suppression_total{reason}` کا استعمال کریں۔ Grafana اثاثہ اب ایک سرشار ** "دبانے کی وجوہات (5m)" ** ان کاؤنٹرز کے ذریعہ چلنے والا پینل بھیجتا ہے اور اس کے علاوہ ایک ** "دبے ہوئے بالٹی ٪" ** اسٹیٹ جو `sum(soranet_privacy_bucket_suppressed) / count(...)` ہر انتخاب کا حساب لگاتا ہے لہذا آپریٹر بجٹ کی خلاف ورزیوں کو تیزی سے دیکھ سکتے ہیں۔ ** "کلکٹر شیئر بیک بلاگ" ** سیریز (`soranet_privacy_pending_collectors`) اور ** "اسنیپ شاٹ دبانے کا تناسب" ** اسٹیٹ اسٹیٹ نے خودکار پھانسیوں کے دوران پھنسے ہوئے جمع کرنے والوں اور بجٹ کے انحراف کو نمایاں کیا۔
2. ** انتباہ ** - پرائیویسی سیکور کاؤنٹرز سے الارم چلائیں: POW مسترد ، کولڈاؤن فریکوئنسی ، RTT بڑھنے اور صلاحیت کو مسترد کرنے میں اسپائکس۔ کیونکہ کاؤنٹر ہر بالٹی کے اندر نیرس ہوتے ہیں ، لہذا شرح پر مبنی آسان قواعد اچھی طرح سے کام کرتے ہیں۔
3. جب گہری ڈیبگنگ کی ضرورت ہو تو ، ریلے سے بالٹی اسنیپ شاٹس کو دوبارہ چلانے کے لئے کہیں یا خام ٹریفک لاگز جمع کرنے کے بجائے بلائنڈ پیمائش کے ثبوت کا معائنہ کریں۔
4. ** برقرار رکھنا ** - کھرچنی چاقو کثرت سے کافی حد تک `max_completed_buckets` سے زیادہ سے زیادہ سے زیادہ ہے۔ برآمد کنندگان کو Prometheus آؤٹ پٹ کو بطور کیننیکل ماخذ سمجھنا چاہئے اور آگے بڑھنے کے بعد مقامی بالٹیوں کو ضائع کرنا چاہئے۔

## دبانے کا تجزیہ اور خودکار پھانسی

SNNET-8 کی قبولیت کا انحصار یہ ظاہر کرنے پر ہے کہ خودکار جمع کرنے والے صحت مند رہتے ہیں اور یہ دباؤ پالیسی کی حدود میں رہتا ہے (<= 10 ٪ بالٹیوں کا فی ریلے کسی بھی 30 منٹ کی ونڈو میں)۔ اب اس گیٹ کو پورا کرنے کے لئے ضروری ٹولنگ ذخیرہ کے ساتھ آتی ہے۔ آپریٹرز کو اسے اپنی ہفتہ وار رسومات میں ضم کرنا چاہئے۔ نئے Grafana دبانے والے پینل ذیل میں پرومق ایل کے ٹکڑوں کی عکاسی کرتے ہیں ، جس سے تعیناتی ٹیموں کو دستی سوالات کا سہارا لینے کی ضرورت سے پہلے براہ راست مرئیت مل جاتی ہے۔

### دبانے کے جائزے کے لئے پروم کیو ایل کی ترکیبیں

آپریٹرز کو مندرجہ ذیل پروم کیو ایل مددگاروں کو ہاتھ میں رکھنا چاہئے۔ دونوں کا مشترکہ ڈیش بورڈ Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) اور الرٹ مینجر قواعد میں حوالہ دیا گیا ہے:

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

اس بات کی تصدیق کرنے کے لئے تناسب کی پیداوار کا استعمال کریں کہ ** "دبے ہوئے بالٹی ٪" ** اسٹیٹ پالیسی بجٹ سے نیچے ہے۔ جب شراکت دار کی گنتی غیر متوقع طور پر گر جاتی ہے تو فوری تاثرات کے لئے اسپائک ڈٹیکٹر کو الرٹ مینجر سے مربوط کریں۔

### آف لائن بالٹی رپورٹنگ سی ایل آئی

ورک اسپیس نے وقتی طور پر NDJSON کیپچر کے لئے `cargo xtask soranet-privacy-report` کو بے نقاب کیا۔ ایک یا زیادہ ریلے ایڈمن برآمدات کی طرف اشارہ کریں:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```مددگار `SoranetSecureAggregator` کے ذریعے گرفتاری سے گزرتا ہے ، STDOUT پر ایک دبانے کا خلاصہ پرنٹ کرتا ہے ، اور اختیاری طور پر `--json-out <path|->` کے ذریعہ ایک ساختہ JSON رپورٹ لکھتا ہے۔ یہ براہ راست کلکٹر (`--bucket-secs` ، `--min-contributors` ، `--expected-shares` ، وغیرہ) کی طرح ہی نوبس کا اعزاز دیتا ہے ، جس سے آپریٹرز کو کسی واقعے کا سامنا کرتے وقت مختلف دہلیز کے تحت تاریخی گرفتاریوں کو دوبارہ چلانے کی اجازت ملتی ہے۔ Grafana کی گرفتاری کے ساتھ JSON کو منسلک کریں تاکہ SNNET-8 دبانے والے تجزیہ گیٹ کے قابل عمل رہیں۔

### پہلی خودکار عملدرآمد کے لئے چیک لسٹ

گورننس کو اب بھی یہ ثابت کرنے کی ضرورت ہے کہ پہلی خودکار عملدرآمد نے دبانے والے بجٹ کو پورا کیا۔ مددگار اب `--max-suppression-ratio <0-1>` کو قبول کرتا ہے تاکہ CI یا آپریٹرز تیزی سے ناکام ہوجائیں جب دبے ہوئے بالٹیوں کو اجازت دی گئی ونڈو (پہلے سے طے شدہ 10 ٪) سے تجاوز کریں یا جب ابھی تک بالٹیاں نہ ہوں۔ تجویز کردہ بہاؤ:

1. ریلے ایڈمن اینڈ پوائنٹس کے علاوہ آرکسٹریٹر اسٹریم `/v2/soranet/privacy/event|share` سے `artifacts/sorafs_privacy/<relay>.ndjson` سے NDJSON برآمد کریں۔
2. پالیسی بجٹ کے ساتھ مددگار چلائیں:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   کمانڈ مشاہدہ شدہ تناسب پرنٹ کرتا ہے اور غیر صفر کوڈ کے ساتھ باہر نکلتا ہے جب بجٹ سے تجاوز کیا جاتا ہے ** یا ** جب ابھی تک بالٹیاں تیار نہیں ہوتی ہیں ، اس بات کا اشارہ کرتے ہیں کہ ٹیلی میٹری ابھی تک عملدرآمد کے لئے تیار نہیں کی گئی ہے۔ براہ راست میٹرکس کو `soranet_privacy_pending_collectors` صفر اور `soranet_privacy_snapshot_suppression_ratio` کو اسی بجٹ سے نیچے گرتے ہوئے دکھایا جانا چاہئے جیسے رن ہوتا ہے۔
3۔ ٹرانسپورٹ ڈیفالٹ کو تبدیل کرنے سے پہلے JSON آؤٹ پٹ اور سی ایل آئی لاگ ان SNET-8 ثبوت پیکیج کے ساتھ آرکائیو کریں تاکہ جائزہ لینے والے عین مطابق نمونے کو دوبارہ پیش کرسکیں۔

## اگلے اقدامات (Snnet-8a)

- دوہری پریو جمع کرنے والوں کو مربوط کریں ، رن ٹائم سے حصص کو جوڑیں تاکہ ریلے اور جمع کرنے والے مستقل `SoranetPrivacyBucketMetricsV1` پے لوڈ جاری کریں۔ *(مکمل - `crates/sorafs_orchestrator/src/lib.rs` اور اس سے وابستہ ٹیسٹ میں `ingest_privacy_payload` دیکھیں۔)*
- مشترکہ Prometheus ڈیش بورڈ اور انتباہی قواعد شائع کریں جس میں دبانے والے فرقوں ، کلکٹر کی صحت اور گمنامی کے قطرے شامل ہیں۔ *(مکمل - `dashboards/grafana/soranet_privacy_metrics.json` ، `dashboards/alerts/soranet_privacy_rules.yml` ، `dashboards/alerts/soranet_policy_rules.yml` اور توثیق فکسچر دیکھیں۔)*
- `privacy_metrics_dp.md` میں بیان کردہ تفریق پرائیویسی انشانکن نمونے تیار کریں ، جس میں تولیدی نوٹ بک اور گورننس ڈائجسٹ شامل ہیں۔ *(مکمل - `scripts/telemetry/run_privacy_dp.py` کے ذریعہ تیار کردہ نوٹ بک اور نمونے C CI ریپر `scripts/telemetry/run_privacy_dp_notebook.sh` ورک فلو `.github/workflows/release-pipeline.yml` کے ذریعے نوٹ بک چلاتا ہے I `docs/source/status/soranet_privacy_dp_digest.md` میں آرکائویٹڈ گورننس ڈائجسٹ۔)

موجودہ ریلیز میں SNNET-8 کی بنیاد فراہم کی گئی ہے: عین مطابق ، رازداری کی سیکر ٹیلی میٹری جو موجودہ Prometheus سکریپرس اور ڈیش بورڈز میں براہ راست فٹ بیٹھتی ہے۔ تفریق پرائیویسی انشانکن نمونے کی جگہ موجود ہے ، ریلیز پائپ لائن ورک فلو نوٹ بک کے آؤٹ پٹس کو تازہ ترین رکھتا ہے ، اور باقی کام پہلے خودکار رن کی نگرانی اور دبانے والے انتباہ تجزیوں کو بڑھانے پر مرکوز ہے۔