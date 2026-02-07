---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پرائیویسی میٹرکس پائپ لائن
عنوان: سورانیٹ پرائیویسی میٹرکس ٹریک (SNNET-8)
سائڈبار_لیبل: رازداری کی پیمائش کا راستہ
تفصیل: سورانیٹ ریلے اور آرکسٹریٹرز کے لئے پرائیویسی کو محفوظ رکھنے والے ٹیلی میٹری کا مجموعہ۔
---

::: نوٹ معیاری ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` کی عکاسی کریں۔ دستاویزات کا پرانا سیٹ ریٹائر ہونے تک دونوں کاپیاں ایک جیسے رکھیں۔
:::

# سورانیٹ میں رازداری کی پیمائش کو ٹریک کریں

SNNET-8 ریلے آپریٹنگ ماحول کے لئے رازداری سے واقف ٹیلی میٹری کی سطح فراہم کرتا ہے۔ ریلے اب مصافحہ اور سرکٹ کے واقعات کو منٹ کے سائز کی بالٹیوں میں جمع کرتا ہے اور صرف موٹے Prometheus کاؤنٹرز کا اخراج کرتا ہے ، اور آپریٹرز کو قابل عمل مرئیت دیتے ہوئے انفرادی سرکٹس کو غیر منظم رکھتا ہے۔

## پیچیدہ جائزہ

- آپریٹنگ ماحول کا نفاذ `tools/soranet-relay/src/privacy.rs` پر `PrivacyAggregator` کے تحت واقع ہے۔
- بالٹیوں کو دیوار کے وقت (`bucket_secs` ، پہلے سے طے شدہ 60 سیکنڈ) کے منٹ میں ترتیب دیا جاتا ہے اور ایک محدود لوپ (`max_completed_buckets` ، پہلے سے طے شدہ 120) میں محفوظ کیا جاتا ہے۔ جمع کرنے والوں کے حصص محدود تاخیر (`max_share_lag_buckets` ، پہلے سے طے شدہ 12) رکھتے ہیں تاکہ پریو کی پریو ونڈوز میموری کو لیک کرنے یا پھنسے ہوئے جمع کرنے والوں کو چھپانے کے بجائے دبے ہوئے بالٹیوں کے طور پر اتاریں۔
- `RelayConfig::privacy` براہ راست `PrivacyConfig` سے میل کھاتا ہے اور ترتیب دینے والی چابیاں (`bucket_secs` ، Prometheus ، `force_flush_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، پیداواری ماحول پہلے سے طے شدہ اقدار کو برقرار رکھتا ہے جبکہ SNNET-8A محفوظ تالیف کی حد متعارف کراتا ہے۔
- رن ٹائم ماڈیولز مددگاروں کے ذریعہ لاگ ان واقعات جیسے: `record_circuit_accepted` ، `record_circuit_rejected` ، `record_throttle` ، `record_throttle_cooldown` ، `record_capacity_reject` ، `record_active_sample` ، `record_active_sample` ، اور `record_verified_bytes` ، اور `record_active_sample` ، اور `record_active_sample` ، اور `record_active_sample`۔

## ریلے کے لئے ایڈمن پوائنٹ

آپریٹرز `GET /privacy/events` کے ذریعے خام آراء کے لئے ریلے ایڈمن سننے والوں کو پول کرسکتے ہیں۔ نقطہ JSON لائن ڈیلیمیٹر (`application/x-ndjson`) واپس کرتا ہے جس میں پے لوڈ `SoranetPrivacyEventV1` پر مشتمل ہوتا ہے جس کی عکاسی اندرونی `PrivacyEventBuffer` سے ہوتی ہے۔ اسٹور میں تازہ ترین واقعات `privacy.event_buffer_capacity` انٹری (پہلے سے طے شدہ 4096) تک ہیں اور اسے پڑھنے پر خالی کردیا جاتا ہے ، لہذا کھرچنے والوں کو خلاء سے بچنے کے لئے کافی رائے شماری کرنی ہوگی۔ واقعات میں ایک ہی مصافحہ ، تھروٹل ، تصدیق شدہ بینڈوتھ ، ایکٹو سرکٹ ، اور GAR اشاروں کا احاطہ کیا گیا ہے جو Prometheus کاؤنٹرز کو کھانا کھلاتے ہیں ، جس سے بہاو جمع کرنے والوں کو رازداری سے محفوظ بریڈ کرمبس کو محفوظ کرنے یا ایک محفوظ اسمبلی ورک فلو کو کھانا کھلانے کی اجازت ملتی ہے۔

