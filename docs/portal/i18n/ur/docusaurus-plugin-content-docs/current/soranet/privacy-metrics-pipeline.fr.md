---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.fr.md
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
`docs/source/soranet/privacy_metrics_pipeline.md` کی عکاسی کرتا ہے۔ دستاویزات کا پرانا سیٹ ریٹائر ہونے تک دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

# سورانیٹ پرائیویسی میٹرکس پائپ لائن

SNNET-8 ریلے رن ٹائم کے لئے رازداری سے واقف ٹیلی میٹری کی سطح متعارف کراتا ہے۔ ریلے اب مصافحہ اور سرکٹ کے واقعات کو ایک منٹ کی بالٹیوں میں جمع کرتا ہے اور صرف موٹے Prometheus کاؤنٹرز کو برآمد کرتا ہے ، اور آپریٹرز کو قابل عمل مرئیت دیتے ہوئے انفرادی سرکٹس کو غیر منسلک رکھتا ہے۔

## اجتماعی جائزہ

- `PrivacyAggregator` کے تحت `tools/soranet-relay/src/privacy.rs` میں رن ٹائم عمل درآمد رہتا ہے۔
- بالٹیوں کو گھڑی کے منٹ (`bucket_secs` ، پہلے سے طے شدہ 60 سیکنڈ) کے ذریعہ ترتیب دیا جاتا ہے اور پابند رنگ میں محفوظ کیا جاتا ہے (`max_completed_buckets` ، پہلے سے طے شدہ 120)۔ کلکٹر کے حصص اپنے پابند بیک بلاگ (`max_share_lag_buckets` ، پہلے سے طے شدہ 12) کو برقرار رکھتے ہیں تاکہ باسی پریو ونڈوز میموری کو لیک کرنے یا مسدود جمع کرنے والوں کو چھپانے کے بجائے حذف شدہ بالٹیوں میں ڈال دیئے جائیں۔
- `RelayConfig::privacy` براہ راست `PrivacyConfig` ، بے نقاب ترتیبات (`bucket_secs` ، Prometheus ، `flush_delay_buckets` ، `force_flush_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، `max_completed_buckets` ، پروڈکشن رن ٹائم ڈیفالٹس رکھتا ہے جبکہ SNNET-8A محفوظ جمع کی دہلیز متعارف کراتا ہے۔
- رن ٹائم ماڈیولز ٹائپڈ مددگاروں کے ذریعہ واقعات ریکارڈ کرتے ہیں: `record_circuit_accepted` ، `record_circuit_rejected` ، `record_throttle` ، `record_throttle_cooldown` ، `record_capacity_reject` ، `record_active_sample` ، `record_active_sample` اور `record_active_sample` اور `record_active_sample` اور `record_active_sample` اور `record_active_sample` اور `record_active_sample`۔

## ریلے ایڈمن اینڈ پوائنٹ

آپریٹرز `GET /privacy/events` کے ذریعے خام مشاہدات کے لئے ریلے ایڈمن سننے والوں سے استفسار کرسکتے ہیں۔ اختتامی نقطہ نئی لائن سے طے شدہ JSON (`application/x-ndjson`) پر مشتمل ہے جس میں `SoranetPrivacyEventV1` پے لوڈز شامل ہیں جو داخلی `PrivacyEventBuffer` سے ظاہر ہوتا ہے۔ بفر `privacy.event_buffer_capacity` اندراجات (پہلے سے طے شدہ 4096) تک حالیہ واقعات کو برقرار رکھتا ہے اور اسے پڑھنے پر خالی کردیا جاتا ہے ، لہذا سوراخوں سے بچنے کے لئے کھرچنیوں کو اکثر جانچ پڑتال کرنی ہوگی۔ واقعات میں ایک ہی مصافحہ ، تھروٹل ، تصدیق شدہ بینڈوتھ ، ایکٹو سرکٹ ، اور GAR اشارے شامل ہیں جو Prometheus کاؤنٹرز کو طاقت دیتے ہیں ، جس سے بہاو جمع کرنے والوں کو رازداری سے محفوظ بریڈ کرمبس یا بجلی سے محفوظ جمع کرنے والے کام کے بہاؤ کو محفوظ کرنے کی اجازت ملتی ہے۔

## ریلے کنفیگریشن

آپریٹرز سیکشن `privacy` کے ذریعے ریلے کنفیگریشن فائل میں رازداری کے ٹیلی میٹری کیڈینس کو ایڈجسٹ کرتے ہیں:

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

پہلے سے طے شدہ فیلڈ اقدار SNNET-8 کی تفصیلات کے مطابق ہیں اور بوجھ پر توثیق کی جاتی ہیں:| فیلڈ | تفصیل | ڈیفالٹ |
| ------- | -------- | -------- |
| `bucket_secs` | ہر جمع ونڈو کی چوڑائی (سیکنڈ)۔ | `60` |
| `min_handshakes` | بالٹی سے پہلے کم سے کم شراکت کاروں کی تعداد کاؤنٹر جاری کرسکتی ہے۔ | `12` |
| `flush_delay_buckets` | فلش کی کوشش کرنے سے پہلے انتظار کرنے کے لئے مکمل بالٹیاں کی تعداد۔ | `1` |
| `force_flush_buckets` | حذف شدہ بالٹی جاری کرنے سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` | بالٹیوں کا بیکلاگ محفوظ ہے (بے حد میموری سے پرہیز کرتا ہے)۔ | `120` |
| `max_share_lag_buckets` | حذف کرنے سے پہلے کلکٹر شیئر برقرار رکھنے کی ونڈو۔ | `12` |
| `expected_shares` | مل کر جمع کرنے سے پہلے حصص کلیکٹر پریو کی ضرورت ہے۔ | `2` |
| `event_buffer_capacity` | ایڈمن فیڈ کے لئے این ڈی جےسن ایونٹ کا بیک بلاگ۔ | `4096` |

`force_flush_buckets` کو `flush_delay_buckets` سے کم ترتیب دینا ، حد کو صفر پر رکھنا ، یا برقرار رکھنے والے گارڈ کو غیر فعال کرنا اب تعیناتیوں سے بچنے کے لئے توثیق میں ناکام ہوجاتا ہے جو ریلے ٹیلی میٹری کو لیک کرتے ہیں۔

حد `event_buffer_capacity` بھی `/admin/privacy/events` کی حدود ہے ، اس بات کو یقینی بناتا ہے کہ کھرچنی غیر معینہ مدت تک پیچھے نہیں رہ سکتی ہے۔

## پریو جمع کرنے والوں کے حصص

SNNET-8A دوہری جمع کرنے والے تعینات کرتا ہے جو خفیہ شیئرنگ پریو بالٹیاں خارج کرتے ہیں۔ آرکسٹریٹر اب این ڈی جے ایسنس اسٹریم `/privacy/events` کو اندراجات `SoranetPrivacyEventV1` کے لئے پارس کرتا ہے اور `SoranetPrivacyPrioShareV1` کو شیئر کرتا ہے ، جس سے وہ `SoranetSecureAggregator::ingest_prio_share` میں منتقل ہوتا ہے۔ ایک بار `PrivacyBucketConfig::expected_shares` شراکتیں آنے کے بعد بالٹی خارج ہوجاتی ہیں ، جو ریلے کے طرز عمل کی عکاسی کرتی ہیں۔ `SoranetPrivacyBucketMetricsV1` میں جوڑنے سے پہلے بالٹی سیدھ اور ہسٹگرام شکل کے لئے حصص کی توثیق کی جاتی ہے۔ اگر ہینڈ شیکس کی مشترکہ تعداد `min_contributors` کے نیچے آتی ہے تو ، بالٹی کو `suppressed` کے طور پر برآمد کیا جاتا ہے ، جو ریلے کی طرف جمع کرنے والے کے طرز عمل کی عکاسی کرتا ہے۔ ہٹا دی گئی ونڈوز اب ایک `suppression_reason` لیبل خارج کرتی ہے تاکہ آپریٹرز ٹیلی میٹری کے سوراخوں کی تشخیص کرتے وقت `insufficient_contributors` ، `collector_suppressed` ، `collector_window_elapsed` ، اور `forced_flush_window_elapsed` کے درمیان فرق کرسکیں۔ `collector_window_elapsed` وجہ بھی اس وقت متحرک ہوجاتی ہے جب پریو `max_share_lag_buckets` سے آگے پیچھے رہ جاتا ہے ، جس سے مسدود جمع کرنے والوں کو باسی جمع کرنے والوں کو میموری میں چھوڑنے کے بغیر مرئی بناتا ہے۔

## ingesion اختتامی نقطہ Torii

Torii اب دو ٹیلی میٹری سے محفوظ HTTP اختتامی نکات کو بے نقاب کرتا ہے تاکہ ریلے اور جمع کرنے والے اپنی مرضی کے مطابق نقل و حمل کو سرایت کیے بغیر مشاہدات کو منتقل کرسکیں:- `POST /v2/soranet/privacy/event` ایک پے لوڈ `RecordSoranetPrivacyEventDto` کو قبول کرتا ہے۔ جسم `SoranetPrivacyEventV1` کے علاوہ ایک اختیاری `source` لیبل کو گھیرتا ہے۔ Torii فعال ٹیلی میٹری پروفائل کے خلاف درخواست کی توثیق کرتا ہے ، ایونٹ کو لاگ کرتا ہے اور HTTP `202 Accepted` کے ساتھ جواب دیتا ہے جس کے ساتھ Norito JSON لفافہ ہوتا ہے جس میں حساب شدہ ونڈو (`bucket_start_unix` ، `bucket_duration_secs`) شامل ہوتا ہے۔
- `POST /v2/soranet/privacy/share` ایک پے لوڈ `RecordSoranetPrivacyShareDto` کو قبول کرتا ہے۔ جسم میں `SoranetPrivacyPrioShareV1` اور ایک اختیاری `forwarded_by` انڈیکس ہے تاکہ آپریٹر جمع کرنے والے کے بہاؤ کا آڈٹ کرسکیں۔ کامیاب گذارشات Norito JSON لفافے کے ساتھ HTTP Grafana واپس کریں گے ، جمع کرنے والے ، بالٹی ونڈو ، اور اشارے کو حذف کرنے کا خلاصہ کرتے ہیں۔ توثیق کی ناکامی جمع کرنے والوں کے مابین ڈٹرمینسٹک غلطی پروسیسنگ کو محفوظ رکھنے کے لئے `Conversion` ٹیلی میٹری کے ردعمل سے مطابقت رکھتی ہے۔ آرکسٹریٹر ایونٹ لوپ اب ان حصص کو خارج کرتا ہے جب وہ ریلے کا انتخاب کرتا ہے ، اور ریلے کی طرف بالٹیوں کے ساتھ ہم آہنگی میں Torii کے پریو جمع کرنے والے کو برقرار رکھتا ہے۔

دونوں نکات ٹیلی میٹری پروفائل کا احترام کرتے ہیں: جب میٹرکس غیر فعال ہوجاتے ہیں تو وہ `503 Service Unavailable` جاری کرتے ہیں۔ کلائنٹ بائنری Norito (`application/x.norito`) یا Norito JSON (`application/x.norito+json`) لاش بھیج سکتے ہیں۔ سرور خود بخود معیاری Torii ایکسٹریکٹرز کے ذریعہ فارمیٹ پر بات چیت کرتا ہے۔

## میٹرکس Prometheus

ہر برآمد شدہ بالٹی میں لیبل `mode` (`entry` ، `middle` ، `exit`) اور `bucket_start` ہیں۔ میٹرکس کے مندرجہ ذیل خاندان جاری کیے گئے ہیں:| میٹرک | تفصیل |
| -------- | -------- |
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` کے ساتھ ہینڈ شیک درجہ بندی۔ |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` کے ساتھ تھروٹل میٹر۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | تھروٹلڈ ہینڈ ہیکس کے ذریعہ فراہم کردہ مجموعی کوولڈاؤن اوقات۔ |
| `soranet_privacy_verified_bytes_total` | اندھے پیمائش کے ثبوت سے تصدیق شدہ بینڈوتھ۔ |
| `soranet_privacy_active_circuits_{avg,max}` | اوسط اور چوٹی کے فعال سرکٹس فی بالٹی۔ |
| `soranet_privacy_rtt_millis{percentile}` | آر ٹی ٹی پرسنٹائل تخمینہ (`p50` ، `p90` ، `p99`)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | ہیشڈ گورننس ایکشن رپورٹ کے کاؤنٹرز کا مقابلہ زمرہ ڈائجسٹ کے ذریعہ کیا گیا ہے۔ |
| `soranet_privacy_bucket_suppressed` | بالٹیوں کو روکا گیا کیونکہ شراکت دار کی دہلیز تک نہیں پہنچی تھی۔ |
| `soranet_privacy_pending_collectors{mode}` | کلیکٹر شیئر کرنے والے جمع کرنے والے مجموعہ کے منتظر ہیں ، جو ریلے وضع کے ذریعہ گروپ کیا گیا ہے۔ |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` کے ساتھ بالٹی کاؤنٹرز کو ہٹا دیا گیا تاکہ ڈیش بورڈ پرائیویسی سوراخ تفویض کرسکیں۔ |
| `soranet_privacy_snapshot_suppression_ratio` | انتباہ بجٹ کے لئے مفید ، آخری ڈرین (0-1) کو حذف/خالی کردیا گیا۔ |
| `soranet_privacy_last_poll_unixtime` | آخری کامیاب سروے کا یونکس ٹائم اسٹیمپ (کلیکٹر-آئیڈل الرٹ کو فیڈ کرتا ہے)۔ |
| `soranet_privacy_collector_enabled` | گیج جو `0` میں تبدیل ہوتا ہے جب رازداری جمع کرنے والا غیر فعال ہوتا ہے یا شروع نہیں ہوتا ہے (کلکٹر سے معذور انتباہ کو کھانا کھلاتا ہے)۔ |
| `soranet_privacy_poll_errors_total{provider}` | پولنگ کی ناکامیوں نے ریلے عرف (ضابطہ کشائی کی غلطیوں ، HTTP کی ناکامیوں ، یا غیر متوقع اسٹیٹس کوڈز میں اضافہ) کے ذریعہ گروپ کیا۔ |

مشاہدات کے بغیر بالٹیاں خاموش رہتی ہیں ، بغیر زیرو سے بھری کھڑکیوں کو بنائے ڈیش بورڈز کو صاف رکھتے ہیں۔

## آپریشنل رہنمائی1. ** ڈیش بورڈز ** - مذکورہ بالا میٹرکس کو `mode` اور `window_start` کے ذریعہ گروپ کردہ پلاٹ کریں۔ کلیکٹر یا ریلے کی پریشانیوں کی اطلاع دینے کے لئے گمشدہ ونڈوز کو اجاگر کریں۔ سوراخوں کو چھانٹتے وقت کلیکٹر سے چلنے والے حذفوں سے شراکت داروں کے فرق کو ممتاز کرنے کے لئے `soranet_privacy_suppression_total{reason}` کا استعمال کریں۔ Grafana اثاثہ میں اب ایک سرشار ** "دبانے کی وجوہات (5 میٹر)" ** ان کاؤنٹرز کے ذریعہ چلنے والے پینل میں شامل ہیں ، نیز ایک ** "دبے ہوئے بالٹی ٪" ** اسٹیٹ جو انتخاب کے ذریعہ `sum(soranet_privacy_bucket_suppressed) / count(...)` کا حساب لگاتا ہے لہذا آپریٹرز ایک نظر میں بجٹ کو ختم کرتا ہے۔ ** "کلکٹر شیئر بیک بلاگ" ** سیریز (`soranet_privacy_pending_collectors`) اور ** "اسنیپ شاٹ دبانے کا تناسب" ** اسٹیٹ خود کار طریقے سے رنز کے دوران بلاک شدہ جمع کرنے والوں اور بجٹ کے بڑھنے کو نمایاں کریں۔
2. ** انتباہ ** - پرائیویسی سے محفوظ کاؤنٹرز سے ڈرائیو الرٹس: POW مسترد چوٹیوں ، کولڈاؤن فریکوئنسی ، RTT بڑھنے اور صلاحیت کی کمی۔ چونکہ کاؤنٹر ہر بالٹی کے اندر اجارہ دار ہوتے ہیں ، لہذا شرح کے آسان قواعد اچھی طرح سے کام کرتے ہیں۔
3. ** واقعہ کا جواب ** - پہلے جمع شدہ ڈیٹا پر انحصار کریں۔ جب گہری ڈیبگنگ کی ضرورت ہوتی ہے تو ، ریلے کو ری پلے بالٹی اسنیپ شاٹس لگائیں یا خام ٹریفک لاگز کی کٹائی کے بجائے بلائنڈ میٹرک شواہد کا معائنہ کریں۔
4. ** برقرار رکھنا ** - `max_completed_buckets` سے زیادہ سے زیادہ سے زیادہ سے زیادہ سے زیادہ سے زیادہ کو کھرچنا۔ برآمد کنندگان کو Prometheus آؤٹ پٹ کو کیننیکل ماخذ کے طور پر سلوک کرنا چاہئے اور مقامی بالٹیوں کو ایک بار منتقل کرنے کے بعد حذف کرنا چاہئے۔

## ہٹانے کا تجزیہ اور خودکار پھانسی

SNNET-8 قبولیت کا انحصار یہ ظاہر کرنے پر ہے کہ خودکار جمع کرنے والے صحت مند رہتے ہیں اور حذف ہونا پالیسی کی حدود میں رہتا ہے (کسی بھی 30 منٹ کی ونڈو میں فی ریلے میں بالٹیوں کا ≤10 ٪)۔ اس ضرورت کو پورا کرنے کے لئے درکار اوزار اب ذخیرہ اندوزی کے ساتھ آتے ہیں۔ آپریٹرز کو انہیں اپنی ہفتہ وار رسومات میں ضم کرنا ہوگا۔ نئے Grafana دبانے والے پینل ذیل میں پروم کیو ایل کے ٹکڑوں کی عکاسی کرتے ہیں ، جس سے دستی ٹیموں کو دستی سوالات کا سہارا لینے سے پہلے براہ راست مرئیت ملتی ہے۔

### ہٹانے کے جائزے کے لئے پروم کیو ایل کی ترکیبیں

آپریٹرز کو مندرجہ ذیل پروم کیو ایل مددگاروں کو ہاتھ میں رکھنا چاہئے۔ دونوں کا مشترکہ ڈیش بورڈ Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) اور الرٹ مینجر کے قواعد میں حوالہ دیا گیا ہے:

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

اس بات کی تصدیق کرنے کے لئے تناسب کا استعمال کریں کہ ** "دبے ہوئے بالٹی ٪" ** اسٹیٹ پالیسی بجٹ کے تحت باقی ہے۔ جب تعاون کرنے والوں کی تعداد غیر متوقع طور پر گرتی ہے تو فوری تاثرات کے لئے اسپائک ڈٹیکٹر کو الرٹ مینجر میں پلگ ان کریں۔

### آف لائن بالٹی رپورٹ سی ایل آئی

ورک اسپیس نے ایک وقت کے NDJSON کیپچر کے لئے `cargo xtask soranet-privacy-report` کو بے نقاب کیا۔ اسے ایک یا زیادہ ریلے ایڈمن برآمدات کی طرف اشارہ کریں:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```یہ آلہ `SoranetSecureAggregator` کے ذریعے گرفتاری سے گزرتا ہے ، STDOUT کو حذف کرنے کا خلاصہ پرنٹ کرتا ہے ، اور اختیاری طور پر `--json-out <path|->` کے ذریعے ایک ساختہ JSON رپورٹ لکھتا ہے۔ یہ اسی ترتیبات کا احترام کرتا ہے جیسے براہ راست کلکٹر (`--bucket-secs` ، `--min-contributors` ، `--expected-shares` ، وغیرہ) ، جب کسی واقعے کی تکمیل کرتے وقت آپریٹرز کو مختلف دہلیز کے تحت تاریخی گرفتاریوں کو دوبارہ چلانے کی اجازت دی جاتی ہے۔ Grafana کے ساتھ JSON منسلک کریں تاکہ SNNET-8 کو حذف کرنے کا پارسنگ گیٹ قابل نہ ہو۔

### خودکار پہلی رن چیک لسٹ

گورننس کو اب بھی یہ ثابت کرنے کی ضرورت ہے کہ پہلی خودکار رن نے حذف کرنے کے بجٹ کو پورا کیا۔ ٹول اب `--max-suppression-ratio <0-1>` کو قبول کرتا ہے تاکہ جب حذف شدہ بالٹیاں اجازت شدہ ونڈو (10 ٪ پہلے سے طے شدہ) سے زیادہ ہو تو CI یا آپریٹرز تیزی سے ناکام ہوسکیں یا جب کوئی بالٹیاں ابھی موجود نہیں ہیں۔ تجویز کردہ بہاؤ:

1. ریلے ایڈمن اینڈ پوائنٹس کے علاوہ `/v2/soranet/privacy/event|share` اسٹریم سے آرکسٹیٹر سے `artifacts/sorafs_privacy/<relay>.ndjson` پر NDJSON برآمد کریں۔
2. پالیسی بجٹ کے ساتھ ٹول چلائیں:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   کمانڈ مشاہدہ شدہ تناسب پرنٹ کرتا ہے اور غیر صفر کوڈ کے ساتھ باہر نکلتا ہے جب بجٹ سے تجاوز کیا جاتا ہے ** یا ** جب کوئی بالٹیاں تیار نہیں ہوتی ہیں ، اس بات کا اشارہ کرتے ہیں کہ ٹیلی میٹری ابھی تک عملدرآمد کے لئے تیار نہیں کی گئی ہے۔ براہ راست میٹرکس کو `soranet_privacy_pending_collectors` صفر اور `soranet_privacy_snapshot_suppression_ratio` کی طرف جانے کے دوران اسی بجٹ کے تحت رہنا چاہئے۔
3. ڈیفالٹ ٹرانسپورٹ کو ٹوگل کرنے سے پہلے JSON آؤٹ پٹ اور CLI لاگ کو Snnet-8 پروف فائل کے ساتھ محفوظ کریں تاکہ جائزہ لینے والے عین مطابق نمونے کو دوبارہ چلاسکیں۔

## اگلے اقدامات (Snnet-8a)

- دوہری پریو جمع کرنے والوں کو انضمام کریں ، ان کے حص share ہ کو رن ٹائم سے جوڑ کر تاکہ ریلے اور جمع کرنے والے مستقل `SoranetPrivacyBucketMetricsV1` پے لوڈ کا اخراج کریں۔ *(ہو گیا - `ingest_privacy_payload` `crates/sorafs_orchestrator/src/lib.rs` اور اس سے وابستہ ٹیسٹ میں دیکھیں۔)*
- مشترکہ Prometheus ڈیش بورڈ اور الرٹ کے قواعد کو حذف کرنے والے سوراخوں ، کلکٹر کی صحت اور گمنامی کے قطرے کو شائع کریں۔ *(ہو گیا - `dashboards/grafana/soranet_privacy_metrics.json` ، `dashboards/alerts/soranet_privacy_rules.yml` ، `dashboards/alerts/soranet_policy_rules.yml` اور توثیق فکسچر دیکھیں۔)*
- `privacy_metrics_dp.md` میں بیان کردہ تفریق پرائیویسی انشانکن نمونے تیار کریں ، جس میں تولیدی نوٹ بک اور گورننس ڈائجسٹ شامل ہیں۔ *(کیا ہوا - `scripts/telemetry/run_privacy_dp.py` کے ذریعہ تیار کردہ نوٹ بک + نمونے ؛ CI ریپر `scripts/telemetry/run_privacy_dp_notebook.sh` ورک فلو `.github/workflows/release-pipeline.yml` کے ذریعے نوٹ بک چلاتا ہے I `docs/source/status/soranet_privacy_dp_digest.md` میں گورننس ڈائجسٹ دائر کیا گیا۔)موجودہ ورژن SNNET-8 فاؤنڈیشن کی فراہمی کرتا ہے: عزم ، رازداری سے محفوظ ٹیلی میٹری جو براہ راست Prometheus سکریپرس اور ڈیش بورڈز کے ساتھ مربوط ہوتی ہے۔ تفریق پرائیویسی انشانکن نمونے کی جگہ موجود ہے ، ریلیز پائپ لائن ورک فلو نوٹ بک کے آؤٹ پٹس کو تازہ ترین رکھتا ہے ، اور باقی کام پہلے خودکار رن کے علاوہ حذف کرنے کے انتباہی تجزیات کو بڑھانے کی نگرانی پر مرکوز ہے۔