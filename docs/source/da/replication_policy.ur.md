---
lang: ur
direction: rtl
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T15:38:30.661849+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ڈیٹا کی دستیابی کی نقل پالیسی (DA-4)

_ اسٹیٹس: پیشرفت میں - مالکان: کور پروٹوکول WG / اسٹوریج ٹیم / SRE_

ڈی اے انجشن پائپ لائن اب برقرار رکھنے کے اہداف کو نافذ کرنے کے اہداف کو نافذ کرتی ہے
`roadmap.md` (ورک اسٹریم DA-4) میں بیان کردہ ہر بلاب کلاس۔ Torii سے انکار کرتا ہے
کالر فراہم کردہ برقراری کے لفافوں کو برقرار رکھیں جو تشکیل شدہ سے مماثل نہیں ہیں
پالیسی ، اس بات کی ضمانت دیتا ہے کہ ہر جائز/اسٹوریج نوڈ مطلوبہ برقرار رکھتا ہے
جمع کرانے والے ارادے پر بھروسہ کیے بغیر عہدوں اور نقلوں کی تعداد۔

## ڈیفالٹ پالیسی

| بلاب کلاس | گرم برقرار رکھنا | سرد برقرار رکھنا | مطلوبہ نقلیں | اسٹوریج کلاس | گورننس ٹیگ |
| ------------ | ----------------- | ------------------ | ------------------------------------------ | -------------------- |
| `taikai_segment` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 گھنٹے | 7 دن | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 گھنٹے | 180 دن | 3 | `cold` | `da.governance` |
| _ ڈیفالٹ (دیگر تمام کلاسز) _ | 6 گھنٹے | 30 دن | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں سرایت کر چکے ہیں اور اس پر لاگو ہیں
تمام `/v2/da/ingest` گذارشات۔ Torii نافذ شدہ کے ساتھ ظاہر ہوتا ہے
جب کال کرنے والے مماثل اقدار فراہم کرتے ہیں تو برقرار رکھنے کا پروفائل اور ایک انتباہ کا اخراج کرتا ہے
آپریٹرز باسی SDKs کا پتہ لگاسکتے ہیں۔

### تائکائی دستیابی کی کلاسیں

تائیکائی روٹنگ مینیفنس (`taikai.trm` میٹا ڈیٹا) اب ایک شامل ہے
`availability_class` اشارہ (`Hot` ، `Warm` ، یا `Cold`)۔ جب موجود ہو تو ، Torii
`torii.da_ingest.replication_policy` سے مماثل برقراری کا پروفائل منتخب کرتا ہے
پے لوڈ کو چنک کرنے سے پہلے ، ایونٹ کے آپریٹرز کو غیر فعال کو کم کرنے کی اجازت دیتا ہے
عالمی پالیسی ٹیبل میں ترمیم کیے بغیر رینڈیشنز۔ پہلے سے طے شدہ ہیں:

| دستیابی کی کلاس | گرم برقرار رکھنا | سرد برقرار رکھنا | مطلوبہ نقلیں | اسٹوریج کلاس | گورننس ٹیگ |
| -------------------- | --------------- | ------------------ | ---------------------------------------- | -------------------- |
| `hot` | 24 گھنٹے | 14 دن | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 گھنٹے | 30 دن | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 گھنٹہ | 180 دن | 3 | `cold` | `da.taikai.archive` |

اگر منشور `availability_class` کو چھوڑ دیتا ہے تو ، انجسٹ کا راستہ واپس آجاتا ہے
`hot` پروفائل لہذا لائیو اسٹریمز اپنی مکمل نقل کو سیٹ رکھیں۔ آپریٹرز کر سکتے ہیں
نئے میں ترمیم کرکے ان اقدار کو اوور رائڈ کریں
ترتیب میں `torii.da_ingest.replication_policy.taikai_availability` بلاک۔

## کنفیگریشن