## ریلے کی ترتیبات

آپریٹرز سیکشن `privacy` کے ذریعے ریلے کنفیگریشن فائل میں رازداری کے ٹیلی میٹری کی تال طے کرتے ہیں:

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

فیلڈز کی پہلے سے طے شدہ اقدار SNNET-8 کی تصریح سے ملتے ہیں اور بوجھ پر اس کی تصدیق کی جاتی ہے۔

| فیلڈ | تفصیل | ڈیفالٹ |
| ------- | ------- | ----------- |
| `bucket_secs` | ہر مجموعہ ونڈو کی چوڑائی (سیکنڈ) | `60` |
| `min_handshakes` | بالٹی کے کاؤنٹرز کو متحرک کرنے سے پہلے کم سے کم شراکت کاروں کی تعداد۔ | `12` |
| `flush_delay_buckets` | مکمل بالٹیاں کی تعداد جس کا ہم خالی کرنے کی کوشش سے پہلے انتظار کرتے ہیں۔ | `1` |
| `force_flush_buckets` | بالٹی کو دبانے سے پہلے زیادہ سے زیادہ عمر جاری ہونے سے پہلے۔ | `6` |
| `max_completed_buckets` | محفوظ بالٹیاں دیر سے ہیں (لامحدود میموری کو روکتی ہیں)۔ | `120` |
| `max_share_lag_buckets` | دبانے سے پہلے کلکٹر کے حصص کے لئے برقرار رکھنے والی ونڈو۔ | `12` |
| `expected_shares` | ضم ہونے سے پہلے درکار پریو کلکٹر کے حصص کی تعداد۔ | `2` |
| `event_buffer_capacity` | ایڈجسن ایونٹ ایڈمن فلو کے بقایاجات۔ | `4096` |`force_flush_buckets` کو `flush_delay_buckets` سے کم ترتیب دینا ، دہلیز کو صفر کرنا ، یا برقرار رکھنے والے گارڈ کو غیر فعال کرنا اب چیک میں ناکام ہوجاتا ہے تاکہ وہ تعیناتیوں سے بچ سکیں جو فی ریلے ٹیلی میٹری کو لیک کرسکتے ہیں۔

`event_buffer_capacity` حد `/admin/privacy/events` کو بھی محدود کرتی ہے ، اس بات کو یقینی بناتے ہوئے کہ کھرچنے والے کھرچنے سے لامتناہی تاخیر نہیں ہوسکتی ہے۔

## پریو کلیکٹر کے حصص

SNNET-8A دوہری جمع کرنے والے تعینات کرتا ہے جو پریو بالٹیاں خفیہ شیئرنگ کے ساتھ جاری کرتے ہیں۔ آرکسٹیٹر اب `/privacy/events` NDJSON اسٹریم کے لئے اندراجات `SoranetPrivacyEventV1` اور `SoranetPrivacyPrioShareV1` کو شیئر کرتا ہے ، اور انہیں `SoranetSecureAggregator::ingest_prio_share` میں منتقل کرتا ہے۔ جیسے ہی `PrivacyBucketConfig::expected_shares` شراکتیں آرہی ہیں ، ریلے کے طرز عمل کی عکس بندی کرتے ہوئے۔ `SoranetPrivacyBucketMetricsV1` میں ضم کرنے سے پہلے بالٹی اور ہسٹوگرام سیدھ کے لئے حصص کی جانچ پڑتال کی جاتی ہے۔ اگر بلٹ ان ہینڈ شیک نمبر `min_contributors` کے نیچے آتا ہے تو ، بالٹی کو `suppressed` کے طور پر برآمد کیا جاتا ہے تاکہ ریلے کے اندر کلکٹر کے طرز عمل کی عکاسی کی جاسکے۔ دبے ہوئے ونڈوز کو اب `suppression_reason` کا لیبل لگا دیا گیا ہے تاکہ آپریٹرز ٹیلی میٹری کے خلیجوں کی تشخیص کرتے وقت `insufficient_contributors` ، `collector_suppressed` ، `collector_window_elapsed` ، اور `forced_flush_window_elapsed` کے درمیان فرق کرسکیں۔ `collector_window_elapsed` کی وجہ بھی اس وقت متحرک ہے جب پریو حصص `max_share_lag_buckets` سے آگے رہیں گے ، جس سے پھنسے ہوئے جمع کرنے والوں کو پرانے جمع کرنے والوں کو میموری میں چھوڑے بغیر مرئی بناتا ہے۔