پالیسی `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور بے نقاب ہوتی ہے
* ڈیفالٹ* ٹیمپلیٹ کے علاوہ فی کلاس اوور رائڈز کی ایک صف۔ کلاس شناخت کرنے والے ہیں
کیس غیر حساس اور قبول `taikai_segment` ، `nexus_lane_sidecar` ،
گورننس سے منظور شدہ توسیع کے لئے `governance_artifact` ، یا `custom:<u16>`۔
اسٹوریج کلاسز `hot` ، `warm` ، یا `cold` کو قبول کرتے ہیں۔

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

مذکورہ بالا ڈیفالٹس کے ساتھ چلانے کے لئے بلاک کو اچھوت چھوڑ دیں۔ سخت کرنے کے لئے a
کلاس ، مماثل اوور رائڈ کو اپ ڈیٹ کریں۔ نئی کلاسوں کے لئے بیس لائن کو تبدیل کرنے کے لئے ،
`default_retention` میں ترمیم کریں۔مخصوص تائکائی دستیابی کی کلاسوں کو ایڈجسٹ کرنے کے لئے ، اندراجات کے تحت شامل کریں
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## نفاذ کے الفاظ

- Torii صارف کے فراہم کردہ `RetentionPolicy` کو نافذ کردہ پروفائل کے ساتھ تبدیل کرتا ہے
  chunking یا ظاہر ہونے سے پہلے۔
- پری بلٹ کے ظاہر ہونے والے مظہر کو جو متضاد برقرار رکھنے کا اعلان کرتے ہیں اسے مسترد کردیا جاتا ہے
  `400 schema mismatch` کے ساتھ لہذا باسی کلائنٹ معاہدے کو کمزور نہیں کرسکتے ہیں۔
- ہر اوور رائڈ ایونٹ لاگ ان ہوتا ہے (`blob_class` ، پیش کردہ بمقابلہ متوقع پالیسی)
  رول آؤٹ کے دوران غیر تعمیل کال کرنے والوں کی سطح پر۔

تازہ ترین گیٹ کے لئے `docs/source/da/ingest_plan.md` (توثیق کی فہرست کی فہرست) دیکھیں
برقرار رکھنے کے نفاذ کا احاطہ کرنا۔

## دوبارہ دوبارہ استعمال کرنے والا ورک فلو (DA-4 فالو اپ)

برقرار رکھنے کا نفاذ صرف پہلا قدم ہے۔ آپریٹرز کو بھی یہ ثابت کرنا ہوگا
براہ راست ظاہر اور نقل کے احکامات تشکیل شدہ پالیسی کے ساتھ منسلک رہتے ہیں لہذا
کہ SoraFS خود بخود دوبارہ تعمیل کے بلبس کو دوبارہ تبدیل کرسکتا ہے۔

1. ** بہاؤ کے لئے دیکھیں۔ ** Torii اخراج
   `overriding DA retention policy to match configured network baseline` جب بھی
   ایک کال کرنے والا باسی برقرار رکھنے کی اقدار پیش کرتا ہے۔ اس کے ساتھ لاگ ان کریں
   `torii_sorafs_replication_*` ٹیلی میٹری کو نقل کی کمی یا تاخیر سے اسپاٹ کریں
   redeplements.
2. ** مختلف ارادے بمقابلہ براہ راست نقلیں۔ ** نیا آڈٹ مددگار استعمال کریں:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   کمانڈ فراہم کردہ سے `torii.da_ingest.replication_policy` لوڈ کرتا ہے
   ہر منشور (JSON یا Norito) کی تشکیل ، کنفیگر ، ڈیکوڈ کرتا ہے ، اور اختیاری طور پر کسی بھی ملتے ہیں
   `ReplicationOrderV1` پے لوڈز بذریعہ منشور ڈائجسٹ۔ خلاصہ جھنڈے دو
   شرائط:

   - `policy_mismatch` - منشور برقرار رکھنے کا پروفائل نافذ شدہ سے ہٹ جاتا ہے
     پالیسی (یہ کبھی نہیں ہونا چاہئے جب تک کہ Torii غلط کنفیگر نہیں ہوتا ہے)۔
   - `replica_shortfall` - براہ راست نقل کے آرڈر کے مقابلے میں کم نقل کی درخواست کرتا ہے
     `RetentionPolicy.required_replicas` یا اس سے کم اسائنمنٹس فراہم کرتا ہے
     ہدف

   غیر صفر سے باہر نکلنے کی حیثیت ایک فعال کمی کی نشاندہی کرتی ہے لہذا CI/آن کال آٹومیشن
   فوری طور پر صفحہ دے سکتا ہے. JSON رپورٹ کو منسلک کریں
   پارلیمنٹ کے ووٹوں کے لئے `docs/examples/da_manifest_review_template.md` پیکٹ۔
3. ** ٹرگر دوبارہ دوبارہ نقل۔
   `ReplicationOrderV1` گورننس ٹولنگ کے ذریعے بیان کیا گیا ہے
   `docs/source/sorafs/storage_capacity_marketplace.md` اور آڈٹ کو دوبارہ چلائیں
   جب تک نقل تیار نہ ہوجائے۔ ایمرجنسی اوور رائڈس کے لئے ، سی ایل آئی آؤٹ پٹ کو جوڑیں
   `iroha app da prove-availability` کے ساتھ تاکہ SREs ایک ہی ڈائجسٹ کا حوالہ دے سکیں
   اور PDP ثبوت۔

ریگریشن کوریج `integration_tests/tests/da/replication_policy.rs` میں رہتا ہے۔
سویٹ نے ایک مماثل برقراری پالیسی کو `/v2/da/ingest` پر پیش کیا اور تصدیق کی
کہ بازیافت شدہ ظاہر ہوتا ہے کالر کے بجائے نافذ شدہ پروفائل کو بے نقاب کرتا ہے
ارادہ

## پروف-ہیلتھ ٹیلی میٹری اور ڈیش بورڈز (DA-5 برج)

روڈ میپ آئٹم ** ڈی اے 5 ** میں PDP/POTR نفاذ کے نتائج کو قابل آڈٹ کرنے کی ضرورت ہے
اصل وقت `SorafsProofHealthAlert` واقعات اب ایک سرشار سیٹ چلاتے ہیں
Prometheus میٹرکس:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

** SoraFS PDP اور POTR صحت ** Grafana بورڈ
(`dashboards/grafana/sorafs_pdp_potr_health.json`) اب ان اشاروں کو بے نقاب کرتا ہے:- * ٹرگر کے ذریعہ پروف ہیلتھ الرٹس * ٹرگر/پنلٹی پرچم کے ذریعہ چارٹ الرٹ کی شرح
  تائکائی/سی ڈی این آپریٹرز یہ ثابت کرسکتے ہیں کہ آیا صرف PDP ، صرف ، صرف ، صرف ، یا دوہری ہڑتالیں ہیں
  فائرنگ
- * کولڈاؤن میں فراہم کنندگان * فی الحال ایک کے تحت فراہم کرنے والوں کی براہ راست رقم کی اطلاع دیتا ہے
  sorafsprofhealthalert cooldown.
- * پروف ہیلتھ ونڈو اسنیپ شاٹ * PDP/POTR کاؤنٹرز ، جرمانے کی رقم کو ضم کرتا ہے ،
  کولڈاؤن پرچم ، اور ہڑتال ونڈو کے آخر میں ایپوچ فی فراہم کنندہ تاکہ گورننس کے جائزہ لینے والے
  ٹیبل کو واقعہ کے پیکٹوں سے جوڑ سکتا ہے۔

ڈی اے انفورسمنٹ شواہد پیش کرتے وقت رن بوکس کو ان پینلز کو لنک کرنا چاہئے۔ وہ
سی ایل آئی پروف اسٹریم کی ناکامیوں کو براہ راست آن چین پر پنالٹی میٹا ڈیٹا سے باندھ دیں اور
روڈ میپ میں پکارا جانے والا مشاہدہ ہک فراہم کریں۔