## Torii انٹری پوائنٹس

Torii اب دو ٹیلی میٹری سے محفوظ HTTP نوڈس کو بے نقاب کرتا ہے تاکہ ریلے اور جمع کرنے والے کسٹم ٹرانسپورٹ کو شامل کیے بغیر نوٹ پاس کرسکیں:

- `POST /v1/soranet/privacy/event` `RecordSoranetPrivacyEventDto` پے لوڈ کو قبول کرتا ہے۔ `SoranetPrivacyEventV1` جسم اختیاری `source` لیبل کے ساتھ لپیٹتا ہے۔ Torii فعال ٹیلی میٹری فائل کے خلاف درخواست کی جانچ پڑتال کرتا ہے ، ایونٹ کو لاگ کرتا ہے ، اور HTTP `202 Accepted` کے ساتھ جواب دیتا ہے جس میں Norito JSON لفافے کے ساتھ حساب کتاب ونڈو (`bucket_start_unix` ، `bucket_duration_secs`) اور ریلے وضع پر مشتمل ہے۔
- `POST /v1/soranet/privacy/share` `RecordSoranetPrivacyShareDto` پے لوڈ کو قبول کرتا ہے۔ جسم میں `SoranetPrivacyPrioShareV1` اور ایک اختیاری ٹپ `forwarded_by` ہے تاکہ آپریٹر جمع کرنے والوں کے بہاؤ کا آڈٹ کرسکیں۔ کامیاب HTTP درخواست کرتا ہے `202 Accepted` کو Norito JSON ریپر کے ساتھ کلیکٹر ، بالٹی ونڈو ، اور دبانے کا اشارہ کا خلاصہ پیش کرتا ہے۔ جبکہ تصدیق کی ناکامیوں کو جمع کرنے والوں میں ناگزیر غلطی کو سنبھالنے کے ل I `Conversion` کے ٹیلی میٹری کے ردعمل سے منسلک کیا گیا ہے۔ آرکسٹریٹر ایونٹ لوپ اب پولنگ ریلے کے وقت ان حصص کو جاری کرتا ہے ، اور پریو کلیکٹر کو Torii پر ریلے پر بالٹیوں کے ساتھ ہم آہنگ رکھتے ہوئے۔

بڑی آنت ٹیلی میٹری فائل کا احترام کرتی ہے: جب ٹیلی میٹری کو غیر فعال کیا جاتا ہے تو `503 Service Unavailable` جاری کرتا ہے۔ کلائنٹ Norito بائنری (`application/x.norito`) یا Norito JSON (`application/x.norito+json`) آبجیکٹ بھیج سکتے ہیں ، اور سرور خود بخود معیاری Torii ایکسٹریکٹرز کے ذریعے فارمیٹ پر بات چیت کرتا ہے۔

## میٹرکس Prometheus

ہر برآمد شدہ بالٹی میں `mode` (`entry` ، `middle` ، `exit`) اور `bucket_start` ہے۔ مندرجہ ذیل معیارات کے کنبے جاری کیے گئے ہیں:| میٹرک | تفصیل |
| -------- | --------------- |
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` کے ساتھ ہینڈ شیک کی درجہ بندی۔ |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` کے ساتھ تھروٹل کاؤنٹرز۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | ہینڈ شیکوں کے ذریعہ فراہم کردہ بنڈل کوولڈون کو بڑھاوا دیا گیا۔ |
| `soranet_privacy_verified_bytes_total` | بینڈوتھ نے اندھے پیمائش کے ثبوتوں سے تصدیق کی۔ |
| `soranet_privacy_active_circuits_{avg,max}` | اوسط اور چوٹی کے فعال سرکٹس فی بالٹی۔ |
| `soranet_privacy_rtt_millis{percentile}` | RTT (`p50` ، `p90` ، `p99`) کے لئے صد فیصد کا تخمینہ)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | گورننس ایکشن رپورٹ ڈائجسٹ کے زمرے کے ذریعہ منقسم کاؤنٹرز۔ |
| `soranet_privacy_bucket_suppressed` | مسدود بالٹیاں کیونکہ شراکت دار کی دہلیز پوری نہیں کی گئی تھی۔ |
| `soranet_privacy_pending_collectors{mode}` | جمع کرنے والے کے ساتھ جمع کرنے والے کو ریلے وضع کے ذریعہ گروپ کیا گیا ہے۔ |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` کے ساتھ دبے ہوئے بالٹیوں کا کاؤنٹرز لہذا پینل رازداری کے فرق کو منسوب کرتے ہیں۔ |
| `soranet_privacy_snapshot_suppression_ratio` | تناسب کو دبانے/آخری ڈرین (0-1) پر فلٹر کیا گیا ، جو الرٹ بجٹ کے لئے مفید ہے۔ |
| `soranet_privacy_last_poll_unixtime` | آخری کامیاب سروے کا یونکس اسٹیمپ (کلکٹر آئیڈل الرٹ کو کھانا کھلاتا ہے)۔ |
| `soranet_privacy_collector_enabled` | جب پرائیویسی جمع کرنے والا کریش ہوجاتا ہے یا شروع کرنے میں ناکام ہوجاتا ہے تو گیج `0` میں تبدیل ہوتا ہے (یہ جمع کرنے والے سے معذور الرٹ کھلا دیتا ہے)۔ |
| `soranet_privacy_poll_errors_total{provider}` | سروے کی ناکامیوں کو عرف ریلے کے ذریعہ گروپ کیا جاتا ہے (غلطیوں ، ایچ ٹی ٹی پی کی ناکامیوں ، یا غیر متوقع اسٹیٹس کوڈز کی ضابطہ کشائی کے ذریعہ اضافہ ہوا ہے)۔ |

نوٹ کے بغیر بالٹیاں خاموش رہیں ، بورڈ کو مصنوعی صفر ونڈوز تیار کیے بغیر صاف رکھیں۔

## آپریٹنگ ہدایات

1. ** ڈیش بورڈز ** - `mode` اور `window_start` کے ذریعہ اوپر کی پیمائش کو پلاٹ کریں۔ کلیکٹر یا ریلے کے مسائل کو ظاہر کرنے کے ل lissed گمشدہ ونڈوز کو اجاگر کریں۔ وقفوں کو چھانٹتے وقت جمع کرنے والے سے چلنے والے دباؤ سے شراکت دار سے چلنے والے دبانے کو ممتاز کرنے کے لئے `soranet_privacy_suppression_total{reason}` کا استعمال کریں۔ اصل Grafana اب ایک سرشار ** "دبانے کی وجوہات (5 میٹر)" ** بورڈ جو ان میٹروں کو کھانا کھلاتا ہے ، نیز ایک اسٹیٹ ** "دبے ہوئے بالٹی ٪" ** کہ `sum(soranet_privacy_bucket_suppressed) / count(...)` ہر انتخاب کے لئے حساب کتاب کرتا ہے تاکہ آپریٹرز تیزی سے بجٹ میں اضافے کو تلاش کرسکیں۔ سٹرنگ ** "کلکٹر شیئر بیک بلاگ" ** (`soranet_privacy_pending_collectors`) اور اسٹیٹ ** "اسنیپ شاٹ دبانے کا تناسب" ** آٹومیشن کے دوران پھنسے ہوئے جمع کرنے والوں اور بجٹ بڑھنے کو اجاگر کریں۔
2. ** الارم ** - پرائیویسی سیکور میٹر سے الارم چلائیں: POW مسترد کرنے میں اضافے ، کولڈاؤن فریکوئنسی ، RTT بڑھنے ، اور طول و عرض مسترد۔ چونکہ کاؤنٹر ہر بالٹی کے اندر غیر سمتل ہوتے ہیں ، لہذا شرح کے آسان قواعد اچھی طرح سے کام کرتے ہیں۔
3. ** واقعہ کا جواب ** - پہلے جمع کردہ ڈیٹا پر انحصار کریں۔ جب گہری ڈیبگنگ کی ضرورت ہوتی ہے تو ، ریلے ری پلے بالٹی اسنیپ شاٹس ہوں یا خام ٹریفک لاگز جمع کرنے کے بجائے خفیہ شدہ پیمائش کے ثبوتوں کی جانچ کریں۔
4. ** برقرار رکھیں ** - `max_completed_buckets` سے زیادہ سے بچنے کے لئے کافی ڈیٹا واپس لیں۔ برآمد کنندگان کو Prometheus کی پیداوار کو معیاری ماخذ کے طور پر غور کرنا چاہئے اور مقامی بالٹیوں کو پاس کرنے کے بعد حذف کرنا چاہئے۔

## تجزیات دبانے اور آٹومیشن

SNNET-8 کی قبولیت کا انحصار یہ ثابت کرنے پر ہے کہ خودکار جمع کرنے والے اچھی حالت میں رہتے ہیں اور یہ دباؤ پالیسی کی حدود میں رہتا ہے (کسی بھی 30 منٹ کی ونڈو پر فی ریلے میں بالٹیوں کا ≤10 ٪)۔ اس پورٹل کو پورا کرنے کے لئے درکار اشیا اب درخت کے ساتھ جہاز بھیج رہے ہیں۔ آپریٹرز کو اسے اپنی ہفتہ وار رسومات سے جوڑنا چاہئے۔ Grafana میں نئے دبانے والے پینل ذیل میں پروم کیو ایل کے اقتباسات کی عکاسی کرتے ہیں ، جس سے دستی سوالات کا سہارا لینے سے پہلے شفٹ ٹیموں کو براہ راست مرئیت ملتی ہے۔

### دبانے کے جائزے کے لئے پروم کیو ایل کی ترکیبیںآپریٹرز کو مندرجہ ذیل پروم کیو ایل مددگاروں کو قریب رکھنا چاہئے: دونوں کا ذکر Grafana کامن بورڈ (`dashboards/grafana/soranet_privacy_metrics.json`) اور الرٹ مینجر قواعد میں کیا گیا ہے:

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

اس بات کو یقینی بنانے کے لئے تناسب کی پیداوار کا استعمال کریں کہ اسٹیٹ ** "دبے ہوئے بالٹی ٪" ** پالیسی بجٹ کے تحت رہتا ہے۔ جب تعاون کرنے والوں کی تعداد غیر متوقع طور پر گرتی ہے تو فوری اطلاع حاصل کرنے کے لئے اسپائک ڈٹیکٹر کو الرٹ مینجر سے مربوط کریں۔

### بیرونی بالٹیوں کی اطلاع دہندگی کا آلہ

ورک اسپیس `cargo xtask soranet-privacy-report` کمانڈ کو ایک وقت کے NDJSON کیپچرز کے لئے فراہم کرتا ہے۔ اسے ریلے کے ایک یا زیادہ سے زیادہ کی طرف براہ راست ہدایت دیں:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

مددگار `SoranetSecureAggregator` کے توسط سے گرفتاری سے گزرتا ہے ، STDOUT پر ایک دبانے کا خلاصہ پرنٹ کرتا ہے ، اور اختیاری طور پر `--json-out <path|->` کے ذریعہ ایک ساختہ JSON رپورٹ لکھتا ہے۔ یہ اسی براہ راست کلکٹر کیز (`--bucket-secs` ، `--min-contributors` ، `--expected-shares` ، وغیرہ) کا احترام کرتا ہے ، جس سے آپریٹرز کو کسی واقعے کی تکمیل کرتے وقت تاریخی گرفتاریوں کو مختلف دہلیز کے ساتھ دوبارہ کھیلنے کی اجازت ملتی ہے۔ JSON کو Grafana اسنیپ شاٹس سے منسلک کریں تاکہ SNNET-8 دبانے والے تجزیات قابل آیت آڈٹ رہیں۔

### پہلی آٹومیشن چیک لسٹ

گورننس کے پاس ابھی بھی اس بات کا ثبوت درکار ہے کہ پہلی آٹومیشن نے دبانے والے بجٹ کو پورا کیا۔ ٹول اب `--max-suppression-ratio <0-1>` کو قبول کرتا ہے تاکہ جب دبے ہوئے بالٹیوں کو اجازت دی گئی ونڈو (پہلے سے طے شدہ 10 ٪) سے زیادہ ہو یا جب ابھی تک کوئی بالٹیاں نہ ہوں تو CI یا آپریٹرز تیزی سے ناکام ہوسکیں۔ تجویز کردہ بہاؤ:

1. ریلے کے ایڈمن پوائنٹس سے NDJSON برآمد کرنے کے ساتھ ساتھ آرکسٹیٹر کے `/v1/soranet/privacy/event|share` کو `artifacts/sorafs_privacy/<relay>.ndjson` میں اسٹریم `/v1/soranet/privacy/event|share` برآمد کریں۔
2. پالیسی بجٹ کے ساتھ اسسٹنٹ کا کام:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   کمانڈ مختص فیصد پرنٹ کرتا ہے اور غیر صفر علامت کے ساتھ باہر نکلتا ہے جب بجٹ سے تجاوز کیا جاتا ہے ** یا ** جب بالٹی ابھی تیار نہیں ہوتی ہے ، اس بات کا اشارہ ہے کہ اس رن کے لئے ابھی تک ٹیلی میٹری تیار نہیں کی گئی ہے۔ براہ راست میٹرکس کو یہ ظاہر کرنا چاہئے کہ `soranet_privacy_pending_collectors` صفر پر جاتا ہے اور یہ کہ `soranet_privacy_snapshot_suppression_ratio` آپریشن کے دوران اسی بجٹ کے تحت رہتا ہے۔
3. ڈیفالٹ ٹرانسپورٹ کو تبدیل کرنے سے پہلے JSON آؤٹ پٹ اور سی ایل آئی لاگ کو SNNET-8 شواہد پیکیج کے ساتھ محفوظ کریں تاکہ جائزہ لینے والے ایک ہی حصوں کو دوبارہ چلاسکیں۔

## اگلے اقدامات (Snnet-8a)

- دوہری پریو جمع کرنے والوں کو ضم کریں اور رن ٹائم میں حصص کے اندراج کو لنک کریں تاکہ ریلے اور جمع کرنے والے مستقل `SoranetPrivacyBucketMetricsV1` پے لوڈ جاری کریں۔ *(ہو گیا - `ingest_privacy_payload` `crates/sorafs_orchestrator/src/lib.rs` میں اور اس کے ساتھ والے ٹیسٹ دیکھیں۔)*
- مشترکہ Prometheus بورڈ اور انتباہی قواعد کو تعی .ن کریں جس میں دبانے والے فرق ، کلکٹر کی توثیق ، ​​اور گمنامی رول بیکس شامل ہیں۔ *(ہو گیا - `dashboards/grafana/soranet_privacy_metrics.json` ، `dashboards/alerts/soranet_privacy_rules.yml` ، `dashboards/alerts/soranet_policy_rules.yml` اور توثیق فائلیں دیکھیں۔)*
- `privacy_metrics_dp.md` میں بیان کردہ تفریق پرائیویسی انشانکن مواد تیار کریں جن میں تولیدی نوٹ بک اور گورننس سمری شامل ہیں۔ *(ہو - نوٹ بک اور مواد `scripts/telemetry/run_privacy_dp.py` کے ذریعہ تیار کیا گیا تھا C CI ریپر `scripts/telemetry/run_privacy_dp_notebook.sh` ورک فلو `.github/workflows/release-pipeline.yml` کے ذریعے نوٹ بک چلاتا ہے I `docs/source/status/soranet_privacy_dp_digest.md` میں گورننس کا خلاصہ محفوظ کیا گیا۔)

موجودہ ریلیز میں SNNET-8 کی بنیاد متعارف کرائی گئی ہے: عین مطابق ، رازداری سے محفوظ ٹیلی میٹری جو براہ راست موجودہ Prometheus سکریپرس اور ڈیش بورڈ سے منسلک ہوتی ہے۔ مختلف رازداری کے انشانکن مواد موجود ہیں ، ایک ریلیز ورک فلو نوٹ بک کی آؤٹ پٹ کو موجودہ رکھتا ہے ، اور باقی کام پہلے آٹومیشن رن کی نگرانی اور دبانے والے انتباہات پر تجزیات کو بڑھانے پر مرکوز ہے۔